use super::api::{self, org_service_server};
use super::helpers;
use crate::auth::{FindableById, UserAuthToken};
use crate::models;
use diesel_async::scoped_futures::ScopedFutureExt;
use diesel_async::AsyncPgConnection;
use std::collections::HashMap;
use tonic::{Request, Response, Status};

#[tonic::async_trait]
impl org_service_server::OrgService for super::GrpcImpl {
    async fn get(
        &self,
        request: Request<api::OrgServiceGetRequest>,
    ) -> super::Result<api::OrgServiceGetResponse> {
        let refresh_token = super::get_refresh_token(&request);
        let request = request.into_inner();
        let mut conn = self.db.conn().await?;
        let org_id = request.id.parse().map_err(crate::Error::from)?;
        let org = models::Org::find_by_id(org_id, &mut conn).await?;
        let org = api::Org::from_model(org, &mut conn).await?;
        let resp = api::OrgServiceGetResponse { org: Some(org) };
        super::response_with_refresh_token(refresh_token, resp)
    }

    async fn list(
        &self,
        request: Request<api::OrgServiceListRequest>,
    ) -> super::Result<api::OrgServiceListResponse> {
        let refresh_token = super::get_refresh_token(&request);
        let request = request.into_inner();

        let mut conn = self.db.conn().await?;
        let member_id = request
            .member_id
            .map(|id| id.parse())
            .transpose()
            .map_err(crate::Error::from)?;
        let orgs = models::Org::filter(member_id, &mut conn).await?;
        let orgs = api::Org::from_models(orgs, &mut conn).await?;
        let resp = api::OrgServiceListResponse { orgs };
        super::response_with_refresh_token(refresh_token, resp)
    }

    async fn create(
        &self,
        request: Request<api::OrgServiceCreateRequest>,
    ) -> super::Result<api::OrgServiceCreateResponse> {
        let refresh_token = super::get_refresh_token(&request);
        let user_id = helpers::try_get_token::<_, UserAuthToken>(&request)?.id;
        let inner = request.into_inner();
        let new_org = models::NewOrg {
            name: &inner.name,
            is_personal: false,
        };
        self.trx(|c| {
            async move {
                let user = models::User::find_by_id(user_id, c).await?;
                let org = new_org.create(user.id, c).await?;
                let msg = api::OrgMessage::created(org.clone(), user, c).await?;
                let org = api::Org::from_model(org, c).await?;
                self.notifier.orgs_sender().send(&msg).await?;
                let resp = api::OrgServiceCreateResponse { org: Some(org) };
                Ok(super::response_with_refresh_token(refresh_token, resp)?)
            }
            .scope_boxed()
        })
        .await
    }

    async fn update(
        &self,
        request: Request<api::OrgServiceUpdateRequest>,
    ) -> super::Result<api::OrgServiceUpdateResponse> {
        let refresh_token = super::get_refresh_token(&request);
        let token = helpers::try_get_token::<_, UserAuthToken>(&request)?;
        let user_id = token.id;
        let inner = request.into_inner();
        let org_id = inner.id.parse().map_err(crate::Error::from)?;
        let update = models::UpdateOrg {
            id: org_id,
            name: inner.name.as_deref(),
        };

        self.trx(|c| {
            async move {
                let org = update.update(c).await?;
                let user = models::User::find_by_id(user_id, c).await?;
                let msg = api::OrgMessage::updated(org, user, c).await?;
                self.notifier.orgs_sender().send(&msg).await?;
                let resp = api::OrgServiceUpdateResponse {};
                Ok(super::response_with_refresh_token(refresh_token, resp)?)
            }
            .scope_boxed()
        })
        .await
    }

    async fn delete(
        &self,
        request: Request<api::OrgServiceDeleteRequest>,
    ) -> super::Result<api::OrgServiceDeleteResponse> {
        use models::OrgRole::*;

        let refresh_token = super::get_refresh_token(&request);
        let token = helpers::try_get_token::<_, UserAuthToken>(&request)?;
        let user_id = token.id;
        let inner = request.into_inner();
        self.trx(|c| {
            async move {
                let org_id = inner.id.parse()?;
                let org = models::Org::find_by_id(org_id, c).await?;
                if org.is_personal {
                    super::bail_unauthorized!("Can't deleted personal org");
                }
                let member = models::Org::find_org_user(user_id, org_id, c).await?;

                // Only owner or admins may delete orgs
                let is_allowed = match member.role {
                    Member => false,
                    Owner | Admin => true,
                };
                if !is_allowed {
                    super::bail_unauthorized!(
                        "User {user_id} has insufficient privileges to delete org {org_id}"
                    );
                }
                tracing::debug!("Deleting org: {}", org_id);
                models::Org::delete(org_id, c).await?;
                let user = models::User::find_by_id(user_id, c).await?;
                let msg = api::OrgMessage::deleted(org, user);
                self.notifier.orgs_sender().send(&msg).await?;
                let resp = api::OrgServiceDeleteResponse {};
                Ok(super::response_with_refresh_token(refresh_token, resp)?)
            }
            .scope_boxed()
        })
        .await
    }

