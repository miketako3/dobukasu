//! Test doubles for the engine boundary.
//!
//! Engines are the only process-external dependency of the HTTP API, so the
//! API tests replace exactly that boundary and keep everything else real.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use dobukasu_core::api::{AppState, router};
use dobukasu_core::{Config, Engine, EngineContext, EngineError, Hit, Registry, SearchQuery};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

pub enum Behaviour {
    Return(Vec<Hit>),
    Fail(&'static str),
    Hang(Duration),
    Echo,
}

pub struct Stub {
    id: &'static str,
    behaviour: Behaviour,
}

#[async_trait]
impl Engine for Stub {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn search(
        &self,
        query: &SearchQuery,
        _context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError> {
        match &self.behaviour {
            Behaviour::Return(hits) => Ok(hits.clone()),
            Behaviour::Fail(message) => Err(EngineError::Protocol((*message).to_owned())),
            Behaviour::Hang(duration) => {
                tokio::time::sleep(*duration).await;
                Ok(Vec::new())
            }
            Behaviour::Echo => {
                let encoded = url::form_urlencoded::Serializer::new(String::new())
                    .append_pair("q", &query.text)
                    .append_pair("limit", &query.limit.to_string())
                    .append_pair("safe", &query.safe_search.to_string())
                    .append_pair("lang", query.lang.as_deref().unwrap_or("none"))
                    .finish();
                Ok(vec![Hit::new(
                    "echo",
                    format!("https://echo.test/?{encoded}"),
                    "",
                )])
            }
        }
    }
}

#[must_use]
pub fn stub(id: &'static str, behaviour: Behaviour) -> Arc<dyn Engine> {
    Arc::new(Stub { id, behaviour })
}

#[must_use]
pub fn hit(title: &str, url: &str, snippet: &str) -> Hit {
    Hit::new(title, url, snippet)
}

/// Hits named after the engine that produced them, useful for assertions.
#[must_use]
pub fn hits(prefix: &str, urls: &[&str]) -> Vec<Hit> {
    urls.iter()
        .enumerate()
        .map(|(index, url)| {
            Hit::new(
                format!("{prefix} #{}", index + 1),
                *url,
                format!("{prefix} snippet"),
            )
        })
        .collect()
}

#[must_use]
pub fn config() -> Config {
    let mut config = Config::default();
    config.search.limit = 10;
    config.search.max_limit = 50;
    config.search.timeout_ms = 200;
    config.search.dedup = true;
    config
}

pub fn app(engines: Vec<Arc<dyn Engine>>, config: Config) -> Router {
    router(AppState::new(
        Registry::new(engines),
        config,
        reqwest::Client::new(),
    ))
}

pub async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("valid request");
    send(app, request).await
}

pub async fn post(app: &Router, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri("/search")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("valid request");
    send(app, request).await
}

pub async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.expect("infallible");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

pub fn urls(body: &Value) -> Vec<String> {
    body["results"]
        .as_array()
        .map(|results| {
            results
                .iter()
                .filter_map(|result| result["url"].as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

pub fn engines_of(body: &Value, index: usize) -> Vec<String> {
    body["results"][index]["engines"]
        .as_array()
        .map(|engines| {
            engines
                .iter()
                .filter_map(|engine| engine.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

pub fn status_of(body: &Value, engine: &str) -> Option<String> {
    body["engines"]
        .as_array()?
        .iter()
        .find(|report| report["id"] == engine)
        .and_then(|report| report["status"].as_str())
        .map(ToOwned::to_owned)
}
