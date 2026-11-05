//! The engine's HTTP adapter: one shared client, timeouts, and honest error
//! mapping. A local server stands in for an upstream.

use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use dobukasu_core::{EngineContext, EngineError};

async fn serve(routes: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = listener.local_addr().expect("address");
    tokio::spawn(async move {
        axum::serve(listener, routes).await.expect("serve");
    });
    format!("http://{address}/")
}

fn context(timeout_ms: u64) -> EngineContext {
    EngineContext::new(
        reqwest::Client::new(),
        Duration::from_millis(timeout_ms),
        "dobukasu-test",
        Some("en-US,en;q=0.9".to_owned()),
    )
}

#[tokio::test]
async fn a_successful_response_returns_the_body() {
    let base = serve(Router::new().route("/", get(|| async { "hello" }))).await;

    let body = context(1_000)
        .fetch_text(base.parse().expect("url"))
        .await
        .expect("body");

    assert_eq!(body, "hello");
}

#[tokio::test]
async fn an_error_status_is_reported_as_a_status() {
    let base =
        serve(Router::new().route("/", get(|| async { (StatusCode::NOT_FOUND, "missing") }))).await;

    let error = context(1_000)
        .fetch_text(base.parse().expect("url"))
        .await
        .expect_err("must fail");

    assert_eq!(error, EngineError::Status(404));
}

#[tokio::test]
async fn too_many_requests_is_reported_as_blocked() {
    let base = serve(Router::new().route(
        "/",
        get(|| async { (StatusCode::TOO_MANY_REQUESTS, "slow down") }),
    ))
    .await;

    let error = context(1_000)
        .fetch_text(base.parse().expect("url"))
        .await
        .expect_err("must fail");

    assert!(matches!(error, EngineError::Blocked(_)), "{error:?}");
}

#[tokio::test]
async fn a_slow_upstream_is_reported_as_a_timeout() {
    let base = serve(Router::new().route(
        "/",
        get(|| async {
            tokio::time::sleep(Duration::from_millis(500)).await;
            "late"
        }),
    ))
    .await;

    let error = context(50)
        .fetch_text(base.parse().expect("url"))
        .await
        .expect_err("must fail");

    assert_eq!(error, EngineError::Timeout);
}

#[tokio::test]
async fn an_unreachable_host_is_a_transport_error() {
    let error = context(200)
        .fetch_text("http://127.0.0.1:1/".parse().expect("url"))
        .await
        .expect_err("must fail");

    assert!(matches!(error, EngineError::Transport(_)), "{error:?}");
}

#[tokio::test]
async fn an_engine_can_ask_for_its_own_user_agent() {
    let base = serve(Router::new().route(
        "/",
        get(|headers: axum::http::HeaderMap| async move {
            headers
                .get(axum::http::header::USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("missing")
                .to_owned()
        }),
    ))
    .await;

    let body = context(1_000)
        .fetch_text_as(base.parse().expect("url"), "Nokia7610/2.0")
        .await
        .expect("body");

    assert_eq!(body, "Nokia7610/2.0");
}
