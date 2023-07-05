use diesel_async::scoped_futures::ScopedFutureExt;
use displaydoc::Display;
use thiserror::Error;
use tonic::{Request, Response, Status};
use tracing::error;

use crate::auth::endpoint::Endpoint;
use crate::auth::resource::ResourceEntry;
use crate::models::api_key::{ApiKey, ApiResource, NewApiKey, UpdateLabel, UpdateScope};
use crate::models::Conn;
use crate::timestamp::NanosUtc;

use super::api::{self, api_key_service_server::ApiKeyService};
use super::Grpc;

#[derive(Debug, Display, Error)]
pub enum Error {
    /// Auth check failed: {0}
    Auth(#[from] crate::auth::Error),
    /// Claims check failed: {0}
    Claims(#[from] crate::auth::claims::Error),
    /// Claims Resource is not a user.
    ClaimsNotUser,
    /// Diesel failure: {0}
    Diesel(#[from] diesel::result::Error),
    /// Create API key request missing scope.
    MissingCreateScope,
    /// ApiKeyScope missing `resource_id`.
    MissingScopeResourceId,
    /// Missing API key `updated_at`.
    MissingUpdatedAt,
    /// Database model error: {0}
    Model(#[from] crate::models::api_key::Error),
    /// Nothing is set to be updated in the request.
    NothingToUpdate,
    /// Parse ApiResource: {0}
    ParseApiResource(crate::models::api_key::Error),
    /// Failed to parse KeyId: {0}
    ParseKeyId(crate::auth::token::api_key::Error),
    /// Failed to parse ResourceId: {0}
    ParseResourceId(uuid::Error),
}

impl From<Error> for Status {
    fn from(err: Error) -> Self {
        error!("{}: {err}", std::any::type_name::<Error>());

        use Error::*;
        match err {
            Auth(_) | Claims(_) | ClaimsNotUser => Status::permission_denied("Access denied."),
            Model(_) | Diesel(_) | MissingUpdatedAt => Status::internal("Internal error."),
            ParseKeyId(_) => Status::invalid_argument("id"),
            MissingCreateScope => Status::invalid_argument("scope"),
            ParseApiResource(_) => Status::invalid_argument("resource"),
            MissingScopeResourceId | ParseResourceId(_) => Status::invalid_argument("resource_id"),
            NothingToUpdate => Status::failed_precondition("Nothing to update."),
        }
    }
}

#[tonic::async_trait]
impl ApiKeyService for Grpc {
    async fn create(
        &self,
        req: Request<api::CreateApiKeyRequest>,
    ) -> super::Resp<api::CreateApiKeyResponse> {
        self.trx(|tx| create(req, tx).scope_boxed())
            .await
            .map(Response::new)
    }

    async fn list(
        &self,
        req: Request<api::ListApiKeyRequest>,
    ) -> super::Resp<api::ListApiKeyResponse> {
        list(req, &mut self.conn().await?)
            .await
            .map(Response::new)
            .map_err(Into::into)
    }

    async fn update(
        &self,
        req: Request<api::UpdateApiKeyRequest>,
    ) -> super::Resp<api::UpdateApiKeyResponse> {
        self.trx(|tx| update(req, tx).scope_boxed())
            .await
            .map(Response::new)
    }

    async fn regenerate(
        &self,
        req: Request<api::RegenerateApiKeyRequest>,
    ) -> super::Resp<api::RegenerateApiKeyResponse> {
        self.trx(|tx| regenerate(req, tx).scope_boxed())
            .await
            .map(Response::new)
    }

    async fn delete(
        &self,
        req: Request<api::DeleteApiKeyRequest>,
    ) -> super::Resp<api::DeleteApiKeyResponse> {
        self.trx(|tx| delete(req, tx).scope_boxed())
            .await
            .map(Response::new)
    }
}

async fn create(
    req: Request<api::CreateApiKeyRequest>,
    tx: &mut Conn,
) -> Result<api::CreateApiKeyResponse, Error> {
    let claims = tx.claims(&req, Endpoint::ApiKeyCreate).await?;

    let req = req.into_inner();
    let scope = req.scope.ok_or(Error::MissingCreateScope)?;

    let entry = ResourceEntry::try_from(scope)?;
    let ensure = claims.ensure_admin(entry.into(), tx).await?;
    let user_id = ensure.user().ok_or(Error::ClaimsNotUser)?.user_id();

    let created = NewApiKey::create(tx, user_id, req.label, entry).await?;

    Ok(api::CreateApiKeyResponse {
        api_key: Some(created.secret.into()),
        created_at: Some(NanosUtc::from(created.api_key.created_at).into()),
    })
}

async fn list(
    req: Request<api::ListApiKeyRequest>,
    conn: &mut Conn,
) -> Result<api::ListApiKeyResponse, Error> {
    let claims = conn.claims(&req, Endpoint::ApiKeyList).await?;
    let user_id = claims.resource().user().ok_or(Error::ClaimsNotUser)?;

    let keys = ApiKey::find_by_user(user_id, conn).await?;
    let api_keys = keys.into_iter().map(api::ListApiKey::from_model).collect();

    Ok(api::ListApiKeyResponse { api_keys })
}

async fn update(
    req: Request<api::UpdateApiKeyRequest>,
    tx: &mut Conn,
) -> Result<api::UpdateApiKeyResponse, Error> {
    let claims = tx.claims(&req, Endpoint::ApiKeyUpdate).await?;

    let req = req.into_inner();
    let key_id = req.id.parse().map_err(Error::ParseKeyId)?;

    let existing = ApiKey::find_by_id(key_id, tx).await?;
    let entry = ResourceEntry::from(&existing);
    let _ = claims.ensure_admin(entry.into(), tx).await?;

    let mut updated_at = None;

    if let Some(label) = req.label {
        updated_at = UpdateLabel::new(key_id, label).update(tx).await.map(Some)?;
    }

    if let Some(scope) = req.scope {
        let entry = ResourceEntry::try_from(scope)?;
        updated_at = UpdateScope::new(key_id, entry).update(tx).await.map(Some)?;
    }

    let updated_at = updated_at
        .ok_or(Error::NothingToUpdate)
        .map(NanosUtc::from)
        .map(Into::into)?;

    Ok(api::UpdateApiKeyResponse {
        updated_at: Some(updated_at),
    })
}

async fn regenerate(
    req: Request<api::RegenerateApiKeyRequest>,
    tx: &mut Conn,
) -> Result<api::RegenerateApiKeyResponse, Error> {
    let claims = tx.claims(&req, Endpoint::ApiKeyRegenerate).await?;

    let req = req.into_inner();
    let key_id = req.id.parse().map_err(Error::ParseKeyId)?;

    let existing = ApiKey::find_by_id(key_id, tx).await?;
    let entry = ResourceEntry::from(&existing);
    let _ = claims.ensure_admin(entry.into(), tx).await?;

    let new_key = NewApiKey::regenerate(key_id, tx).await?;
    let updated_at = new_key.api_key.updated_at.ok_or(Error::MissingUpdatedAt)?;

    Ok(api::RegenerateApiKeyResponse {
        api_key: Some(new_key.secret.into()),
        updated_at: Some(NanosUtc::from(updated_at).into()),
    })
}

async fn delete(
    req: Request<api::DeleteApiKeyRequest>,
    tx: &mut Conn,
) -> Result<api::DeleteApiKeyResponse, Error> {
    let claims = tx.claims(&req, Endpoint::ApiKeyDelete).await?;

    let req = req.into_inner();
    let key_id = req.id.parse().map_err(Error::ParseKeyId)?;

    let existing = ApiKey::find_by_id(key_id, tx).await?;
    let entry = ResourceEntry::from(&existing);
    let _ = claims.ensure_admin(entry.into(), tx).await?;

    ApiKey::delete(key_id, tx).await?;

    Ok(api::DeleteApiKeyResponse {})
}

impl api::ListApiKey {
    fn from_model(api_key: ApiKey) -> Self {
        let scope = api::ApiKeyScope::from_model(&api_key);

        api::ListApiKey {
            id: Some(format!("{}", *api_key.id)),
            label: Some(api_key.label),
            scope: Some(scope),
            created_at: Some(NanosUtc::from(api_key.created_at).into()),
            updated_at: api_key.updated_at.map(NanosUtc::from).map(Into::into),
        }
    }
}

impl api::ApiKeyScope {
    fn from_model(api_key: &ApiKey) -> Self {
        api::ApiKeyScope {
            resource: api_key.resource as i32,
            resource_id: Some(format!("{}", *api_key.resource_id)),
        }
    }

    #[cfg(any(test, feature = "integration-test"))]
    pub fn from_entry(entry: ResourceEntry) -> Self {
        api::ApiKeyScope {
            resource: ApiResource::from(entry.resource_type) as i32,
            resource_id: Some(format!("{}", *entry.resource_id)),
        }
    }
}

impl TryFrom<api::ApiKeyScope> for ResourceEntry {
    type Error = Error;

    fn try_from(scope: api::ApiKeyScope) -> Result<Self, Self::Error> {
        let api_resource =
            ApiResource::try_from(scope.resource).map_err(Error::ParseApiResource)?;
        let resource_type = api_resource.into();

        let resource_id = scope
            .resource_id
            .ok_or(Error::MissingScopeResourceId)?
            .parse()
            .map_err(Error::ParseResourceId)?;

        Ok(ResourceEntry {
            resource_type,
            resource_id,
        })
    }
}
