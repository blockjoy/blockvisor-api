use super::api::{self, hosts_server};
use super::helpers;
use crate::auth::{HostAuthToken, JwtToken, TokenRole, TokenType};
use crate::models;
use diesel_async::scoped_futures::ScopedFutureExt;
use tonic::{Request, Response};

#[tonic::async_trait]
impl hosts_server::Hosts for super::GrpcImpl {
    /// Get a host by id.
    async fn get(
        &self,
        request: Request<api::GetHostRequest>,
    ) -> super::Result<api::GetHostResponse> {
        let refresh_token = super::get_refresh_token(&request);
        let request = request.into_inner();
        let host_id = request.id.parse().map_err(crate::Error::from)?;
        let mut conn = self.conn().await?;
        let host = models::Host::find_by_id(host_id, &mut conn).await?;
        let host = api::Host::from_model(host).await?;
        let response = api::GetHostResponse { host: Some(host) };
        super::response_with_refresh_token(refresh_token, response)
    }

    async fn list(
        &self,
        request: Request<api::ListHostsRequest>,
    ) -> super::Result<api::ListHostsResponse> {
        let refresh_token = super::get_refresh_token(&request);
        let mut conn = self.conn().await?;
        let hosts = models::Host::filter(&mut conn).await?;
        let hosts = api::Host::from_models(hosts).await?;
        let response = api::ListHostsResponse { hosts };
        super::response_with_refresh_token(refresh_token, response)
    }

    async fn create(
        &self,
        request: Request<api::CreateHostRequest>,
    ) -> super::Result<api::CreateHostResponse> {
        let request = request.into_inner();
        let new_host = request.as_new()?;
        self.trx(|c| new_host.create(c).scope_boxed()).await?;
        let response = api::CreateHostResponse {};

        Ok(Response::new(response))
    }

    async fn update(
        &self,
        request: Request<api::UpdateHostRequest>,
    ) -> super::Result<api::UpdateHostResponse> {
        let request = request.into_inner();
        let updater = request.as_update()?;
        self.trx(|c| updater.update(c).scope_boxed()).await?;
        let response = api::UpdateHostResponse {};
        Ok(Response::new(response))
    }

    async fn delete(
        &self,
        request: Request<api::DeleteHostRequest>,
    ) -> super::Result<api::DeleteHostResponse> {
        let request = request.into_inner();
        let host_id = request.id.parse().map_err(crate::Error::from)?;
        self.trx(|c| models::Host::delete(host_id, c).scope_boxed())
            .await?;
        let response = api::DeleteHostResponse {};

        Ok(Response::new(response))
    }

    async fn provision(
        &self,
        request: Request<api::ProvisionHostRequest>,
    ) -> super::Result<api::ProvisionHostResponse> {
        let inner = request.into_inner();
        let request_id = inner.request_id.clone();

        let host = self
            .trx(|c| models::HostProvision::claim_by_grpc_provision(&inner, c).scope_boxed())
            .await?;
        let token: HostAuthToken = JwtToken::create_token_for::<models::Host>(
            &host,
            TokenType::HostAuth,
            TokenRole::Service,
            None,
        )?;
        let token = token.encode()?;
        let result = api::ProvisionHostResponse {
            host_id: host.id.to_string(),
            token,
            messages: vec!["All good".into()],
            origin_request_id: request_id,
        };
        Ok(Response::new(result))
    }
}

impl api::Host {
    pub async fn from_models(models: Vec<models::Host>) -> crate::Result<Vec<Self>> {
        models
            .into_iter()
            .map(|model| {
                let dto = Self {
                    id: model.id.to_string(),
                    name: model.name,
                    version: model.version,
                    location: model.location,
                    cpu_count: model.cpu_count.map(|n| n.try_into()).transpose()?,
                    mem_size: model.mem_size.map(|n| n.try_into()).transpose()?,
                    disk_size: model.disk_size.map(|n| n.try_into()).transpose()?,
                    os: model.os,
                    os_version: model.os_version,
                    ip: model.ip_addr,
                    status: model.status.into(),
                    created_at: Some(super::try_dt_to_ts(model.created_at)?),
                    ip_range_from: Some(model.ip_range_from.ip().to_string()),
                    ip_range_to: Some(model.ip_range_to.ip().to_string()),
                    ip_gateway: Some(model.ip_gateway.ip().to_string()),
                };
                Ok(dto)
            })
            .collect()
    }

    pub async fn from_model(model: models::Host) -> crate::Result<Self> {
        Ok(Self::from_models(vec![model]).await?[0].clone())
    }
}

impl api::CreateHostRequest {
    pub fn as_new(&self) -> crate::Result<models::NewHost<'_>> {
        Ok(models::NewHost {
            name: &self.name,
            version: self.version.as_deref(),
            location: self.location.as_deref(),
            cpu_count: self.cpu_count.map(|n| n.try_into()).transpose()?,
            mem_size: self.mem_size.map(|n| n.try_into()).transpose()?,
            disk_size: self.disk_size.map(|n| n.try_into()).transpose()?,
            os: self.os.as_deref(),
            os_version: self.os_version.as_deref(),
            ip_addr: &self.ip_addr,
            status: models::ConnectionStatus::Online,
            ip_range_from: self.ip_range_from.parse()?,
            ip_range_to: self.ip_range_to.parse()?,
            ip_gateway: self.ip_gateway.parse()?,
        })
    }
}

impl api::UpdateHostRequest {
    pub fn as_update(&self) -> crate::Result<models::UpdateHost<'_>> {
        Ok(models::UpdateHost {
            id: self.id.parse()?,
            name: self.name.as_deref(),
            version: self.version.as_deref(),
            location: self.location.as_deref(),
            cpu_count: None,
            mem_size: None,
            disk_size: None,
            os: self.os.as_deref(),
            os_version: self.os_version.as_deref(),
            ip_addr: None,
            status: None,
            ip_range_from: None,
            ip_range_to: None,
            ip_gateway: None,
        })
    }
}

impl api::ProvisionHostRequest {
    pub fn as_new(&self, provision: models::HostProvision) -> crate::Result<models::NewHost<'_>> {
        let new_host = models::NewHost {
            name: &self.name,
            version: Some(&self.version),
            location: None,
            cpu_count: Some(self.cpu_count),
            mem_size: Some(self.mem_size_bytes),
            disk_size: Some(self.disk_size_bytes),
            os: Some(&self.os),
            os_version: Some(&self.os_version),
            ip_addr: &self.ip,
            status: self.status.try_into()?,
            ip_range_from: provision
                .ip_range_from
                .ok_or_else(helpers::required("provision.ip_range_from"))?,
            ip_range_to: provision
                .ip_range_to
                .ok_or_else(helpers::required("provision.ip_range_to"))?,
            ip_gateway: provision
                .ip_gateway
                .ok_or_else(helpers::required("provision.ip_gateway"))?,
        };
        Ok(new_host)
    }
}
