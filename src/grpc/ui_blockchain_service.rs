use super::blockjoy_ui::blockchain_service_server::BlockchainService;
use super::blockjoy_ui::{self, ResponseMeta};
use super::convert;
use crate::auth::UserAuthToken;
use crate::cookbook::get_networks;
use crate::errors::ApiError;
use crate::grpc::helpers::try_get_token;
use crate::grpc::{get_refresh_token, response_with_refresh_token};
use crate::models;

type Result<T, E = tonic::Status> = std::result::Result<T, E>;

impl blockjoy_ui::Blockchain {
    fn from_model(model: models::Blockchain) -> crate::Result<Self> {
        let supported_nodes_types = serde_json::to_string(&model.supported_node_types()?)?;

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
            supported_nodes_types,
            created_at: Some(convert::try_dt_to_ts(model.created_at)?),
            updated_at: Some(convert::try_dt_to_ts(model.updated_at)?),
            networks: vec![],
        };
        Ok(blockchain)
    }
}

#[tonic::async_trait]
impl BlockchainService for super::GrpcImpl {
    async fn get(
        &self,
        request: tonic::Request<blockjoy_ui::GetBlockchainRequest>,
    ) -> Result<tonic::Response<blockjoy_ui::GetBlockchainResponse>> {
        let refresh_token = get_refresh_token(&request);
        let token = try_get_token::<_, UserAuthToken>(&request)?.try_into()?;
        let inner = request.into_inner();
        let id = inner.id.parse().map_err(ApiError::from)?;
        let mut conn = self.db.conn().await?;
        let blockchain = models::Blockchain::find_by_id(id, &mut conn)
            .await
            .map_err(|_| tonic::Status::not_found("No such blockchain"))?;
        let response = blockjoy_ui::GetBlockchainResponse {
            meta: Some(ResponseMeta::from_meta(inner.meta, Some(token))),
            blockchain: Some(blockjoy_ui::Blockchain::from_model(blockchain)?),
        };
        response_with_refresh_token(refresh_token, response)
    }

    async fn list(
        &self,
        request: tonic::Request<blockjoy_ui::ListBlockchainsRequest>,
    ) -> Result<tonic::Response<blockjoy_ui::ListBlockchainsResponse>> {
        let refresh_token = get_refresh_token(&request);
        let token = try_get_token::<_, UserAuthToken>(&request)?.try_into()?;
        let inner = request.into_inner();
        let mut conn = self.db.conn().await?;
        let blockchains = models::Blockchain::find_all(&mut conn).await?;
        let mut grpc_blockchains = vec![];

        for blockchain in blockchains {
            let node_types = blockchain.supported_node_types()?;
            let name = blockchain.name.clone();
            let mut blockchain = blockjoy_ui::Blockchain::from_model(blockchain)?;

            for node_properties in node_types {
                let node_type = models::NodeType::str_from_value(node_properties.id);
                let version = Some(node_properties.version.to_string());
                let res = get_networks(name.clone(), node_type.clone(), version.clone()).await;
                let nets = match res {
                    Ok(nets) => nets,
                    Err(e) => {
                        tracing::error!(
                            "Could not get networks for {name} {node_type} version {version:?}: {e}"
                        );
                        continue;
                    }
                };

                blockchain.networks.extend(nets.iter().map(|c| c.into()));
            }

            grpc_blockchains.push(blockchain);
        }

        let response = blockjoy_ui::ListBlockchainsResponse {
            meta: Some(ResponseMeta::from_meta(inner.meta, Some(token))),
            blockchains: grpc_blockchains,
        };

        response_with_refresh_token(refresh_token, response)
    }
}
