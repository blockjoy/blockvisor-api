mod setup;

use axum::http::{Request, StatusCode};
use hyper::Body;
use tower::ServiceExt;

fn possible_routes() -> Vec<(&'static str, &'static str, StatusCode)> {
    vec![
        // Non nested routes
        ("/health", "GET", StatusCode::OK),
    ]
}

#[tokio::test]
async fn test_possible_routes() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    let db = api::TestDb::setup().await;
    let routes = possible_routes();
    let app = api::http::server(db.pool.clone()).await;

    let mut cnt = 1;

    for item in routes {
        let route = item.0;
        let method = item.1;
        let expected_response_code = item.2;

        println!("testing route #{cnt} {method} {route}");

        let req = Request::builder()
            .method(method)
            .uri(route)
            .body(Body::empty())?;
        let response = app.clone().oneshot(req).await?;

        assert_eq!(response.status(), expected_response_code);
        cnt += 1;
    }

    Ok(())
}
