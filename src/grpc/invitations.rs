use diesel_async::scoped_futures::ScopedFutureExt;
use tonic::{Request, Status};

use super::api::{self, invitation_service_server};
use super::helpers::required;
use crate::auth::token::{Endpoint, Resource};
use crate::mail::{self, MailClient};
use crate::{auth, models};

#[tonic::async_trait]
impl invitation_service_server::InvitationService for super::GrpcImpl {
    async fn create(
        &self,
        req: Request<api::InvitationServiceCreateRequest>,
    ) -> super::Resp<api::InvitationServiceCreateResponse> {
        self.trx(|c| create(req, c).scope_boxed())
            .await?
            .into_resp(&self.notifier)
            .await
    }

    async fn list(
        &self,
        req: Request<api::InvitationServiceListRequest>,
    ) -> super::Resp<api::InvitationServiceListResponse> {
        let mut conn = self.conn().await?;
        let resp = list(req, &mut conn).await?;
        Ok(resp)
    }

    async fn accept(
        &self,
        req: Request<api::InvitationServiceAcceptRequest>,
    ) -> super::Resp<api::InvitationServiceAcceptResponse> {
        self.trx(|c| accept(req, c).scope_boxed())
            .await?
            .into_resp(&self.notifier)
            .await
    }

    async fn decline(
        &self,
        req: Request<api::InvitationServiceDeclineRequest>,
    ) -> super::Resp<api::InvitationServiceDeclineResponse> {
        self.trx(|c| decline(req, c).scope_boxed())
            .await?
            .into_resp(&self.notifier)
            .await
    }

    async fn revoke(
        &self,
        req: Request<api::InvitationServiceRevokeRequest>,
    ) -> super::Resp<api::InvitationServiceRevokeResponse> {
        self.trx(|c| revoke(req, c).scope_boxed())
            .await?
            .into_resp(&self.notifier)
            .await
    }
}

async fn create(
    req: Request<api::InvitationServiceCreateRequest>,
    conn: &mut models::Conn,
) -> crate::Result<super::Outcome<api::InvitationServiceCreateResponse>> {
    let claims = auth::get_claims(&req, Endpoint::InvitationCreate, conn).await?;
    let req = req.into_inner();
    // In principle, it is allowed for resources other than a user to create an invite, but we
    // currently include a field `created_by_user` with a created invite.
    let (is_allowed, caller) = match claims.resource() {
        Resource::User(user_id) => {
            let caller = models::User::find_by_id(user_id, conn).await?;
            let is_allowed = models::Org::is_admin(caller.id, req.org_id.parse()?, conn).await?;
            (is_allowed, caller)
        }
        Resource::Org(_) => todo!(),
        Resource::Host(_) => todo!(),
        Resource::Node(_) => todo!(),
    };
    if !is_allowed {
        super::forbidden!("Access denied");
    }

    let org_id = req.org_id.parse()?;
    if models::Invitation::has_open_invite(org_id, &req.invitee_email, conn).await? {
        super::forbidden!("User is already invited");
    }

    // Check if the user-to-invite is not already a member of the organization
    let invited_user = models::User::find_by_email(&req.invitee_email, conn).await;
    if let Ok(invited_user) = &invited_user {
        if models::Org::is_member(invited_user.id, org_id, conn).await? {
            super::forbidden!("Already a member");
        }
    }

    let invitation = req.into_new(caller.id, conn).await?.create(conn).await?;

    match invited_user {
        Ok(user) => {
            // Note that here we abort the transaction if sending the email failed. This way we do
            // not get invites in the db that we cannot send emails to. The existence of such an
            // invite would prevent them from trying to recreate again at a later point.
            MailClient::new(&conn.context.config)
                .invitation_for_registered(&caller, &user, "1 week")
                .await?;
        }
        Err(_) => {
            let invitee = mail::Recipient {
                email: &invitation.invitee_email,
                first_name: "",
                last_name: "",
                preferred_language: None,
            };

            MailClient::new(&conn.context.config)
                .invitation(
                    &invitation,
                    &caller,
                    invitee,
                    "1 week",
                    &conn.context.cipher,
                )
                .await?;
        }
    }

    let org = models::Org::find_by_id(invitation.created_for_org, conn).await?;
    let msg = api::OrgMessage::invitation_created(org, invitation)?;
    let resp = api::InvitationServiceCreateResponse {};
    Ok(super::Outcome::new(resp).with_msg(msg))
}

