use super::blockjoy::Parameter;
use crate::errors::Result as ApiResult;
use crate::grpc::blockjoy::container_image::StatusName;
use crate::grpc::blockjoy::{
    self, command, node_command, Command as GrpcCommand, ContainerImage, NodeCommand, NodeCreate,
    NodeDelete, NodeInfoGet, NodeRestart, NodeStop,
};
use crate::grpc::helpers::required;
use crate::models;
use crate::models::{Blockchain, Command, HostCmd, Node, NodeTypeKey};
use prost_types::Timestamp;

impl Parameter {
    fn new(name: &str, val: &str) -> Self {
        Self {
            name: name.to_owned(),
            value: val.to_owned(),
        }
    }
}

pub async fn db_command_to_grpc_command(
    cmd: Command,
    db: &models::DbPool,
) -> ApiResult<GrpcCommand> {
    let mut node_cmd = NodeCommand {
        id: cmd.resource_id.to_string(),
        command: None,
        api_command_id: cmd.id.to_string(),
        created_at: None,
    };

    node_cmd.command = match cmd.cmd {
        HostCmd::RestartNode => Some(node_command::Command::Restart(NodeRestart::default())),
        HostCmd::KillNode => {
            tracing::debug!("Using NodeStop for KillNode");
            Some(node_command::Command::Stop(NodeStop::default()))
        }
        HostCmd::ShutdownNode => Some(node_command::Command::Stop(NodeStop::default())),
        HostCmd::UpdateNode => {
            tracing::debug!("Using NodeUpgrade for UpdateNode");

            // TODO: add image
            // Self {
            //     r#type: Some(command::Type::Node(NodeUpgrade {})),
            // }

            let mut conn = db.conn().await?;
            let node = Node::find_by_id(cmd.resource_id, &mut conn).await?;
            let network = Parameter::new("network", &node.network);
            let cmd = blockjoy::NodeInfoUpdate {
                name: node.name,
                self_update: Some(node.self_update),
                properties: node
                    .node_type
                    .iter_props()
                    .flat_map(|p| p.value().as_ref().map(|v| (p.name(), v)))
                    .map(|(name, value)| Parameter::new(name, value))
                    .chain([network])
                    .collect(),
            };

            Some(node_command::Command::Update(cmd))
        }
        HostCmd::MigrateNode => {
            tracing::debug!("Using NodeGenericCommand for MigrateNode");
            unimplemented!();
            /*
            node_cmd.command = Some(node_command::Command::Generic(NodeGenericCommand::default()))
             */
        }
        HostCmd::GetNodeVersion => {
            tracing::debug!("Using NodeInfoGet for GetNodeVersion");
            Some(node_command::Command::InfoGet(NodeInfoGet::default()))
        }
        // The following should be HostCommands
        HostCmd::CreateNode => {
            let mut conn = db.conn().await?;
            let node = Node::find_by_id(cmd.resource_id, &mut conn).await?;
            let blockchain = Blockchain::find_by_id(node.blockchain_id, &mut conn).await?;
            let image = ContainerImage {
                protocol: blockchain.name,
                node_type: NodeTypeKey::str_from_value(node.node_type.0.get_id()).to_lowercase(),
                node_version: node
                    .version
                    .clone()
                    .unwrap_or_else(|| "latest".to_string())
                    .to_lowercase(),
                status: StatusName::Development.into(),
            };
            let network = Parameter::new("network", &node.network);
            let create_cmd = NodeCreate {
                name: node.name.unwrap_or_default(),
                blockchain: node.blockchain_id.to_string(),
                image: Some(image),
                r#type: node.node_type.to_json()?,
                ip: node.ip_addr.ok_or_else(required("node.ip_addr"))?,
                gateway: node.ip_gateway.ok_or_else(required("node.ip_gateway"))?,
                self_update: node.self_update,
                properties: node
                    .node_type
                    .iter_props()
                    .flat_map(|p| p.value().as_ref().map(|v| (p.name(), v)))
                    .map(|(name, value)| Parameter::new(name, value))
                    .chain([network])
                    .collect(),
            };

            Some(node_command::Command::Create(create_cmd))
        }
        HostCmd::DeleteNode => Some(node_command::Command::Delete(NodeDelete {})),
        HostCmd::GetBVSVersion => unimplemented!(),
        HostCmd::UpdateBVS => unimplemented!(),
        HostCmd::RestartBVS => unimplemented!(),
        HostCmd::RemoveBVS => unimplemented!(),
        HostCmd::CreateBVS => unimplemented!(),
        HostCmd::StopBVS => unimplemented!(),
        // TODO: Missing
        // NodeStart, NodeUpgrade
    };

    Ok(GrpcCommand {
        r#type: Some(command::Type::Node(node_cmd)),
    })
}

