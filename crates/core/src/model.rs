//! Core types shared by every dobukasu plugin.

use serde::{Deserialize, Serialize};

/// A single search hit as reported by one engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

impl Hit {
    #[must_use]
    pub fn new(
        title: impl Into<String>,
        url: impl Into<String>,
        snippet: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            url: url.into(),
            snippet: snippet.into(),
        }
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.title.trim().is_empty() && !self.url.trim().is_empty()
    }
}

/// Time filter understood by the engines that support one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeRange {
    Day,
    Week,
    Month,
    Year,
}

/// What the caller asked for. Engines only read this; they never mutate it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub text: String,
    pub limit: usize,
    pub lang: Option<String>,
    pub region: Option<String>,
    pub time_range: Option<TimeRange>,
    pub safe_search: bool,
}

impl SearchQuery {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            limit: 10,
            lang: None,
            region: None,
            time_range: None,
            safe_search: false,
        }
    }
}

/// One merged result. `engines` lists everyone that returned the same URL.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub engines: Vec<String>,
    pub score: f64,
}

/// How one engine finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineStatus {
    Ok,
    Timeout,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EngineReport {
    pub id: String,
    pub status: EngineStatus,
    pub count: usize,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Everything one search produced: the merged list and how each engine did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchOutcome {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub engines: Vec<EngineReport>,
    pub took_ms: u64,
}

impl SearchOutcome {
    /// True when every engine that was asked failed.
    #[must_use]
    pub fn all_failed(&self) -> bool {
        !self.engines.is_empty()
            && self
                .engines
                .iter()
                .all(|report| report.status != EngineStatus::Ok)
    }
}

/// Hits returned by a single engine, in the engine's own ranking order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineHits {
    pub engine: String,
    pub hits: Vec<Hit>,
}

impl EngineHits {
    #[must_use]
    pub fn new(engine: impl Into<String>, hits: Vec<Hit>) -> Self {
        Self {
            engine: engine.into(),
            hits,
        }
    }
}
