use super::api::{self, user_service_server};
use crate::auth;
use crate::auth::expiration_provider;
use crate::mail;
use crate::models;
use diesel_async::scoped_futures::ScopedFutureExt;

#[tonic::async_trait]
impl user_service_server::UserService for super::GrpcImpl {
    async fn create(
        &self,
        req: tonic::Request<api::UserServiceCreateRequest>,
    ) -> super::Resp<api::UserServiceCreateResponse> {
        self.trx(|c| create(req, c).scope_boxed()).await
    }

    async fn get(
        &self,
        req: tonic::Request<api::UserServiceGetRequest>,
    ) -> super::Resp<api::UserServiceGetResponse> {
        let mut conn = self.conn().await?;
        let resp = get(req, &mut conn).await?;
        Ok(resp)
    }

    async fn update(
        &self,
        req: tonic::Request<api::UserServiceUpdateRequest>,
    ) -> super::Resp<api::UserServiceUpdateResponse> {
        self.trx(|c| update(req, c).scope_boxed()).await
    }

    async fn delete(
        &self,
        req: tonic::Request<api::UserServiceDeleteRequest>,
    ) -> super::Resp<api::UserServiceDeleteResponse> {
        self.trx(|c| delete(req, c).scope_boxed()).await
    }
}

async fn get(
    req: tonic::Request<api::UserServiceGetRequest>,
    conn: &mut diesel_async::AsyncPgConnection,
) -> super::Result<api::UserServiceGetResponse> {
    let claims = auth::get_claims(&req, auth::Endpoint::UserGet, conn).await?;
    let req = req.into_inner();
    let user = models::User::find_by_id(req.id.parse()?, conn).await?;
    let is_allowed = match claims.resource() {
        auth::Resource::User(user_id) => user_id == user.id,
        auth::Resource::Org(_) => false,
        auth::Resource::Host(_) => false,
        auth::Resource::Node(_) => false,
    };
    if !is_allowed {
        super::forbidden!("Access not allowed")
    }
    let resp = api::UserServiceGetResponse {
        user: Some(api::User::from_model(user)?),
    };
    let mut resp = tonic::Response::new(resp);
    let auth::Resource::User(user_id) = claims.resource() else { panic!("Not user")  };
    let iat = chrono::Utc::now();
    let refresh_exp =
        expiration_provider::ExpirationProvider::expiration(auth::REFRESH_EXPIRATION_USER_MINS)?;
    let refresh = auth::Refresh::new(user_id, iat, refresh_exp)?;
    let refresh = refresh.as_set_cookie()?;
    resp.metadata_mut().insert("set-cookie", refresh.parse()?);

    Ok(resp)
}

async fn create(
    req: tonic::Request<api::UserServiceCreateRequest>,
    conn: &mut diesel_async::AsyncPgConnection,
) -> super::Result<api::UserServiceCreateResponse> {
    // This endpoint doesn't require authentication.
    let inner = req.into_inner();
    let new_user = inner.as_new()?;
    let new_user = new_user.create(conn).await?;
    mail::MailClient::new()
        .registration_confirmation(&new_user)
        .await?;
    let resp = api::UserServiceCreateResponse {
        user: Some(api::User::from_model(new_user.clone())?),
    };
    Ok(tonic::Response::new(resp))
}

async fn update(
    req: tonic::Request<api::UserServiceUpdateRequest>,
    conn: &mut diesel_async::AsyncPgConnection,
) -> super::Result<api::UserServiceUpdateResponse> {
    let claims = auth::get_claims(&req, auth::Endpoint::UserUpdate, conn).await?;
    let req = req.into_inner();
    let user = models::User::find_by_id(req.id.parse()?, conn).await?;
    let is_allowed = match claims.resource() {
        auth::Resource::User(user_id) => user_id == user.id,
        auth::Resource::Org(_) => false,
        auth::Resource::Host(_) => false,
        auth::Resource::Node(_) => false,
    };
    if !is_allowed {
        super::forbidden!("Access not allowed")
    }
    let user = req.as_update()?.update(conn).await?;
    let resp = api::UserServiceUpdateResponse {
        user: Some(api::User::from_model(user)?),
    };
    Ok(tonic::Response::new(resp))
}

async fn delete(
    req: tonic::Request<api::UserServiceDeleteRequest>,
    conn: &mut diesel_async::AsyncPgConnection,
) -> super::Result<api::UserServiceDeleteResponse> {
    let claims = auth::get_claims(&req, auth::Endpoint::UserUpdate, conn).await?;
    let req = req.into_inner();
    let user = models::User::find_by_id(req.id.parse()?, conn).await?;
    let is_allowed = match claims.resource() {
        auth::Resource::User(user_id) => user_id == user.id,
        auth::Resource::Org(_) => false,
        auth::Resource::Host(_) => false,
        auth::Resource::Node(_) => false,
    };
    if !is_allowed {
        super::forbidden!("Access not allowed")
    }
    models::User::delete(user.id, conn).await?;
    let resp = api::UserServiceDeleteResponse {};
    Ok(tonic::Response::new(resp))
}

impl api::User {
    pub fn from_model(model: models::User) -> crate::Result<Self> {
        let user = Self {
            id: model.id.to_string(),
            email: model.email,
            first_name: model.first_name,
            last_name: model.last_name,
            created_at: Some(super::try_dt_to_ts(model.created_at)?),
            updated_at: None,
        };
        Ok(user)
    }
}

impl api::UserServiceCreateRequest {
    fn as_new(&self) -> crate::Result<models::NewUser> {
        models::NewUser::new(
            &self.email,
            &self.first_name,
            &self.last_name,
            &self.password,
        )
    }
}

impl api::UserServiceUpdateRequest {
    pub fn as_update(&self) -> crate::Result<models::UpdateUser<'_>> {
        Ok(models::UpdateUser {
            id: self.id.parse()?,
            first_name: self.first_name.as_deref(),
            last_name: self.last_name.as_deref(),
        })
    }
}
