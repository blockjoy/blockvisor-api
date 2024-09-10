use blockvisor_api::auth::resource::{OrgId, UserId};
use blockvisor_api::grpc::api;
use blockvisor_api::model::{Org, SubscriptionId};

use crate::setup::helper::traits::{SocketRpc, SubscriptionService};
use crate::setup::TestServer;

#[tokio::test]
async fn subscription_service_api_requests() {
    let mut test = TestServer::new().await;
    let user_id = test.seed().user.id;
    let external_id = test.rand_string(8).await;

    let mut conn = test.conn().await;
    let org = Org::find_personal(user_id, &mut conn).await.unwrap();

    let created = create(&test, org.id, user_id, &external_id).await.unwrap();
    let sub = created.subscription.unwrap();
    assert_eq!(sub.org_id, org.id.to_string());
    assert_eq!(sub.user_id, user_id.to_string());
    assert_eq!(sub.external_id, external_id);

    let get_sub = get(&test, org.id).await.unwrap().subscription.unwrap();
    assert_eq!(get_sub, sub);

    let subs = list(&test, user_id).await.unwrap().subscriptions;
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].id, sub.id);

    assert!(update(&test, org.id).await.is_ok());

    let _ = delete(&test, sub.id.parse().unwrap()).await.unwrap();
    let subs = list(&test, user_id).await.unwrap().subscriptions;
    assert_eq!(subs.len(), 0);
}

async fn create(
    test: &TestServer,
    org_id: OrgId,
    user_id: UserId,
    external_id: &str,
) -> Result<api::SubscriptionServiceCreateResponse, tonic::Status> {
    let req = api::SubscriptionServiceCreateRequest {
        org_id: org_id.to_string(),
        user_id: user_id.to_string(),
        external_id: external_id.to_string(),
    };

    test.send_admin(SubscriptionService::create, req).await
}

async fn get(
    test: &TestServer,
    org_id: OrgId,
) -> Result<api::SubscriptionServiceGetResponse, tonic::Status> {
    let req = api::SubscriptionServiceGetRequest {
        org_id: org_id.to_string(),
    };
    test.send_admin(SubscriptionService::get, req).await
}

async fn list(
    test: &TestServer,
    user_id: UserId,
) -> Result<api::SubscriptionServiceListResponse, tonic::Status> {
    let req = api::SubscriptionServiceListRequest {
        user_id: Some(user_id.to_string()),
    };
    test.send_admin(SubscriptionService::list, req).await
}

async fn update(
    test: &TestServer,
    org_id: OrgId,
) -> Result<api::SubscriptionServiceUpdateResponse, tonic::Status> {
    let req = api::SubscriptionServiceUpdateRequest {
        org_id: Some(org_id.to_string()),
    };
    test.send_admin(SubscriptionService::update, req).await
}

async fn delete(
    test: &TestServer,
    id: SubscriptionId,
) -> Result<api::SubscriptionServiceDeleteResponse, tonic::Status> {
    let req = api::SubscriptionServiceDeleteRequest { id: id.to_string() };

    test.send_admin(SubscriptionService::delete, req).await
}