/// Function to convert the datetimes from the database into the API representation of a timestamp.
pub fn try_dt_to_ts(datetime: chrono::DateTime<chrono::Utc>) -> ApiResult<Timestamp> {
    const NANOS_PER_SEC: i64 = 1_000_000_000;
    let nanos = datetime.timestamp_nanos();
    let timestamp = Timestamp {
        seconds: nanos / NANOS_PER_SEC,
        // This _should_ never fail because 1_000_000_000 fits into an i32, but using `as` was
        // hiding this bug in the first place. Therefore I have left the `try_into` call here.
        nanos: (nanos % NANOS_PER_SEC).try_into()?,
    };
    Ok(timestamp)
}

pub mod from {
    use super::try_dt_to_ts;
    use crate::cookbook::cookbook_grpc::NetworkConfiguration;
    use crate::errors::ApiError;
    use crate::grpc;
    use crate::grpc::blockjoy::HostInfo;
    use crate::grpc::blockjoy::Keyfile;
    use crate::grpc::blockjoy_ui::blockchain_network::NetworkType;
    use crate::grpc::blockjoy_ui::{
        self, node::NodeStatus as GrpcNodeStatus, node::StakingStatus as GrpcStakingStatus,
        node::SyncStatus as GrpcSyncStatus, Host as GrpcHost, HostProvision as GrpcHostProvision,
        Node as GrpcNode, Organization, User as GrpcUiUser,
    };
    use crate::grpc::blockjoy_ui::{BlockchainNetwork, FilterCriteria};
    use crate::grpc::helpers::required;
    use crate::models::HostSelectiveUpdate;
    use crate::models::{
        self, ConnectionStatus, ContainerStatus, HostProvision, HostRequest, NodeChainStatus,
        NodeCreateRequest, NodeInfo, NodeKeyFile, NodeStakingStatus, NodeSyncStatus, Org, User,
        UserSelectiveUpdate,
    };
    use crate::models::{Invitation, NodeFilter};
    use anyhow::anyhow;
    use serde_json::Value;
    use std::i64;
    use std::net::AddrParseError;
    use std::str::FromStr;
    use std::string::FromUtf8Error;
    use tonic::{Code, Status};
    use uuid::Uuid;

    impl From<GrpcUiUser> for UserSelectiveUpdate {
        fn from(user: GrpcUiUser) -> Self {
            Self {
                first_name: user.first_name,
                last_name: user.last_name,
                fee_bps: None,
                staking_quota: None,
                refresh_token: None,
            }
        }
    }

    impl TryFrom<Invitation> for blockjoy_ui::Invitation {
        type Error = ApiError;

        fn try_from(value: Invitation) -> Result<Self, Self::Error> {
            Ok(Self {
                id: Some(value.id.to_string()),
                created_by_id: Some(value.created_by_user.to_string()),
                created_by_user_name: Some(value.created_by_user_name),
                created_for_org_id: Some(value.created_for_org.to_string()),
                created_for_org_name: Some(value.created_for_org_name),
                invitee_email: Some(value.invitee_email),
                created_at: Some(try_dt_to_ts(value.created_at)?),
                accepted_at: value.accepted_at.map(try_dt_to_ts).transpose()?,
                declined_at: value.declined_at.map(try_dt_to_ts).transpose()?,
            })
        }
    }

