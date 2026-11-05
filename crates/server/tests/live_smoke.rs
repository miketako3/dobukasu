//! Network smoke tests. Ignored by default; run with
//! `cargo test -- --ignored` from a machine with internet access.
//!
//! These assert the one thing the offline suite cannot: that the upstream
//! still answers with markup the parsers understand.

// Link every bundled plug-in into this test binary, exactly like `main` does.
use dobukasu_engine_bing as _;
use dobukasu_engine_duckduckgo as _;
use dobukasu_engine_google as _;

use std::time::Duration;

use dobukasu_core::config::Config;
use dobukasu_core::registry::Registry;
use dobukasu_core::{EngineContext, SearchQuery};

fn context() -> EngineContext {
    EngineContext::new(
        reqwest::Client::builder().build().expect("client"),
        Duration::from_secs(20),
        Config::default().search.user_agent,
        Some("en-US,en;q=0.9".to_owned()),
    )
}

async fn search(id: &str) -> Vec<dobukasu_core::Hit> {
    let registry = Registry::from_plugins(&Config::default()).expect("registry");
    let engine = registry.get(id).expect("engine is enabled");
    let mut query = SearchQuery::new("rust programming language");
    query.limit = 10;
    engine
        .search(&query, &context())
        .await
        .unwrap_or_else(|error| panic!("{id} failed: {error}"))
}

#[tokio::test]
#[ignore = "needs network access"]
async fn bing_still_answers() {
    let hits = search("bing").await;
    assert_ne!(hits.len(), 0);
}

#[tokio::test]
#[ignore = "needs network access"]
async fn duckduckgo_still_answers() {
    let hits = search("duckduckgo").await;
    assert!(
        !hits.is_empty(),
        "duckduckgo may be rate limiting this host"
    );
}

#[tokio::test]
#[ignore = "needs network access"]
async fn google_either_answers_or_reports_a_block() {
    let registry = Registry::from_plugins(&Config::default()).expect("registry");
    let engine = registry.get("google").expect("engine is enabled");
    match engine
        .search(&SearchQuery::new("rust programming language"), &context())
        .await
    {
        Ok(hits) => assert_ne!(hits.len(), 0),
        Err(error) => {
            let message = error.to_string();
            assert!(
                message.contains("block") || message.contains("status"),
                "unexpected google failure: {message}"
            );
        }
    }
}
