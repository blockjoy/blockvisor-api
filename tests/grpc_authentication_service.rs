#[allow(dead_code)]
mod setup;

use api::auth::TokenIdentifyable;
use api::grpc::blockjoy_ui::authentication_service_client::AuthenticationServiceClient;
use api::grpc::blockjoy_ui::{ApiToken, LoginUserRequest, RefreshTokenRequest, RequestMeta};
use base64::encode as base64_encode;
use setup::setup;
use std::sync::Arc;
use test_macros::*;
use tonic::transport::Channel;
use tonic::{Request, Status};
use uuid::Uuid;

#[before(call = "setup")]
#[tokio::test]
async fn responds_ok_with_valid_credentials_for_login() {
    let db = _before_values.await;
    let request_meta = RequestMeta {
        id: Some(Uuid::new_v4().to_string()),
        token: None,
        fields: vec![],
        pagination: None,
    };
    let inner = LoginUserRequest {
        meta: Some(request_meta),
        email: "admin@here.com".to_string(),
        password: "abc12345".to_string(),
    };

    assert_grpc_request! { login, Request::new(inner), tonic::Code::Ok, db, AuthenticationServiceClient<Channel> };
}

#[before(call = "setup")]
#[tokio::test]
async fn responds_error_with_invalid_credentials_for_login() {
    let db = _before_values.await;
    let request_meta = RequestMeta {
        id: Some(Uuid::new_v4().to_string()),
        token: None,
        fields: vec![],
        pagination: None,
    };
    let inner = LoginUserRequest {
        meta: Some(request_meta),
        email: "foo@bar.com".to_string(),
        password: "eafe12345".to_string(),
    };

    assert_grpc_request! { login, Request::new(inner), tonic::Code::Unauthenticated, db, AuthenticationServiceClient<Channel> };
}

#[before(call = "setup")]
#[tokio::test]
async fn responds_ok_with_valid_credentials_for_refresh() {
    let db = _before_values.await;
    let user = db.admin_user().await;
    let token = user.get_token(&db.pool).await.unwrap();
    let request_meta = RequestMeta {
        id: Some(Uuid::new_v4().to_string()),
        token: Some(ApiToken {
            value: token.token.clone(),
        }),
        fields: vec![],
        pagination: None,
    };
    let inner = RefreshTokenRequest {
        meta: Some(request_meta),
    };
    let mut request = Request::new(inner);

    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", token.to_base64()).parse().unwrap(),
    );

    assert_grpc_request! { refresh, request, tonic::Code::Ok, db, AuthenticationServiceClient<Channel> };
}

#[before(call = "setup")]
#[tokio::test]
async fn responds_unauthenticated_with_invalid_credentials_for_refresh() {
    let db = _before_values.await;
    let user = db.admin_user().await;
    let invalid_token = base64_encode("asdf.asdfasdfasdfasdfasdf.asfasdfasdfasdfaf");
    let token = user.get_token(&db.pool).await.unwrap();
    let request_meta = RequestMeta {
        id: Some(Uuid::new_v4().to_string()),
        token: Some(ApiToken {
            value: token.token.clone(),
        }),
        fields: vec![],
        pagination: None,
    };
    let inner = RefreshTokenRequest {
        meta: Some(request_meta),
    };
    let mut request = Request::new(inner);

    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", invalid_token).parse().unwrap(),
    );

    assert_grpc_request! { refresh, request, tonic::Code::Unauthenticated, db, AuthenticationServiceClient<Channel> };
}
