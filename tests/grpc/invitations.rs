use blockvisor_api::auth::{self, FindableById};
use blockvisor_api::grpc::api;
use blockvisor_api::models;

type Service = api::invitations_client::InvitationsClient<super::Channel>;

async fn create_invitation(tester: &super::Tester) -> anyhow::Result<models::Invitation> {
    let user = tester.admin_user().await;
    let org = tester.org().await;
    let new_invitation = models::NewInvitation {
        created_by_user: user.id,
        created_by_user_name: user.last_name,
        created_for_org: org.id,
        created_for_org_name: org.name,
        invitee_email: "test@here.com",
    };
    let mut conn = tester.conn().await;
    let inv = new_invitation.create(&mut conn).await.unwrap();
    Ok(inv)
}

#[tokio::test]
async fn responds_ok_for_create() {
    let tester = super::Tester::new().await;
    let org_id = tester.org().await.id;
    let req = api::CreateInvitationRequest {
        invitee_email: "hugo@boss.com".to_string(),
        created_for_org_id: org_id.to_string(),
    };

    tester.send_admin(Service::create, req).await.unwrap();

    let mut conn = tester.conn().await;
    let cnt = models::Invitation::received("hugo@boss.com", &mut conn)
        .await
        .unwrap()
        .len();

    assert_eq!(cnt, 1);
}

#[tokio::test]
async fn responds_ok_for_list_pending() {
    let tester = super::Tester::new().await;
    let invitation = create_invitation(&tester).await.unwrap();
    let user = tester.admin_user().await;
    let org = tester.org_for(&user).await;
    let req = api::ListPendingInvitationRequest {
        org_id: org.id.to_string(),
    };

    tester.send_admin(Service::list_pending, req).await.unwrap();

    let mut conn = tester.conn().await;
    let invitations = models::Invitation::received(&invitation.invitee_email, &mut conn)
        .await
        .unwrap();

    assert_eq!(invitations.len(), 1);
}

#[tokio::test]
async fn responds_ok_for_list_received() {
    let tester = super::Tester::new().await;
    let invitation = create_invitation(&tester).await.unwrap();
    let user = tester.admin_user().await;
    let req = api::ListReceivedInvitationRequest {
        user_id: user.id.to_string(),
    };

    tester
        .send_admin(Service::list_received, req)
        .await
        .unwrap();
    let mut conn = tester.conn().await;
    let invitations = models::Invitation::received(&invitation.invitee_email, &mut conn)
        .await
        .unwrap();

    assert_eq!(invitations.len(), 1);
}

#[tokio::test]
async fn responds_ok_for_accept() {
    let tester = super::Tester::new().await;
    let invitation = create_invitation(&tester).await.unwrap();
    let token = auth::InvitationToken::create_for_invitation(&invitation).unwrap();
    let grpc_invitation = api::Invitation {
        id: invitation.id.to_string(),
        ..Default::default()
    };
    let req = api::InvitationRequest {
        invitation: Some(grpc_invitation),
    };

    tester
        .send_with(Service::accept, req, token, super::DummyRefresh)
        .await
        .unwrap();
}

#[tokio::test]
async fn responds_ok_for_decline() {
    let tester = super::Tester::new().await;
    let invitation = create_invitation(&tester).await.unwrap();
    let token = auth::InvitationToken::create_for_invitation(&invitation).unwrap();

    let grpc_invitation = api::Invitation {
        id: invitation.id.to_string(),
        created_by_id: invitation.created_by_user.to_string(),
        created_for_org_id: invitation.created_for_org.to_string(),
        invitee_email: invitation.invitee_email.to_string(),
        created_at: None,
        accepted_at: None,
        declined_at: None,
        created_by_user_name: "hugo".to_string(),
        created_for_org_name: "boss".to_string(),
    };
    let req = api::InvitationRequest {
        invitation: Some(grpc_invitation),
    };

    tester
        .send_with(Service::decline, req, token, super::DummyRefresh)
        .await
        .unwrap();
}

#[tokio::test]
async fn responds_ok_for_revoke() {
    let tester = super::Tester::new().await;
    let invitation = create_invitation(&tester).await.unwrap();
    let user = tester.admin_user().await;
    let mut conn = tester.conn().await;
    let org = models::Org::find_by_id(invitation.created_for_org, &mut conn)
        .await
        .unwrap();
    // If the user is already added, thats okay
    let _ = models::Org::add_member(user.id, org.id, models::OrgRole::Admin, &mut conn).await;
    let grpc_invitation = api::Invitation {
        id: invitation.id.to_string(),
        created_by_id: invitation.created_by_user.to_string(),
        created_for_org_id: invitation.created_for_org.to_string(),
        invitee_email: invitation.invitee_email.to_string(),
        created_at: None,
        accepted_at: None,
        declined_at: None,
        created_by_user_name: "hugo".to_string(),
        created_for_org_name: "boss".to_string(),
    };
    let req = api::InvitationRequest {
        invitation: Some(grpc_invitation),
    };

    tester.send_admin(Service::revoke, req).await.unwrap();
}