    impl From<HostSelectiveUpdate> for HostInfo {
        fn from(update: HostSelectiveUpdate) -> Self {
            Self {
                id: Some(update.id.to_string()),
                name: update.name,
                version: update.version,
                location: update.location,
                cpu_count: update.cpu_count,
                mem_size: update.mem_size,
                disk_size: update.disk_size,
                os: update.os,
                os_version: update.os_version,
                ip: None,
                ip_range_from: update.ip_range_from.map(|v| v.to_string()),
                ip_range_to: update.ip_range_to.map(|v| v.to_string()),
                ip_gateway: update.ip_gateway.map(|v| v.to_string()),
            }
        }
    }

    impl TryFrom<FilterCriteria> for NodeFilter {
        type Error = ();

        fn try_from(value: FilterCriteria) -> Result<Self, Self::Error> {
            Ok(Self {
                status: value.states,
                node_types: value.node_types,
                blockchains: value
                    .blockchain_ids
                    .iter()
                    .map(|id| Uuid::from_str(id.as_str()).unwrap_or_default())
                    .collect(),
            })
        }
    }

    impl TryFrom<GrpcHost> for HostSelectiveUpdate {
        type Error = ApiError;

        fn try_from(host: GrpcHost) -> Result<Self, Self::Error> {
            let updater = Self {
                id: host.id.ok_or_else(required("update.id"))?.parse()?,
                name: host.name,
                version: host.version,
                location: host.location,
                cpu_count: host.cpu_count,
                mem_size: host.mem_size,
                disk_size: host.disk_size,
                os: host.os,
                os_version: host.os_version,
                ip_addr: host.ip,
                status: None,
                ip_range_from: None,
                ip_range_to: None,
                ip_gateway: None,
                ..Default::default()
            };
            Ok(updater)
        }
    }

    impl TryFrom<HostProvision> for GrpcHostProvision {
        type Error = ApiError;

        fn try_from(hp: HostProvision) -> Result<Self, Self::Error> {
            let hp = Self {
                id: Some(hp.id),
                host_id: hp.host_id.map(|id| id.to_string()),
                created_at: Some(try_dt_to_ts(hp.created_at)?),
                claimed_at: hp.claimed_at.map(try_dt_to_ts).transpose()?,
                install_cmd: hp.install_cmd.map(String::from),
                ip_range_from: hp
                    .ip_range_from
                    .map(|ip| ip.to_string())
                    .ok_or_else(required("host_provision.ip_range_from"))?,
                ip_range_to: hp
                    .ip_range_to
                    .map(|ip| ip.to_string())
                    .ok_or_else(required("host_provision.ip_range_to"))?,
                ip_gateway: hp
                    .ip_gateway
                    .map(|ip| ip.to_string())
                    .ok_or_else(required("host_provision.ip_gateway"))?,
            };
            Ok(hp)
        }
    }

    impl TryFrom<GrpcHost> for HostRequest {
        type Error = ApiError;

        fn try_from(host: GrpcHost) -> Result<Self, Self::Error> {
            let req = Self {
                name: host.name.ok_or_else(required("host.name"))?,
                version: host.version,
                location: host.location,
                cpu_count: host.cpu_count,
                mem_size: host.mem_size,
                disk_size: host.disk_size,
                os: host.os,
                os_version: host.os_version,
                ip_addr: host.ip.ok_or_else(required("host.ip"))?,
                status: ConnectionStatus::Online,
                ip_range_from: Some(
                    host.ip_range_from
                        .ok_or_else(required("host.ip_range_from"))?
                        .parse()
                        .map_err(|e: AddrParseError| ApiError::UnexpectedError(anyhow!(e)))?,
                ),
                ip_range_to: Some(
                    host.ip_range_to
                        .ok_or_else(required("host.ip_range_to"))?
                        .parse()
                        .map_err(|e: AddrParseError| ApiError::UnexpectedError(anyhow!(e)))?,
                ),
                ip_gateway: Some(
                    host.ip_gateway
                        .ok_or_else(required("host.ip_gateway"))?
                        .parse()
                        .map_err(|e: AddrParseError| ApiError::UnexpectedError(anyhow!(e)))?,
                ),
            };
            Ok(req)
        }
    }