    async fn remove_member(
        &self,
        request: Request<api::OrgServiceRemoveMemberRequest>,
    ) -> Result<Response<api::OrgServiceRemoveMemberResponse>, Status> {
        use models::OrgRole::*;

        let refresh_token = super::get_refresh_token(&request);
        let caller_id = helpers::try_get_token::<_, UserAuthToken>(&request)?.id;
        let inner = request.into_inner();
        self.trx(|c| {
            async move {
                let user_id = inner.user_id.parse()?;
                let org_id = inner.org_id.parse()?;
                let member = models::Org::find_org_user(caller_id, org_id, c).await?;
                let is_allowed = match member.role {
                    // Members can only remove themselves
                    Member => caller_id == user_id,
                    Owner | Admin => true,
                };
                if !is_allowed {
                    super::bail_unauthorized!(
                        "User {caller_id} has insufficient privileges to remove other user \
                            {user_id} from org {org_id}"
                    )
                }
                let user_to_remove = models::User::find_by_id(user_id, c).await?;
                models::Org::remove_org_user(user_id, org_id, c).await?;
                // In case a user needs to be re-invited later, we also remove the (already
                // accepted) invites from the database. This is to prevent them from running
                // into a unique constraint when they are invited again.
                models::Invitation::remove_by_org_user(&user_to_remove.email, org_id, c).await?;
                let org = models::Org::find_by_id(org_id, c).await?;
                let user = models::User::find_by_id(caller_id, c).await?;
                let msg = api::OrgMessage::updated(org, user, c).await?;
                self.notifier.orgs_sender().send(&msg).await?;
                let resp = api::OrgServiceRemoveMemberResponse {};
                Ok(super::response_with_refresh_token(refresh_token, resp)?)
            }
            .scope_boxed()
        })
        .await
    }
}

impl api::Org {
    /// Converts a list of `models::Org` into a list of `api::Org`. We take care to perform O(1)
    /// queries, no matter the length of `models`. For this we need to find all users belonging to
    /// this each org.
    pub async fn from_models(
        models: Vec<models::Org>,
        conn: &mut AsyncPgConnection,
    ) -> crate::Result<Vec<Self>> {
        // We find all OrgUsers belonging to each model. This gives us a map from `org_id` to
        // `Vec<OrgUser>`.
        let org_users = models::OrgUser::by_orgs(&models, conn).await?;

        // Now we get the actual users for each `OrgUser`, because we also need to provide the name
        // and email of each user.
        let user_ids: Vec<uuid::Uuid> = org_users.values().flatten().map(|ou| ou.user_id).collect();
        let users: HashMap<uuid::Uuid, models::User> = models::User::find_by_ids(&user_ids, conn)
            .await?
            .into_iter()
            .map(|u| (u.id, u))
            .collect();

        // Finally we can loop over the models to construct the final list of messages we set out to
        // create.
        models
            .into_iter()
            .map(|model| {
                let empty = vec![];
                let org_users = org_users.get(&model.id).unwrap_or(&empty);
                Ok(Self {
                    id: model.id.to_string(),
                    name: model.name.clone(),
                    personal: model.is_personal,
                    member_count: org_users.len().try_into()?,
                    created_at: Some(super::try_dt_to_ts(model.created_at)?),
                    updated_at: Some(super::try_dt_to_ts(model.updated_at)?),
                    members: org_users
                        .iter()
                        .map(|ou| {
                            let user = &users[&ou.user_id];
                            let mut org = api::OrgUser {
                                user_id: ou.user_id.to_string(),
                                org_id: ou.org_id.to_string(),
                                role: 0, // We use the setter to set this field for type-safety
                                name: user.name(),
                                email: user.email.clone(),
                            };
                            org.set_role(api::OrgRole::from_model(ou.role));
                            org
                        })
                        .collect(),
                })
            })
            .collect()
    }

    pub async fn from_model(
        model: models::Org,
        conn: &mut AsyncPgConnection,
    ) -> crate::Result<Self> {
        Ok(Self::from_models(vec![model], conn).await?[0].clone())
    }
}

impl api::OrgRole {
    fn from_model(model: models::OrgRole) -> Self {
        match model {
            models::OrgRole::Admin => Self::Admin,
            models::OrgRole::Owner => Self::Owner,
            models::OrgRole::Member => Self::Member,
        }
    }
}
