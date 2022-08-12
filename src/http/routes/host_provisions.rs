//! Routes namespaced by ***/host_provisions***

use crate::http::handlers::*;
use crate::http::routes::*;

pub fn routes() -> Router {
    Router::new()
        .route("/", post(create_host_provision))
        .route("/:id", get(get_host_provision))
}