    impl TryFrom<&User> for GrpcUiUser {
        type Error = ApiError;

        fn try_from(user: &User) -> Result<Self, Self::Error> {
            let user = Self {
                id: Some(user.id.to_string()),
                email: Some(user.email.clone()),
                first_name: Some(user.first_name.clone()),
                last_name: Some(user.last_name.clone()),
                created_at: Some(try_dt_to_ts(user.created_at)?),
                updated_at: None,
            };
            Ok(user)
        }
    }

    impl From<ApiError> for Status {
        fn from(e: ApiError) -> Self {
            let msg = format!("{:?}", e);

            match e {
                ApiError::ValidationError(_) => Status::invalid_argument(msg),
                ApiError::NotFoundError(_) => Status::not_found(msg),
                ApiError::DuplicateResource { .. } => Status::invalid_argument(msg),
                ApiError::InvalidAuthentication(_) => Status::unauthenticated(msg),
                ApiError::InsufficientPermissionsError => Status::permission_denied(msg),
                ApiError::UuidParseError(_) => Status::invalid_argument(msg),
                _ => Status::internal(msg),
            }
        }
    }

    impl From<Status> for ApiError {
        fn from(status: Status) -> Self {
            let e = anyhow!(format!("{:?}", status));

            match status.code() {
                Code::Unauthenticated => ApiError::InvalidAuthentication(e),
                Code::PermissionDenied => ApiError::InsufficientPermissionsError,
                _ => ApiError::UnexpectedError(e),
            }
        }
    }

    impl TryFrom<User> for GrpcUiUser {
        type Error = ApiError;

        fn try_from(user: User) -> Result<Self, Self::Error> {
            let user = Self {
                id: Some(user.id.to_string()),
                email: Some(user.email),
                first_name: Some(user.first_name),
                last_name: Some(user.last_name),
                created_at: Some(try_dt_to_ts(user.created_at)?),
                updated_at: None,
            };
            Ok(user)
        }
    }

    impl TryFrom<Org> for Organization {
        type Error = ApiError;

        fn try_from(org: Org) -> Result<Self, Self::Error> {
            Organization::try_from(&org)
        }
    }

    impl TryFrom<&Org> for Organization {
        type Error = ApiError;

        fn try_from(org: &Org) -> Result<Self, Self::Error> {
            let org = Self {
                id: Some(org.id.to_string()),
                name: Some(org.name.clone()),
                personal: Some(org.is_personal),
                member_count: org.member_count,
                created_at: Some(try_dt_to_ts(org.created_at)?),
                updated_at: Some(try_dt_to_ts(org.updated_at)?),
                current_user: None,
            };
            Ok(org)
        }
    }

    impl From<models::OrgUser> for grpc::blockjoy_ui::OrgUser {
        fn from(value: models::OrgUser) -> Self {
            Self {
                user_id: value.user_id.to_string(),
                org_id: value.org_id.to_string(),
                role: value.role as i32,
            }
        }
    }

    impl TryFrom<Keyfile> for NodeKeyFile {
        type Error = ApiError;

        fn try_from(value: Keyfile) -> Result<Self, Self::Error> {
            Ok(Self {
                name: value.name,
                content: String::from_utf8(value.content)
                    .map_err(|e: FromUtf8Error| ApiError::UnexpectedError(anyhow!(e)))?,
                ..Default::default()
            })
        }
    }

    impl TryFrom<GrpcNode> for NodeCreateRequest {
        type Error = ApiError;

