//! Engine plug-in contract and the compile-time registry plugins register into.

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
        self.request_as(url, &self.user_agent.clone())
    }

    fn request_as(&self, url: Url, user_agent: &str) -> reqwest::RequestBuilder {
        let mut request = self
            .http
            .get(url)
            .timeout(self.timeout)
            .header(USER_AGENT, user_agent)
            .header(ACCEPT, HTML_ACCEPT);
        if let Some(language) = &self.accept_language {
            request = request.header(ACCEPT_LANGUAGE, language);
        }
        request
    }

    /// Fetch a page and return its body, mapping transport failures onto
    /// [`EngineError`].
    pub async fn fetch_text(&self, url: Url) -> Result<String, EngineError> {
        self.fetch_text_as(url, &self.user_agent.clone()).await
    }

    /// Fetch a page with an engine-specific user agent, for upstreams that
    /// only serve their plain layout to particular clients (Google and its
    /// Nokia mobile agents).
    pub async fn fetch_text_as(&self, url: Url, user_agent: &str) -> Result<String, EngineError> {
        self.fetch(self.request_as(url, user_agent)).await
    }

    /// Send a request the engine built itself — JSON APIs need their own
    /// `Accept` header and auth — and map the answer onto [`EngineError`].
    pub async fn fetch(&self, request: reqwest::RequestBuilder) -> Result<String, EngineError> {
        let response = request.send().await.map_err(|error| transport(&error))?;
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(EngineError::Blocked("rate limited".to_owned()));
        }
        if matches!(
            status,
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
        ) {
            return Err(EngineError::Blocked(format!(
                "upstream refused this client ({})",
                status.as_u16()
            )));
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

    /// User agent this upstream answers best. `None` uses the shared default.
    ///
    /// Some engines want a browser-like string, others serve their real
    /// results only to clients that admit to being a program.
    fn user_agent(&self) -> Option<&'static str> {
        None
    }

    async fn search(
        &self,
        query: &SearchQuery,
        context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError>;
}

/// One compile-time plug-in. Every engine crate submits one of these; the core
/// crate never names an engine.
pub struct Plugin {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub enabled_by_default: bool,
    pub build: fn() -> Result<Arc<dyn Engine>, EngineError>,
}

inventory::collect!(Plugin);

/// Every plug-in linked into this binary, ordered by id.
#[must_use]
pub fn plugins() -> Vec<&'static Plugin> {
    let mut all: Vec<&'static Plugin> = inventory::iter::<Plugin>.into_iter().collect();
    all.sort_by_key(|plugin| plugin.id);
    all
}