async fn list(
    req: Request<api::InvitationServiceListRequest>,
    conn: &mut models::Conn,
) -> super::Result<api::InvitationServiceListResponse> {
    let claims = auth::get_claims(&req, Endpoint::InvitationCreate, conn).await?;
    let req = req.into_inner();

    let parse = |s: &str| s.parse::<uuid::Uuid>();
    let is_allowed = match claims.resource() {
        Resource::User(user_id) => {
            if let Some(org_id) = &req.org_id {
                models::Org::is_member(user_id, org_id.parse()?, conn).await?
            } else if let Some(created_by) = &req.created_by {
                parse(created_by)? == user_id
            } else if let Some(invitee_email) = &req.invitee_email {
                let user = models::User::find_by_email(invitee_email, conn).await?;
                user.id == user_id
            } else {
                false
            }
        }
        Resource::Org(org_id) => {
            if let Some(org) = &req.org_id {
                parse(org)? == org_id
            } else {
                false
            }
        }
        Resource::Host(_) => false,
        Resource::Node(_) => false,
    };
    if !is_allowed {
        super::forbidden!("Access denied");
    }
    let filter = req.as_filter()?;
    let invitations = models::Invitation::filter(filter, conn).await?;
    let invitations = api::Invitation::from_models(invitations)?;
    let resp = api::InvitationServiceListResponse { invitations };
    Ok(tonic::Response::new(resp))
}

async fn accept(
    req: Request<api::InvitationServiceAcceptRequest>,
    conn: &mut models::Conn,
) -> crate::Result<super::Outcome<api::InvitationServiceAcceptResponse>> {
    let claims = auth::get_claims(&req, Endpoint::InvitationAccept, conn).await?;
    let req = req.into_inner();
    let invitation = models::Invitation::find_by_id(req.invitation_id.parse()?, conn).await?;
    let is_allowed = match claims.resource() {
        Resource::User(user_id) => {
            let user = models::User::find_by_id(user_id, conn).await?;
            invitation.invitee_email == user.email
        }
        Resource::Org(org_id) => {
            let email = claims.data.get("email").ok_or_else(required("email"))?;
            let user = models::User::find_by_email(email, conn).await?;
            invitation.created_for_org == org_id && invitation.invitee_email == user.email
        }
        Resource::Host(_) => false,
        Resource::Node(_) => false,
    };
    if !is_allowed {
        super::forbidden!("Access denied");
    }
    if invitation.accepted_at.is_some() {
        return Err(Status::failed_precondition("Invitation already accepted").into());
    }
    if invitation.declined_at.is_some() {
        return Err(Status::failed_precondition("Invitation is declined").into());
    }

    let invitation = invitation.accept(conn).await?;
    let org = models::Org::find_by_id(invitation.created_for_org, conn).await?;
    // Only registered users can accept an invitation
    let new_member = models::User::find_by_email(&invitation.invitee_email, conn).await?;
    let org_user = org
        .add_member(new_member.id, models::OrgRole::Member, conn)
        .await?;
    let org = models::Org::find_by_id(org_user.org_id, conn).await?;
    let user = models::User::find_by_id(org_user.user_id, conn).await?;
    let msg = api::OrgMessage::invitation_accepted(org, invitation, user)?;
    let resp = api::InvitationServiceAcceptResponse {};
    Ok(super::Outcome::new(resp).with_msg(msg))
}

async fn decline(
    req: Request<api::InvitationServiceDeclineRequest>,
    conn: &mut models::Conn,
) -> crate::Result<super::Outcome<api::InvitationServiceDeclineResponse>> {
    let claims = auth::get_claims(&req, Endpoint::InvitationDecline, conn).await?;
    let req = req.into_inner();
    let invitation = models::Invitation::find_by_id(req.invitation_id.parse()?, conn).await?;
    let is_allowed = match claims.resource() {
        Resource::User(user_id) => {
            let user = models::User::find_by_id(user_id, conn).await?;
            invitation.invitee_email == user.email
        }
        Resource::Org(org_id) => {
            let email = claims.data.get("email").ok_or_else(required("email"))?;
            invitation.created_for_org == org_id && invitation.invitee_email == *email
        }
        Resource::Host(_) => false,
        Resource::Node(_) => false,
    };
    if !is_allowed {
        super::forbidden!("Access denied");
    }
    if invitation.accepted_at.is_some() {
        return Err(Status::failed_precondition("Invite is accepted").into());
    }
    if invitation.declined_at.is_some() {
        return Err(Status::failed_precondition("Invite already declined").into());
    }

    invitation.decline(conn).await?;
    let org = models::Org::find_by_id(invitation.created_for_org, conn).await?;
    let msg = api::OrgMessage::invitation_declined(org, invitation)?;
    let resp = api::InvitationServiceDeclineResponse {};
    Ok(super::Outcome::new(resp).with_msg(msg))
}

