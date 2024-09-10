pub mod job;
pub use job::{NodeJob, NodeJobProgress, NodeJobStatus, NodeJobs};

pub mod log;
pub use log::{LogEvent, NewNodeLog, NodeEvent, NodeEventData, NodeLog};

pub mod report;
pub use report::{NewNodeReport, NodeReport};

pub mod scheduler;
pub use scheduler::{NodeScheduler, ResourceAffinity, SimilarNodeAffinity};

pub mod status;
pub use status::{NextState, NodeHealth, NodeState, NodeStatus, ProtocolStatus};

use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Utc};
use diesel::dsl;
use diesel::expression::expression_types::NotSelectable;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::result::DatabaseErrorKind::UniqueViolation;
use diesel::result::Error::{DatabaseError, NotFound};
use diesel::sql_types::Bool;
use diesel_async::RunQueryDsl;
use displaydoc::Display;
use futures_util::future::OptionFuture;
use petname::{Generator, Petnames};
use thiserror::Error;
use tracing::warn;

use crate::auth::rbac::{BillingPerm, NodeAdminPerm};
use crate::auth::resource::{HostId, NodeId, OrgId, Resource, ResourceId, ResourceType, UserId};
use crate::auth::AuthZ;
use crate::database::{Conn, WriteConn};
use crate::grpc::common;
use crate::stripe::api::subscription::SubscriptionItemId;
use crate::util::sql::{self, IpNetwork, Tags, Version};
use crate::util::{SearchOperator, SortOrder};

use super::host::{Host, HostRequirements};
use super::image::config::{ConfigType, NewConfig};
use super::image::{Config, ConfigId, Image, ImageId, NodeConfig};
use super::protocol::version::{ProtocolVersion, VersionId};
use super::protocol::{Protocol, ProtocolId};
use super::schema::nodes;
use super::{Command, IpAddress, Org, Paginate, Region, RegionId};

