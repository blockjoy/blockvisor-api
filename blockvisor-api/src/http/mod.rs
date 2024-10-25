pub mod handlers;
pub mod response;

use std::sync::Arc;

use axum::routing::Router;
use axum_tracing_opentelemetry::middleware::OtelAxumLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::config::Context;

use self::handlers::{api_key, chargebee, health, mqtt, stripe};

pub fn router(context: &Arc<Context>) -> Router {
    let cors = CorsLayer::new()
        .allow_headers(Any)
        .allow_methods(Any)
        .allow_origin(Any);

    Router::new()
        .layer(cors)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(OtelAxumLayer::default())
        .nest("/v1/api_key", api_key::router(context.clone()))
        .nest("/v1/stripe", stripe::router(context.clone()))
        .nest("/mqtt", mqtt::router(context.clone()))
        .nest("/chargebee", chargebee::router(context.clone()))
        .merge(health::router(context.clone()))
}