        fn try_from(node: GrpcNode) -> Result<Self, Self::Error> {
            let req = Self {
                org_id: Uuid::parse_str(
                    node.org_id
                        .ok_or_else(|| ApiError::validation("GrpcNode.org_id is required"))?
                        .as_str(),
                )?,
                host_name: node.host_name.unwrap_or_default(),
                name: Some(petname::petname(3, "_")),
                groups: Some(node.groups.join(",")),
                version: node.version.map(String::from),
                ip_addr: node.ip.map(String::from),
                ip_gateway: node.ip_gateway.map(String::from),
                blockchain_id: Uuid::parse_str(
                    node.blockchain_id
                        .ok_or_else(|| ApiError::validation("GrpcNode.blockchain_id is required"))?
                        .as_str(),
                )?,
                node_type: node
                    .r#type
                    .ok_or_else(required("node.type"))?
                    .try_into()
                    .map(sqlx::types::Json)
                    .map_err(|_| ApiError::validation("Node property JSON is invalid"))?,
                address: node.address.map(String::from),
                wallet_address: node.wallet_address.map(String::from),
                block_height: node.block_height.map(i64::from),
                node_data: node.node_data.map(Value::from),
                chain_status: node
                    .status
                    .ok_or_else(required("node.status"))?
                    .try_into()?,
                sync_status: NodeSyncStatus::Unknown,
                staking_status: Some(NodeStakingStatus::Unknown),
                container_status: ContainerStatus::Unknown,
                self_update: node.self_update.unwrap_or(false),
                vcpu_count: 0,
                mem_size_mb: 0,
                disk_size_gb: 0,
                network: node.network.ok_or_else(required("node.network"))?,
            };

            Ok(req)
        }
    }

    impl TryFrom<GrpcNode> for NodeInfo {
        type Error = ApiError;

        fn try_from(node: GrpcNode) -> Result<Self, Self::Error> {
            let node_info = Self {
                version: node.version,
                ip_addr: node.ip,
                block_height: node.block_height,
                node_data: node.node_data.map(Value::from),
                chain_status: node.status.map(|n| n.try_into()).transpose()?,
                sync_status: None,
                staking_status: None,
                container_status: None,
                self_update: node.self_update.unwrap_or(false),
            };
            Ok(node_info)
        }
    }

    impl From<NodeChainStatus> for GrpcNodeStatus {
        fn from(ncs: NodeChainStatus) -> Self {
            match ncs {
                NodeChainStatus::Unknown => GrpcNodeStatus::UndefinedApplicationStatus,
                NodeChainStatus::Provisioning => GrpcNodeStatus::Provisioning,
                NodeChainStatus::Broadcasting => GrpcNodeStatus::Broadcasting,
                NodeChainStatus::Cancelled => GrpcNodeStatus::Cancelled,
                NodeChainStatus::Delegating => GrpcNodeStatus::Delegating,
                NodeChainStatus::Delinquent => GrpcNodeStatus::Delinquent,
                NodeChainStatus::Disabled => GrpcNodeStatus::Disabled,
                NodeChainStatus::Earning => GrpcNodeStatus::Earning,
                NodeChainStatus::Elected => GrpcNodeStatus::Elected,
                NodeChainStatus::Electing => GrpcNodeStatus::Electing,
                NodeChainStatus::Exported => GrpcNodeStatus::Exported,
                NodeChainStatus::Ingesting => GrpcNodeStatus::Ingesting,
                NodeChainStatus::Mining => GrpcNodeStatus::Mining,
                NodeChainStatus::Minting => GrpcNodeStatus::Minting,
                NodeChainStatus::Processing => GrpcNodeStatus::Processing,
                NodeChainStatus::Relaying => GrpcNodeStatus::Relaying,
                NodeChainStatus::Removed => GrpcNodeStatus::Removed,
                NodeChainStatus::Removing => GrpcNodeStatus::Removing,
            }
        }
    }

    impl From<NodeSyncStatus> for GrpcSyncStatus {
        fn from(nss: NodeSyncStatus) -> Self {
            match nss {
                NodeSyncStatus::Unknown => GrpcSyncStatus::UndefinedSyncStatus,
                NodeSyncStatus::Synced => GrpcSyncStatus::Synced,
                NodeSyncStatus::Syncing => GrpcSyncStatus::Syncing,
            }
        }
    }