async fn revoke(
    req: Request<api::InvitationServiceRevokeRequest>,
    conn: &mut models::Conn,
) -> crate::Result<super::Outcome<api::InvitationServiceRevokeResponse>> {
    let claims = auth::get_claims(&req, Endpoint::InvitationRevoke, conn).await?;
    let req = req.into_inner();
    let invitation = models::Invitation::find_by_id(req.invitation_id.parse()?, conn).await?;
    let is_allowed = match claims.resource() {
        Resource::User(user_id) => {
            models::Org::is_member(user_id, invitation.created_for_org, conn).await?
        }
        Resource::Org(_) => false,
        Resource::Host(_) => false,
        Resource::Node(_) => false,
    };
    if !is_allowed {
        super::forbidden!("Access denied");
    }
    if invitation.accepted_at.is_some() {
        return Err(Status::failed_precondition("Invite is accepted").into());
    }
    if invitation.declined_at.is_some() {
        return Err(Status::failed_precondition("Invite is declined").into());
    }
    invitation.revoke(conn).await?;
    let org = models::Org::find_by_id(invitation.created_for_org, conn).await?;
    let msg = api::OrgMessage::invitation_declined(org, invitation)?;
    let resp = api::InvitationServiceRevokeResponse {};
    Ok(super::Outcome::new(resp).with_msg(msg))
}

impl api::InvitationServiceCreateRequest {
    pub async fn into_new(
        self,
        created_by_user: uuid::Uuid,
        conn: &mut models::Conn,
    ) -> crate::Result<models::NewInvitation> {
        let creator = models::User::find_by_id(created_by_user, conn).await?;
        let org_id = self.org_id.parse()?;
        let for_org = models::Org::find_by_id(org_id, conn).await?;

        let name = format!(
            "{} {} ({})",
            creator.first_name, creator.last_name, creator.email
        );
        Ok(models::NewInvitation {
            created_by_user,
            created_by_user_name: name,
            created_for_org: for_org.id,
            created_for_org_name: for_org.name,
            invitee_email: self.invitee_email,
        })
    }
}

impl api::Invitation {
    fn from_models(models: Vec<models::Invitation>) -> crate::Result<Vec<Self>> {
        models.into_iter().map(Self::from_model).collect()
    }

    pub fn from_model(model: models::Invitation) -> crate::Result<Self> {
        let mut invitation = Self {
            id: model.id.to_string(),
            created_by: model.created_by_user.to_string(),
            created_by_name: model.created_by_user_name,
            org_id: model.created_for_org.to_string(),
            org_name: model.created_for_org_name,
            invitee_email: model.invitee_email,
            created_at: Some(super::try_dt_to_ts(model.created_at)?),
            status: 0, // We use the setter to set this field for type-safety
            accepted_at: model.accepted_at.map(super::try_dt_to_ts).transpose()?,
            declined_at: model.declined_at.map(super::try_dt_to_ts).transpose()?,
        };
        let status = match (model.accepted_at, model.declined_at) {
            (None, None) => api::InvitationStatus::Open,
            (Some(_), None) => api::InvitationStatus::Accepted,
            (None, Some(_)) => api::InvitationStatus::Declined,
            (Some(_), Some(_)) => api::InvitationStatus::Unspecified,
        };
        invitation.set_status(status);
        Ok(invitation)
    }
}

impl api::InvitationServiceListRequest {
    fn as_filter(&self) -> crate::Result<models::InvitationFilter> {
        let status = self.status();
        let status = (status != api::InvitationStatus::Unspecified).then_some(status);
        Ok(models::InvitationFilter {
            org_id: self.org_id.as_ref().map(|id| id.parse()).transpose()?,
            invitee_email: self.invitee_email.as_deref(),
            created_by: self.created_by.as_ref().map(|id| id.parse()).transpose()?,
            accepted: status.map(|s| s == api::InvitationStatus::Accepted),
            declined: status.map(|s| s == api::InvitationStatus::Declined),
        })
    }
}