#[derive(Debug, Display, Error)]
pub enum Error {
    /// Node Cloudflare error: {0}
    Cloudflare(#[from] crate::cloudflare::Error),
    /// Node Command error: {0}
    Command(Box<crate::model::command::Error>),
    /// Node image config error: {0}
    Config(#[from] crate::model::image::config::Error),
    /// Failed to create node: {0}
    Create(diesel::result::Error),
    /// Failed to delete node `{0}`: {1}
    Delete(NodeId, diesel::result::Error),
    /// Failed to find deleted node by id `{0}`: {1}
    FindDeletedById(NodeId, diesel::result::Error),
    /// Failed to find nodes by host id {0}: {1}
    FindByHostId(HostId, diesel::result::Error),
    /// Failed to find nodes by host ids `{0:?}`: {1}
    FindByHostIds(HashSet<HostId>, diesel::result::Error),
    /// Failed to find node by id `{0}`: {1}
    FindById(NodeId, diesel::result::Error),
    /// Failed to find nodes by ids `{0:?}`: {1}
    FindByIds(HashSet<NodeId>, diesel::result::Error),
    /// Failed to find nodes by org id {0}: {1}
    FindByOrgId(OrgId, diesel::result::Error),
    /// Failed to generate node name. This should not happen.
    GenerateName,
    /// Node host error: {0}
    Host(#[from] crate::model::host::Error),
    /// Host doesn't have enough free CPU: {0}
    HostFreeCpu(HostId),
    /// Host doesn't have enough free disk: {0}
    HostFreeDisk(HostId),
    /// Host has no free IP addresses: {0}
    HostFreeIp(HostId),
    /// Host doesn't have enough free memory: {0}
    HostFreeMem(HostId),
    /// Node image error: {0},
    Image(#[from] crate::model::image::Error),
    /// Node ip address error: {0},
    IpAddress(#[from] crate::model::ip_address::Error),
    /// Missing node-admin-transfer permission.
    MissingTransferPerm,
    /// Node log error: {0}
    NodeLog(#[from] self::log::Error),
    /// No host id or scheduler.
    NoHostOrScheduler,
    /// Failed to find a matching host.
    NoMatchingHost,
    /// Cannot launch node without a region.
    NoRegion,
    /// No host id or scheduler.
    NoScheduler,
    /// Node org error: {0}
    Org(#[from] crate::model::org::Error),
    /// Node pagination: {0}
    Paginate(#[from] crate::model::paginate::Error),
    /// Failed to parse HostId: {0}
    ParseHostId(uuid::Error),
    /// Failed to parse IpAddr: {0}
    ParseIpAddr(std::net::AddrParseError),
    /// Node protocol error: {0}
    Protocol(#[from] crate::model::protocol::Error),
    /// Node protocol version error: {0}
    ProtocolVersion(#[from] crate::model::protocol::version::Error),
    /// Node region error: {0}
    Region(#[from] crate::model::region::Error),
    /// Node report error: {0}
    Report(#[from] self::report::Error),
    /// Store error for node: {0}
    Store(#[from] crate::store::Error),
    /// Node stripe error: {0}
    Stripe(#[from] crate::stripe::Error),
    /// Failed to update the node config: {0}
    UpdateConfig(diesel::result::Error),
    /// Failed to update the node status: {0}
    UpdateStatus(diesel::result::Error),
    /// Failed to update metrics for node {0}: {1}
    UpdateMetrics(NodeId, diesel::result::Error),
    /// Failed to upgrade the node: {0}
    Upgrade(diesel::result::Error),
    /// The node is already using the requested image_id.
    UpgradeSameImage,
    /// Store error for node: {0}
    Vault(#[from] crate::store::vault::Error),
    /// Failed to parse VM cpu count: {0}
    VmCpu(std::num::TryFromIntError),
    /// Failed to parse VM memory bytes: {0}
    VmMemory(std::num::TryFromIntError),
    /// Failed to parse VM disk bytes: {0}
    VmDisk(std::num::TryFromIntError),
}

impl From<Error> for Status {
    fn from(err: Error) -> Self {
        use Error::*;
        tracing::error!("{err}");
        match err {
            Create(DatabaseError(UniqueViolation, _)) => Status::already_exists("Already exists."),
            Delete(_, NotFound)
            | FindByHostId(_, NotFound)
            | FindById(_, NotFound)
            | FindByOrgId(_, NotFound) => Status::not_found("Not found."),
            HostFreeCpu(_) => Status::failed_precondition("Host cpu."),
            HostFreeDisk(_) => Status::failed_precondition("Host memory."),
            HostFreeIp(_) => Status::failed_precondition("Host IP."),
            HostFreeMem(_) => Status::failed_precondition("Host disk."),
            MissingTransferPerm => Status::forbidden("Missing permission."),
            NoMatchingHost => Status::resource_exhausted("No matching host."),
            UpgradeSameImage => Status::already_exists("image_id"),
            Command(err) => (*err).into(),
            Config(err) => err.into(),
            Host(err) => err.into(),
            Image(err) => err.into(),
            IpAddress(err) => err.into(),
            NodeLog(err) => err.into(),
            Org(err) => err.into(),
            Paginate(err) => err.into(),
            Protocol(err) => err.into(),
            ProtocolVersion(err) => err.into(),
            Region(err) => err.into(),
            Report(err) => err.into(),
            Store(err) => err.into(),
            Vault(err) => err.into(),
            _ => Status::internal("Internal error."),
        }
    }
}

#[derive(Clone, Debug, Queryable, AsChangeset, Selectable)]
pub struct Node {
    pub id: NodeId,
    pub node_name: String,
    pub display_name: Option<String>,
    pub old_node_id: Option<NodeId>,
    pub org_id: OrgId,
    pub host_id: HostId,
    pub image_id: ImageId,
    pub config_id: ConfigId,
    pub protocol_id: ProtocolId,
    pub protocol_version_id: VersionId,
    pub semantic_version: Version,
    pub auto_upgrade: bool,
    pub node_state: NodeState,
    pub next_state: Option<NextState>,
    pub protocol_state: Option<String>,
    pub protocol_health: Option<NodeHealth>,
    pub jobs: Option<NodeJobs>,
    pub note: Option<String>,
    pub tags: Tags,
    pub ip_address: IpNetwork,
    pub ip_gateway: IpNetwork,
    pub p2p_address: Option<String>,
    pub dns_id: String,
    pub dns_name: String,
    pub dns_url: Option<String>,
    pub cpu_cores: i64,
    pub memory_bytes: i64,
    pub disk_bytes: i64,
    pub block_height: Option<i64>,
    pub block_age: Option<i64>,
    pub consensus: Option<bool>,
    pub scheduler_similarity: Option<SimilarNodeAffinity>,
    pub scheduler_resource: Option<ResourceAffinity>,
    pub scheduler_region_id: Option<RegionId>,
    pub stripe_item_id: Option<SubscriptionItemId>,
    pub created_by_type: ResourceType,
    pub created_by_id: ResourceId,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Node {
    pub async fn by_id(id: NodeId, conn: &mut Conn<'_>) -> Result<Self, Error> {
        nodes::table
            .find(id)
            .filter(nodes::deleted_at.is_null())
            .get_result(conn)
            .await
            .map_err(|err| Error::FindById(id, err))
    }

    pub async fn deleted_by_id(id: NodeId, conn: &mut Conn<'_>) -> Result<Self, Error> {
        nodes::table
            .find(id)
            .get_result(conn)
            .await
            .map_err(|err| Error::FindDeletedById(id, err))
    }

    pub async fn by_ids(ids: &HashSet<NodeId>, conn: &mut Conn<'_>) -> Result<Vec<Self>, Error> {
        nodes::table
            .filter(nodes::id.eq_any(ids))
            .filter(nodes::deleted_at.is_null())
            .get_results(conn)
            .await
            .map_err(|err| Error::FindByIds(ids.clone(), err))
    }

    pub async fn by_org_id(org_id: OrgId, conn: &mut Conn<'_>) -> Result<Vec<Self>, Error> {
        nodes::table
            .filter(nodes::org_id.eq(org_id))
            .filter(nodes::deleted_at.is_null())
            .get_results(conn)
            .await
            .map_err(|err| Error::FindByOrgId(org_id, err))
    }

    pub async fn by_host_id(host_id: HostId, conn: &mut Conn<'_>) -> Result<Vec<Self>, Error> {
        nodes::table
            .filter(nodes::host_id.eq(host_id))
            .filter(nodes::deleted_at.is_null())
            .get_results(conn)
            .await
            .map_err(|err| Error::FindByHostId(host_id, err))
    }

    pub async fn by_host_ids(
        host_ids: &HashSet<HostId>,
        conn: &mut Conn<'_>,
    ) -> Result<Vec<Self>, Error> {
        nodes::table
            .filter(nodes::host_id.eq_any(host_ids))
            .filter(nodes::deleted_at.is_null())
            .get_results(conn)
            .await
            .map_err(|err| Error::FindByHostIds(host_ids.clone(), err))
    }

    pub async fn delete(id: NodeId, write: &mut WriteConn<'_, '_>) -> Result<(), Error> {
        let node: Node = diesel::update(nodes::table.find(id))
            .set(nodes::deleted_at.eq(Utc::now()))
            .get_result(write)
            .await
            .map_err(|err| Error::Delete(id, err))?;

        Org::remove_node(node.org_id, write).await?;
        Host::remove_node(&node, write).await?;

        Command::delete_node_pending(node.id, write)
            .await
            .map_err(|err| Error::Command(Box::new(err)))?;

        if let Err(err) = write.ctx.dns.delete(&node.dns_id).await {
            warn!("Failed to remove node dns: {err}");
        }

        let prefix = format!("node/{id}/secret");
        let secrets = write.ctx.vault.read().await.list_path(&prefix).await?;
        if let Some(names) = secrets {
            for name in names {
                let path = format!("{prefix}/{name}");
                let result = write.ctx.vault.read().await.delete_path(&path).await;
                match result {
                    Ok(()) | Err(crate::store::vault::Error::PathNotFound) => (),
                    Err(err) => return Err(err.into()),
                }
            }
        }

        if let Some(item_id) = node.stripe_item_id {
            write.ctx.stripe.remove_subscription(&item_id).await?;
        }

        Ok(())
    }

    pub async fn next_host(
        &self,
        protocol: &Protocol,
        write: &mut WriteConn<'_, '_>,
    ) -> Result<Option<Host>, Error> {
        let candidates = match self.scheduler(write).await? {
            Some(scheduler) => {
                let requirements = HostRequirements {
                    scheduler,
                    protocol,
                    org_id: None,
                    cpu_cores: self.cpu_cores,
                    memory_bytes: self.memory_bytes,
                    disk_bytes: self.disk_bytes,
                };

                Host::candidates(requirements, Some(2), write).await?
            }
            None => vec![Host::by_id(self.host_id, None, write).await?],
        };

        let mut counts: HashMap<HostId, usize> = HashMap::new();
        for log in NodeLog::by_node_id(self.id, write).await? {
            if log.event == NodeEvent::CreateStarted {
                *counts.entry(log.host_id).or_insert(0) += 1;
            }
        }

        let host_ids = counts.keys().copied().collect();
        let org_ids = hashset! {};
        let hosts_tried: Vec<_> = Host::by_ids(&host_ids, &org_ids, write)
            .await?
            .into_iter()
            .map(|host @ Host { id, .. }| (host, counts[&id]))
            .collect();

        let best = match (hosts_tried.as_slice(), candidates.len()) {
            // If there are 0 hosts to try, we return None.
            (_, 0) => return Ok(None),
            // If we are on the first host to try we just take the first candidate.
            ([], _) => candidates[0].clone(),
            // If we are on the first host to try and we tried once, we try that host again.
            ([(host, 1)], 1) => host.clone(),
            // Now we need at least two candidates, so lets check for that.
            (_, 1) => return Ok(None),
            // If there is 1 host that we tried so far, we can try a new one
            ([_], _) => candidates[1].clone(),
            // If we are on the second host to try and we tried once, we try that host again.
            ([_, (host, 1)], _) => host.clone(),
            // Otherwise we exhausted our our options and return None
            (_, _) => return Ok(None),
        };

        Ok(Some(best))
    }

    pub async fn scheduler(&self, conn: &mut Conn<'_>) -> Result<Option<NodeScheduler>, Error> {
        let Some(resource) = self.scheduler_resource else {
            return Ok(None);
        };

        Ok(Some(NodeScheduler {
            region: self.region(conn).await?,
            similarity: self.scheduler_similarity,
            resource,
        }))
    }

    pub async fn region(&self, conn: &mut Conn<'_>) -> Result<Option<Region>, Error> {
        let Some(region_id) = self.scheduler_region_id else {
            return Ok(None);
        };

        Region::by_id(region_id, conn)
            .await
            .map(Some)
            .map_err(Error::Region)
    }

    pub fn status(&self) -> NodeStatus {
        NodeStatus {
            state: self.node_state,
            next: self.next_state,
            protocol: match (&self.protocol_state, self.protocol_health) {
                (Some(state), Some(health)) => Some(ProtocolStatus {
                    state: state.clone(),
                    health,
                }),
                _ => None,
            },
        }
    }

    pub async fn report(
        &self,
        created_by: Resource,
        message: String,
        conn: &mut Conn<'_>,
    ) -> Result<NodeReport, Error> {
        let report = NewNodeReport {
            node_id: self.id,
            created_by_type: created_by.typ(),
            created_by_id: created_by.id(),
            message,
        };
        report.create(conn).await.map_err(Error::Report)
    }

    pub fn created_by(&self) -> Resource {
        Resource::new(self.created_by_type, self.created_by_id)
    }
}

#[derive(Debug)]
pub struct NodeSearch {
    pub operator: SearchOperator,
    pub id: Option<String>,
    pub node_name: Option<String>,
    pub display_name: Option<String>,
    pub dns_name: Option<String>,
    pub ip: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub enum NodeSort {
    NodeName(SortOrder),
    DnsName(SortOrder),
    DisplayName(SortOrder),
    NodeState(SortOrder),
    NextState(SortOrder),
    ProtocolState(SortOrder),
    ProtocolHealth(SortOrder),
    BlockHeight(SortOrder),
    CreatedAt(SortOrder),
    UpdatedAt(SortOrder),
}

impl NodeSort {
    fn into_expr<T>(self) -> Box<dyn BoxableExpression<T, Pg, SqlType = NotSelectable>>
    where
        nodes::node_name: SelectableExpression<T>,
        nodes::dns_name: SelectableExpression<T>,
        nodes::display_name: SelectableExpression<T>,
        nodes::created_at: SelectableExpression<T>,
        nodes::updated_at: SelectableExpression<T>,
        nodes::node_state: SelectableExpression<T>,
        nodes::next_state: SelectableExpression<T>,
        nodes::protocol_state: SelectableExpression<T>,
        nodes::protocol_health: SelectableExpression<T>,
        nodes::block_height: SelectableExpression<T>,
    {
        use NodeSort::*;
        use SortOrder::*;

        match self {
            NodeName(Asc) => Box::new(nodes::node_name.asc()),
            NodeName(Desc) => Box::new(nodes::node_name.desc()),

            DnsName(Asc) => Box::new(nodes::dns_name.asc()),
            DnsName(Desc) => Box::new(nodes::dns_name.desc()),

            DisplayName(Asc) => Box::new(nodes::display_name.asc()),
            DisplayName(Desc) => Box::new(nodes::display_name.desc()),

            NodeState(Asc) => Box::new(nodes::node_state.asc()),
            NodeState(Desc) => Box::new(nodes::node_state.desc()),

            NextState(Asc) => Box::new(nodes::next_state.asc()),
            NextState(Desc) => Box::new(nodes::next_state.desc()),

            ProtocolState(Asc) => Box::new(nodes::protocol_state.asc()),
            ProtocolState(Desc) => Box::new(nodes::protocol_state.desc()),

            ProtocolHealth(Asc) => Box::new(nodes::protocol_health.asc()),
            ProtocolHealth(Desc) => Box::new(nodes::protocol_health.desc()),

            BlockHeight(Asc) => Box::new(nodes::block_height.asc()),
            BlockHeight(Desc) => Box::new(nodes::block_height.desc()),

            CreatedAt(Asc) => Box::new(nodes::created_at.asc()),
            CreatedAt(Desc) => Box::new(nodes::created_at.desc()),

            UpdatedAt(Asc) => Box::new(nodes::updated_at.asc()),
            UpdatedAt(Desc) => Box::new(nodes::updated_at.desc()),
        }
    }
}

#[derive(Debug)]
pub struct NodeFilter {
    pub org_ids: Vec<OrgId>,
    pub protocol_ids: Vec<ProtocolId>,
    pub host_ids: Vec<HostId>,
    pub user_ids: Vec<UserId>,
    pub ip_addresses: Vec<IpNetwork>,
    pub node_states: Vec<NodeState>,
    pub next_states: Vec<NextState>,
    pub semantic_versions: Vec<String>,
    pub search: Option<NodeSearch>,
    pub sort: VecDeque<NodeSort>,
    pub offset: u64,
    pub limit: u64,
}

impl NodeFilter {
    const DELETED_STATES: &[NodeState] = &[NodeState::Deleting, NodeState::Deleted];

    pub async fn query(mut self, conn: &mut Conn<'_>) -> Result<(Vec<Node>, u64), Error> {
        let mut query = nodes::table.into_boxed();

        if let Some(search) = self.search {
            query = query.filter(search.into_expression());
        }

        if !self.org_ids.is_empty() {
            query = query.filter(nodes::org_id.eq_any(self.org_ids));
        }

        if !self.host_ids.is_empty() {
            query = query.filter(nodes::host_id.eq_any(self.host_ids));
        }

        if !self.user_ids.is_empty() {
            query = query.filter(nodes::created_by_id.eq_any(self.user_ids));
        }

        if !self.protocol_ids.is_empty() {
            query = query.filter(nodes::protocol_id.eq_any(self.protocol_ids));
        }

        if !self.ip_addresses.is_empty() {
            query = query.filter(nodes::ip_address.eq_any(self.ip_addresses));
        }

        if !self.semantic_versions.is_empty() {
            query = query.filter(nodes::semantic_version.eq_any(self.semantic_versions));
        }

        // exclude deleted nodes unless a deleted state is requested.
        if !self
            .node_states
            .iter()
            .any(|s| Self::DELETED_STATES.contains(s))
        {
            query = query.filter(nodes::deleted_at.is_null());
        }

        if !self.node_states.is_empty() {
            query = query.filter(nodes::node_state.eq_any(self.node_states));
        }

        if !self.next_states.is_empty() {
            query = query.filter(nodes::next_state.eq_any(self.next_states));
        }

        if let Some(sort) = self.sort.pop_front() {
            query = query.order_by(sort.into_expr());
        } else {
            query = query.order_by(nodes::created_at.desc());
        }

        while let Some(sort) = self.sort.pop_front() {
            query = query.then_order_by(sort.into_expr());
        }

        query
            .select(Node::as_select())
            .paginate(self.limit, self.offset)?
            .count_results(conn)
            .await
            .map_err(Into::into)
    }
}

impl NodeSearch {
    fn into_expression(self) -> Box<dyn BoxableExpression<nodes::table, Pg, SqlType = Bool>> {
        match self.operator {
            SearchOperator::Or => {
                let mut predicate: Box<dyn BoxableExpression<nodes::table, Pg, SqlType = Bool>> =
                    Box::new(false.into_sql::<Bool>());
                if let Some(id) = self.id {
                    predicate = Box::new(predicate.or(sql::text(nodes::id).like(id)));
                }
                if let Some(name) = self.node_name {
                    predicate = Box::new(predicate.or(sql::lower(nodes::node_name).like(name)));
                }
                if let Some(name) = self.display_name {
                    predicate = Box::new(
                        predicate.or(sql::lower(sql::coalesce(nodes::display_name, "")).like(name)),
                    );
                }
                if let Some(name) = self.dns_name {
                    predicate = Box::new(predicate.or(sql::lower(nodes::dns_name).like(name)));
                }
                if let Some(ip) = self.ip {
                    predicate = Box::new(predicate.or(dsl::abbrev(nodes::ip_address).like(ip)));
                }
                predicate
            }
            SearchOperator::And => {
                let mut predicate: Box<dyn BoxableExpression<nodes::table, Pg, SqlType = Bool>> =
                    Box::new(true.into_sql::<Bool>());
                if let Some(id) = self.id {
                    predicate = Box::new(predicate.and(sql::text(nodes::id).like(id)));
                }
                if let Some(name) = self.node_name {
                    predicate = Box::new(predicate.and(sql::lower(nodes::node_name).like(name)));
                }
                if let Some(name) = self.display_name {
                    predicate = Box::new(
                        predicate
                            .and(sql::lower(sql::coalesce(nodes::display_name, "")).like(name)),
                    );
                }
                if let Some(name) = self.dns_name {
                    predicate = Box::new(predicate.and(sql::lower(nodes::dns_name).like(name)));
                }
                if let Some(ip) = self.ip {
                    predicate = Box::new(predicate.and(dsl::abbrev(nodes::ip_address).like(ip)));
                }
                predicate
            }
        }
    }
}

#[derive(Debug, Insertable)]
#[diesel(table_name = nodes)]
pub struct NewNode {
    pub org_id: OrgId,
    pub image_id: ImageId,
    pub config_id: ConfigId,
    pub old_node_id: Option<NodeId>,
    pub protocol_id: ProtocolId,
    pub protocol_version_id: VersionId,
    pub semantic_version: Version,
    pub auto_upgrade: bool,
    pub scheduler_similarity: Option<SimilarNodeAffinity>,
    pub scheduler_resource: Option<ResourceAffinity>,
    pub scheduler_region_id: Option<RegionId>,
    pub tags: Tags,
}

impl NewNode {
    pub async fn create(
        &self,
        node_counts: Option<Vec<NodeCount>>,
        created_by: Resource,
        authz: &AuthZ,
        write: &mut WriteConn<'_, '_>,
    ) -> Result<Vec<Node>, Error> {
        let config = Config::by_id(self.config_id, write).await?;
        let node_config = config.node_config()?;

        let org = Org::by_id(self.org_id, write).await?;
        let version =
            ProtocolVersion::by_id(self.protocol_version_id, Some(self.org_id), authz, write)
                .await?;

        let secrets = if let Some(old_id) = self.old_node_id {
            let prefix = format!("node/{old_id}/secret");
            let names = write.ctx.vault.read().await.list_path(&prefix).await?;

            if let Some(names) = names {
                let mut secrets = HashMap::with_capacity(names.len());
                for name in names {
                    let path = format!("{prefix}/{name}");
                    let result = write.ctx.vault.read().await.get_bytes(&path).await;
                    let _ = match result {
                        Ok(data) => secrets.insert(name.clone(), data),
                        Err(crate::store::vault::Error::PathNotFound) => None,
                        Err(err) => return Err(err.into()),
                    };
                }
                Some(secrets)
            } else {
                None
            }
        } else {
            None
        };

        let mut created = Vec::new();
        if let Some(counts) = node_counts {
            for count in counts {
                let host = Host::by_id(count.host_id, None, write).await?;
                for _ in 0..count.node_count {
                    match self
                        .create_node(
                            &host,
                            &org,
                            &version,
                            &node_config,
                            secrets.as_ref(),
                            created_by,
                            authz,
                            write,
                        )
                        .await
                    {
                        Ok(node) => created.push(node),
                        Err(err) => {
                            for node in created {
                                if let Err(err) = write.ctx.dns.delete(&node.dns_id).await {
                                    warn!("Failed to delete DNS record {}: {err}", node.dns_id);
                                }
                            }

                            return Err(err);
                        }
                    }
                }
            }
        } else {
            let scheduler = self
                .scheduler(write)
                .await?
                .ok_or(Error::NoHostOrScheduler)?;
            let host = self.find_host(scheduler, authz, write).await?;
            let node = self
                .create_node(
                    &host,
                    &org,
                    &version,
                    &node_config,
                    secrets.as_ref(),
                    created_by,
                    authz,
                    write,
                )
                .await?;
            created.push(node);
        }

        Ok(created)
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_node(
        &self,
        host: &Host,
        org: &Org,
        version: &ProtocolVersion,
        node_config: &NodeConfig,
        secrets: Option<&HashMap<String, Vec<u8>>>,
        created_by: Resource,
        authz: &AuthZ,
        mut write: &mut WriteConn<'_, '_>,
    ) -> Result<Node, Error> {
        let cpu_cores = i64::try_from(node_config.vm.cpu_cores).map_err(Error::VmCpu)?;
        let memory_bytes = i64::try_from(node_config.vm.memory_bytes).map_err(Error::VmMemory)?;
        let disk_bytes = i64::try_from(node_config.vm.disk_bytes).map_err(Error::VmDisk)?;

        if cpu_cores + host.node_cpu_cores > host.cpu_cores {
            return Err(Error::HostFreeCpu(host.id));
        } else if memory_bytes + host.node_memory_bytes > host.memory_bytes {
            return Err(Error::HostFreeMem(host.id));
        } else if disk_bytes + host.node_disk_bytes > host.disk_bytes {
            return Err(Error::HostFreeDisk(host.id));
        }

        let ip_address = IpAddress::next_for_host(host.id, write)
            .await?
            .ok_or_else(|| Error::HostFreeIp(host.id))?;

        let stripe_item_id = if authz.has_perm(BillingPerm::Exempt) {
            None
        } else {
            let region = Region::by_id(host.region_id.ok_or(Error::NoRegion)?, write).await?;
            if let Some(sku) = version.sku(&region) {
                let item = write.ctx.stripe.add_subscription(org, &sku).await?;
                Some(item.id)
            } else {
                None
            }
        };

        loop {
            let name = Petnames::small()
                .generate_one(3, "-")
                .ok_or(Error::GenerateName)?;
            let dns_id = write.ctx.dns.create(&name, ip_address.ip.ip()).await?.id;

            match diesel::insert_into(nodes::table)
                .values((
                    self,
                    nodes::node_name.eq(&name),
                    nodes::host_id.eq(host.id),
                    nodes::node_state.eq(NodeState::Starting),
                    nodes::ip_address.eq(&ip_address.ip),
                    nodes::ip_gateway.eq(&host.ip_gateway),
                    nodes::dns_id.eq(&dns_id),
                    nodes::dns_name.eq(&name),
                    nodes::cpu_cores.eq(cpu_cores),
                    nodes::memory_bytes.eq(memory_bytes),
                    nodes::disk_bytes.eq(disk_bytes),
                    nodes::stripe_item_id.eq(&stripe_item_id),
                    nodes::created_by_type.eq(created_by.typ()),
                    nodes::created_by_id.eq(created_by.id()),
                    nodes::created_at.eq(Utc::now()),
                ))
                .get_result::<Node>(&mut write)
                .await
            {
                Ok(node) => {
                    Org::add_node(self.org_id, write).await?;
                    Host::add_node(&node, write).await?;

                    if let Some(secrets) = secrets {
                        for (name, data) in secrets {
                            let path = format!("node/{}/secret/{name}", node.id);
                            let _version =
                                write.ctx.vault.read().await.set_bytes(&path, data).await?;
                        }
                    }

                    return Ok(node);
                }

                Err(err) => {
                    if let Err(err) = write.ctx.dns.delete(&dns_id).await {
                        warn!("Failed to delete DNS record {dns_id}: {err}");
                    }

                    if let DatabaseError(UniqueViolation, ref info) = err {
                        if info.column_name() == Some("name") {
                            warn!("Node name {} already taken. Retrying...", name);
                            continue;
                        }
                    }

                    return Err(Error::Create(err));
                }
            }
        }
    }

    /// Finds the most suitable host to place the node on.
    async fn find_host(
        &self,
        scheduler: NodeScheduler,
        authz: &AuthZ,
        conn: &mut Conn<'_>,
    ) -> Result<Host, Error> {
        let config = Config::by_id(self.config_id, conn).await?;
        let node_config = config.node_config()?;
        let protocol = Protocol::by_id(self.protocol_id, Some(self.org_id), authz, conn).await?;

        let requirements = HostRequirements {
            scheduler,
            protocol: &protocol,
            org_id: None,
            cpu_cores: i64::try_from(node_config.vm.cpu_cores).map_err(Error::VmCpu)?,
            memory_bytes: i64::try_from(node_config.vm.memory_bytes).map_err(Error::VmMemory)?,
            disk_bytes: i64::try_from(node_config.vm.disk_bytes).map_err(Error::VmDisk)?,
        };

        let candidates = Host::candidates(requirements, Some(1), conn).await?;
        candidates.into_iter().next().ok_or(Error::NoMatchingHost)
    }

    async fn scheduler(&self, conn: &mut Conn<'_>) -> Result<Option<NodeScheduler>, Error> {
        let Some(resource) = self.scheduler_resource else {
            return Ok(None);
        };
        let region = self.scheduler_region_id.map(|id| Region::by_id(id, conn));
        let region = OptionFuture::from(region).await.transpose()?;

        Ok(Some(NodeScheduler {
            region,
            similarity: self.scheduler_similarity,
            resource,
        }))
    }
}

pub struct NodeCount {
    pub host_id: HostId,
    pub node_count: u32,
}

impl NodeCount {
    pub const fn one(host_id: HostId) -> Self {
        NodeCount {
            host_id,
            node_count: 1,
        }
    }
}

impl TryFrom<&common::NodeCount> for NodeCount {
    type Error = Error;

    fn try_from(count: &common::NodeCount) -> Result<Self, Self::Error> {
        Ok(NodeCount {
            host_id: count.host_id.parse().map_err(Error::ParseHostId)?,
            node_count: count.node_count,
        })
    }
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = nodes)]
pub struct UpdateNode<'u> {
    pub id: NodeId,
    pub org_id: Option<OrgId>,
    pub host_id: Option<HostId>,
    pub display_name: Option<&'u str>,
    pub auto_upgrade: Option<bool>,
    pub ip_address: Option<IpNetwork>,
    pub ip_gateway: Option<IpNetwork>,
    pub note: Option<&'u str>,
    pub tags: Option<Tags>,
}

impl<'u> UpdateNode<'u> {
    pub async fn apply(self, authz: &AuthZ, conn: &mut Conn<'_>) -> Result<Node, Error> {
        let node = Node::by_id(self.id, conn).await?;

        if let Some(org_id) = self.org_id {
            if org_id != node.org_id {
                if !authz.has_perm(NodeAdminPerm::Transfer) {
                    return Err(Error::MissingTransferPerm);
                }

                let event = LogEvent::OrgTransferred(log::OrgTransferred {
                    old: node.org_id,
                    new: org_id,
                });
                NewNodeLog::from(&node, authz, event).create(conn).await?;
            }
        }

        diesel::update(nodes::table.find(self.id))
            .set((self, nodes::updated_at.eq(Utc::now())))
            .get_result(conn)
            .await
            .map_err(Error::UpdateConfig)
    }
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = nodes)]
pub struct UpdateNodeState<'u> {
    pub id: NodeId,
    pub node_state: Option<NodeState>,
    pub next_state: Option<Option<NextState>>,
    pub protocol_state: Option<String>,
    pub protocol_health: Option<NodeHealth>,
    pub p2p_address: Option<&'u str>,
}

impl<'u> UpdateNodeState<'u> {
    pub async fn apply(self, conn: &mut Conn<'_>) -> Result<Node, Error> {
        diesel::update(nodes::table.find(self.id))
            .set((self, nodes::updated_at.eq(Utc::now())))
            .get_result(conn)
            .await
            .map_err(Error::UpdateStatus)
    }
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = nodes)]
pub struct UpdateNodeMetrics {
    pub id: NodeId,
    pub node_state: Option<NodeState>,
    pub protocol_state: Option<String>,
    pub protocol_health: Option<NodeHealth>,
    pub block_height: Option<i64>,
    pub block_age: Option<i64>,
    pub consensus: Option<bool>,
    pub jobs: Option<NodeJobs>,
}

impl UpdateNodeMetrics {
    pub async fn apply(&self, conn: &mut Conn<'_>) -> Result<Node, Error> {
        let row = nodes::table
            .find(self.id)
            .filter(nodes::deleted_at.is_null());

        diesel::update(row)
            .set(self)
            .get_result(conn)
            .await
            .map_err(|err| Error::UpdateMetrics(self.id, err))
    }

    pub async fn apply_all(updates: Vec<Self>, conn: &mut Conn<'_>) -> Result<Vec<Node>, Error> {
        let mut results = Vec::with_capacity(updates.len());
        for update in updates {
            let updated = update.apply(conn).await?;
            results.push(updated);
        }
        Ok(results)
    }
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = nodes)]
pub struct UpgradeNode {
    pub id: NodeId,
    pub org_id: Option<OrgId>,
    pub image_id: ImageId,
}

impl UpgradeNode {
    pub async fn apply(self, authz: &AuthZ, conn: &mut Conn<'_>) -> Result<Node, Error> {
        let node = Node::by_id(self.id, conn).await?;
        let config = Config::by_id(node.config_id, conn).await?;
        let org_id = Some(self.org_id.unwrap_or(node.org_id));

        if self.image_id == config.image_id {
            return Err(Error::UpgradeSameImage);
        }

        let image = Image::by_id(self.image_id, org_id, authz, conn).await?;
        let old_config = config.node_config()?;
        let new_config = old_config.upgrade(image, org_id, conn).await?;

        let new_config = NewConfig {
            image_id: self.image_id,
            archive_id: new_config.image.archive_id,
            config_type: ConfigType::Node,
            config: new_config.into(),
        };
        let config = new_config.create(authz, conn).await?;

        let event = LogEvent::UpgradeStarted(log::UpgradeStarted {
            old: node.image_id,
            new: self.image_id,
        });
        NewNodeLog::from(&node, authz, event).create(conn).await?;

        diesel::update(nodes::table.find(self.id))
            .set((
                nodes::image_id.eq(self.image_id),
                nodes::config_id.eq(config.id),
                nodes::updated_at.eq(Utc::now()),
            ))
            .get_result(conn)
            .await
            .map_err(Error::Upgrade)
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use crate::auth::rbac::access::tests::view_authz;
    use crate::config::Context;

    use super::*;

    #[tokio::test]
    async fn can_filter_nodes() {
        let (ctx, db) = Context::with_mocked().await.unwrap();
        let (meta_tx, _meta_rx) = mpsc::unbounded_channel();
        let (mqtt_tx, _mqtt_rx) = mpsc::unbounded_channel();
        let mut write = WriteConn {
            conn: &mut db.conn().await,
            ctx: &ctx,
            meta_tx,
            mqtt_tx,
        };

        let new_node = NewNode {
            org_id: db.seed.org.id,
            image_id: db.seed.image.id,
            config_id: db.seed.config.id,
            old_node_id: None,
            protocol_id: db.seed.protocol.id,
            protocol_version_id: db.seed.version.id,
            semantic_version: "1.2.3".parse().unwrap(),
            auto_upgrade: false,
            scheduler_similarity: None,
            scheduler_resource: Some(ResourceAffinity::MostResources),
            scheduler_region_id: None,
            tags: Default::default(),
        };

        let node_counts = Some(vec![NodeCount::one(db.seed.host1.id)]);
        let authz = view_authz(db.seed.node.id);
        let created_by = Resource::from(&authz);

        new_node
            .create(node_counts, created_by, &authz, &mut write)
            .await
            .unwrap();

        let filter = NodeFilter {
            org_ids: vec![db.seed.org.id],
            protocol_ids: vec![db.seed.protocol.id],
            host_ids: vec![db.seed.host1.id],
            user_ids: vec![],
            ip_addresses: vec![],
            node_states: vec![NodeState::Running],
            next_states: vec![],
            semantic_versions: vec![],
            search: None,
            sort: VecDeque::new(),
            offset: 0,
            limit: 10,
        };

        let (nodes, _count) = filter.query(&mut write).await.unwrap();
        assert_eq!(nodes.len(), 1);
    }
}
