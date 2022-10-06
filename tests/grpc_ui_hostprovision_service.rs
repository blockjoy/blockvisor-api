#[allow(dead_code)]
mod setup;

use api::auth::TokenIdentifyable;
use api::grpc::blockjoy_ui::host_provision_service_client::HostProvisionServiceClient;
use api::grpc::blockjoy_ui::{
    CreateHostProvisionRequest, GetHostProvisionRequest, HostProvision as GrpcHostProvision,
    RequestMeta,
};
use api::models::{HostProvision, HostProvisionRequest};
use setup::setup;
use sqlx::postgres::PgRow;
use sqlx::Row;
use std::sync::Arc;
use test_macros::*;
use tonic::transport::Channel;
use tonic::{Request, Status};
use uuid::Uuid;

#[before(call = "setup")]
#[tokio::test]
async fn responds_not_found_without_valid_id_for_get() {
    let db = Arc::new(_before_values.await);
    let request_meta = RequestMeta {
        id: Some(Uuid::new_v4().to_string()),
        token: None,
        fields: vec![],
        pagination: None,
    };
    let user = db.admin_user().await;
    let token = user.get_token(&db.pool).await.unwrap();
    let inner = GetHostProvisionRequest {
        meta: Some(request_meta),
        id: Some("foo-bar1".to_string()),
    };
    let mut request = Request::new(inner);

    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", token.to_base64()).parse().unwrap(),
    );

    assert_grpc_request! { get, request, tonic::Code::NotFound, db, HostProvisionServiceClient<Channel> };
}

#[before(call = "setup")]
#[tokio::test]
async fn responds_ok_with_valid_id_for_get() {
    let db = Arc::new(_before_values.await);
    let request_meta = RequestMeta {
        id: Some(Uuid::new_v4().to_string()),
        token: None,
        fields: vec![],
        pagination: None,
    };
    let user = db.admin_user().await;
    let mut tx = db.pool.begin().await.unwrap();
    let org_id = sqlx::query("select org_id from orgs_users where user_id = $1 limit 1")
        .bind(user.id)
        .fetch_one(&mut tx)
        .await
        .map(PgRow::from)
        .unwrap();
    tx.commit().await.unwrap();

    let token = user.get_token(&db.pool).await.unwrap();
    let req = HostProvisionRequest {
        org_id: org_id.get::<Uuid, usize>(0),
        nodes: None,
    };
    let provision = HostProvision::create(req, &db.pool).await.unwrap();

    let inner = GetHostProvisionRequest {
        meta: Some(request_meta),
        id: Some(provision.id),
    };
    let mut request = Request::new(inner);

    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", token.to_base64()).parse().unwrap(),
    );

    assert_grpc_request! { get, request, tonic::Code::Ok, db, HostProvisionServiceClient<Channel> };
}

#[before(call = "setup")]
#[tokio::test]
async fn responds_error_with_invalid_provision_for_create() {
    let db = Arc::new(_before_values.await);
    let request_meta = RequestMeta {
        id: Some(Uuid::new_v4().to_string()),
        token: None,
        fields: vec![],
        pagination: None,
    };
    let user = db.admin_user().await;
    let token = user.get_token(&db.pool).await.unwrap();
    let inner = CreateHostProvisionRequest {
        meta: Some(request_meta),
        host_provision: None,
    };
    let mut request = Request::new(inner);

    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", token.to_base64()).parse().unwrap(),
    );

    assert_grpc_request! { create, request, tonic::Code::InvalidArgument, db, HostProvisionServiceClient<Channel> };
}

#[before(call = "setup")]
#[tokio::test]
async fn responds_ok_with_valid_provision_for_create() {
    let db = Arc::new(_before_values.await);
    let request_meta = RequestMeta {
        id: Some(Uuid::new_v4().to_string()),
        token: None,
        fields: vec![],
        pagination: None,
    };
    let user = db.admin_user().await;
    let mut tx = db.pool.begin().await.unwrap();
    let org_id = sqlx::query("select org_id from orgs_users where user_id = $1 limit 1")
        .bind(user.id)
        .fetch_one(&mut tx)
        .await
        .map(PgRow::from)
        .unwrap();
    tx.commit().await.unwrap();

    let token = user.get_token(&db.pool).await.unwrap();
    let provision = GrpcHostProvision {
        org_id: org_id.get::<Uuid, usize>(0).to_string(),
        ..Default::default()
    };
    let inner = CreateHostProvisionRequest {
        meta: Some(request_meta),
        host_provision: Some(provision),
    };
    let mut request = Request::new(inner);

    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", token.to_base64()).parse().unwrap(),
    );

    assert_grpc_request! { create, request, tonic::Code::Ok, db, HostProvisionServiceClient<Channel> };
}