    impl From<NodeStakingStatus> for GrpcStakingStatus {
        fn from(nss: NodeStakingStatus) -> Self {
            match nss {
                NodeStakingStatus::Unknown => GrpcStakingStatus::UndefinedStakingStatus,
                NodeStakingStatus::Staked => GrpcStakingStatus::Staked,
                NodeStakingStatus::Staking => GrpcStakingStatus::Staking,
                NodeStakingStatus::Validating => GrpcStakingStatus::Validating,
                NodeStakingStatus::Follower => GrpcStakingStatus::Follower,
                NodeStakingStatus::Consensus => GrpcStakingStatus::Consensus,
                NodeStakingStatus::Unstaked => GrpcStakingStatus::Unstaked,
            }
        }
    }

    impl TryFrom<BlockchainNetwork> for crate::cookbook::BlockchainNetwork {
        type Error = ApiError;

        fn try_from(value: BlockchainNetwork) -> Result<Self, Self::Error> {
            Ok(Self {
                name: value.name,
                url: value.url,
                network_type: NetworkType::from_i32(value.net_type)
                    .ok_or_else(|| ApiError::UnexpectedError(anyhow!("Unknown network type")))?,
            })
        }
    }

    impl From<&NetworkConfiguration> for crate::cookbook::BlockchainNetwork {
        fn from(value: &NetworkConfiguration) -> Self {
            Self {
                name: value.name.clone(),
                url: value.url.clone(),
                network_type: NetworkType::from_i32(value.net_type).unwrap_or_default(),
            }
        }
    }

    impl From<&crate::cookbook::BlockchainNetwork> for BlockchainNetwork {
        fn from(value: &crate::cookbook::BlockchainNetwork) -> Self {
            Self {
                name: value.name.clone(),
                url: value.url.clone(),
                net_type: value.network_type.into(),
            }
        }
    }

    impl TryFrom<&models::Blockchain> for blockjoy_ui::Blockchain {
        type Error = ApiError;

        fn try_from(model: &models::Blockchain) -> Result<Self, Self::Error> {
            let json = &model.supported_node_types.0;
            let json = serde_json::to_string(&json)
                .map_err(|e| anyhow!("Could not serialize supported node types: {e}"))?;

            let blockchain = Self {
                id: Some(model.id.to_string()),
                name: Some(model.name.clone()),
                description: model.description.clone(),
                status: model.status as i32,
                project_url: model.project_url.clone(),
                repo_url: model.repo_url.clone(),
                supports_etl: model.supports_etl,
                supports_node: model.supports_node,
                supports_staking: model.supports_staking,
                supports_broadcast: model.supports_broadcast,
                version: model.version.clone(),
                supported_nodes_types: json,
                created_at: Some(try_dt_to_ts(model.created_at)?),
                updated_at: Some(try_dt_to_ts(model.updated_at)?),
                networks: vec![],
            };
            Ok(blockchain)
        }
    }

    impl TryFrom<models::Blockchain> for blockjoy_ui::Blockchain {
        type Error = ApiError;

        fn try_from(model: models::Blockchain) -> Result<Self, Self::Error> {
            Self::try_from(&model)
        }
    }

    impl TryFrom<NodeKeyFile> for Keyfile {
        type Error = ApiError;

        fn try_from(value: NodeKeyFile) -> Result<Self, Self::Error> {
            Ok(Self {
                name: value.name.clone(),
                content: value.content.into_bytes(),
            })
        }
    }
}

pub mod into {
    use crate::{
        errors::ApiError,
        grpc::{
            blockjoy::{HostInfo, HostInfoUpdateRequest},
            helpers::required,
        },
    };
    use tonic::Request;

    pub trait IntoData<R, T> {
        type Error;

        fn into_data(self) -> Result<T, Self::Error>;
    }

    impl IntoData<Request<HostInfoUpdateRequest>, (String, HostInfo)>
        for Request<HostInfoUpdateRequest>
    {
        type Error = ApiError;

        fn into_data(self) -> Result<(String, HostInfo), Self::Error> {
            let inner = self.into_inner();
            let id = inner.request_id.unwrap_or_default();
            let info = inner.info.ok_or_else(required("info"))?;

            Ok((id, info))
        }
    }
}
