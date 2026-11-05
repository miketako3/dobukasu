//! Engine plug-in contract and the compile-time registry plugins register into.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, USER_AGENT};
use url::Url;

use crate::error::EngineError;
use crate::model::{Hit, SearchQuery};

const HTML_ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";

/// Everything an engine needs from the host: one shared HTTP client and the
/// request-scoped knobs.
#[derive(Clone, Debug)]
pub struct EngineContext {
    http: reqwest::Client,
    timeout: Duration,
    user_agent: String,
    accept_language: Option<String>,
}

impl EngineContext {
    #[must_use]
    pub fn new(
        http: reqwest::Client,
        timeout: Duration,
        user_agent: impl Into<String>,
        accept_language: Option<String>,
    ) -> Self {
        Self {
            http,
            timeout,
            user_agent: user_agent.into(),
            accept_language,
        }
    }

    pub fn request(&self, url: Url) -> reqwest::RequestBuilder {
        let mut request = self
            .http
            .get(url)
            .timeout(self.timeout)
            .header(USER_AGENT, &self.user_agent)
            .header(ACCEPT, HTML_ACCEPT);
        if let Some(language) = &self.accept_language {
            request = request.header(ACCEPT_LANGUAGE, language);
        }
        request
    }

    /// Fetch a page and return its body, mapping transport failures onto
    /// [`EngineError`].
    pub async fn fetch_text(&self, url: Url) -> Result<String, EngineError> {
        let response = self
            .request(url)
            .send()
            .await
            .map_err(|error| transport(&error))?;
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(EngineError::Blocked("rate limited".to_owned()));
        }
        if !status.is_success() {
            return Err(EngineError::Status(status.as_u16()));
        }
        response.text().await.map_err(|error| transport(&error))
    }
}

fn transport(error: &reqwest::Error) -> EngineError {
    if error.is_timeout() {
        EngineError::Timeout
    } else {
        EngineError::Transport(error.to_string())
    }
}

/// A search backend. Implementations are pure hosts of a remote ranking.
#[async_trait]
pub trait Engine: Send + Sync + 'static {
    fn id(&self) -> &'static str;

    async fn search(
        &self,
        query: &SearchQuery,
        context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError>;
}

/// Typed access to `[engines.<id>]` configuration values.
#[derive(Debug, Clone, Default)]
pub struct PluginOptions {
    values: BTreeMap<String, toml::Value>,
}

impl PluginOptions {
    #[must_use]
    pub fn new(values: BTreeMap<String, toml::Value>) -> Self {
        Self { values }
    }

    #[must_use]
    pub fn string(&self, key: &str) -> Option<&str> {
        self.values.get(key).and_then(toml::Value::as_str)
    }

    #[must_use]
    pub fn string_or(&self, key: &str, fallback: &str) -> String {
        self.string(key).unwrap_or(fallback).to_owned()
    }
}

/// One compile-time plug-in. Every engine crate submits one of these; the core
/// crate never names an engine.
pub struct Plugin {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub enabled_by_default: bool,
    pub build: fn(&PluginOptions) -> Result<Arc<dyn Engine>, EngineError>,
}

inventory::collect!(Plugin);

/// Every plug-in linked into this binary, ordered by id.
#[must_use]
pub fn plugins() -> Vec<&'static Plugin> {
    let mut all: Vec<&'static Plugin> = inventory::iter::<Plugin>.into_iter().collect();
    all.sort_by_key(|plugin| plugin.id);
    all
}

#[must_use]
pub fn plugin(id: &str) -> Option<&'static Plugin> {
    plugins().into_iter().find(|plugin| plugin.id == id)
}
