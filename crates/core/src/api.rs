//! The HTTP surface: JSON in, JSON out, nothing else.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::engine::{Engine, EngineContext};
use crate::error::{DobukasuError, EngineError};
use crate::merge::fuse;
use crate::model::{
    EngineHits, EngineReport, EngineStatus, SearchQuery, SearchResponse, TimeRange,
};
use crate::registry::Registry;

const MAX_QUERY_CHARS: usize = 1_024;
const MIN_TIMEOUT_MS: u64 = 100;
const MAX_TIMEOUT_MS: u64 = 30_000;

#[derive(Clone)]
pub struct AppState {
    registry: Arc<Registry>,
    config: Arc<Config>,
    http: reqwest::Client,
}

impl AppState {
    #[must_use]
    pub fn new(registry: Registry, config: Config, http: reqwest::Client) -> Self {
        Self {
            registry: Arc::new(registry),
            config: Arc::new(config),
            http,
        }
    }

    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/healthz", get(healthz))
        .route("/engines", get(list_engines))
        .route("/search", get(search_get).post(search_post))
        .with_state(state)
}

/// A parameter accepted as a string (query string) or native JSON scalar.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Param {
    Text(String),
    Integer(u64),
    Flag(bool),
}

impl Param {
    fn text(&self) -> String {
        match self {
            Self::Text(value) => value.clone(),
            Self::Integer(value) => value.to_string(),
            Self::Flag(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchParams {
    pub q: Option<Param>,
    pub engines: Option<Param>,
    pub limit: Option<Param>,
    pub lang: Option<Param>,
    pub region: Option<Param>,
    pub time_range: Option<Param>,
    pub safesearch: Option<Param>,
    pub dedup: Option<Param>,
    pub timeout_ms: Option<Param>,
}

#[derive(Debug, Serialize)]
struct ErrorInfo {
    code: &'static str,
    message: String,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: ErrorInfo,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    engines: Vec<EngineReport>,
}

fn error_response(error: &DobukasuError) -> Response {
    let status = StatusCode::from_u16(error.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = ErrorBody {
        error: ErrorInfo {
            code: error.code(),
            message: error.to_string(),
        },
        engines: Vec::new(),
    };
    (status, Json(body)).into_response()
}

async fn index() -> Response {
    Json(serde_json::json!({
        "name": "dobukasu",
        "version": env!("CARGO_PKG_VERSION"),
        "endpoints": ["GET /search", "POST /search", "GET /engines", "GET /healthz"],
    }))
    .into_response()
}

async fn healthz() -> Response {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
    .into_response()
}

#[derive(Debug, Serialize)]
struct EngineInfo {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    enabled: bool,
    default: bool,
}

#[derive(Debug, Serialize)]
struct EnginesBody {
    engines: Vec<EngineInfo>,
}

async fn list_engines(State(state): State<AppState>) -> Response {
    let engines = state
        .registry
        .ids()
        .iter()
        .map(|id| {
            let plugin = crate::engine::plugin(id);
            EngineInfo {
                id,
                label: plugin.map_or(id, |plugin| plugin.label),
                description: plugin.map_or("", |plugin| plugin.description),
                enabled: true,
                default: state.registry.is_default(id),
            }
        })
        .collect();
    Json(EnginesBody { engines }).into_response()
}

async fn search_get(
    State(state): State<AppState>,
    params: Result<Query<SearchParams>, QueryRejection>,
) -> Response {
    match params {
        Ok(Query(params)) => handle_search(&state, params).await,
        Err(rejection) => error_response(&DobukasuError::InvalidRequest(rejection.body_text())),
    }
}

async fn search_post(
    State(state): State<AppState>,
    params: Result<Json<SearchParams>, JsonRejection>,
) -> Response {
    match params {
        Ok(Json(params)) => handle_search(&state, params).await,
        Err(rejection) => error_response(&DobukasuError::InvalidRequest(rejection.body_text())),
    }
}

async fn handle_search(state: &AppState, params: SearchParams) -> Response {
    let plan = match build_request(state, &params) {
        Ok(plan) => plan,
        Err(error) => return error_response(&error),
    };

    let started = Instant::now();
    let (sources, reports) = run_engines(state, &plan.query, plan.engines, plan.timeout).await;
    let results = fuse(&sources, plan.query.limit, plan.dedup);

    if reports
        .iter()
        .all(|report| report.status != EngineStatus::Ok)
    {
        let mut body = ErrorBody {
            error: ErrorInfo {
                code: DobukasuError::AllEnginesFailed.code(),
                message: DobukasuError::AllEnginesFailed.to_string(),
            },
            engines: reports,
        };
        body.error.message = format!(
            "every engine failed: {}",
            body.engines
                .iter()
                .filter_map(|report| report
                    .error
                    .as_ref()
                    .map(|error| format!("{}: {error}", report.id)))
                .collect::<Vec<_>>()
                .join("; ")
        );
        return (StatusCode::BAD_GATEWAY, Json(body)).into_response();
    }

    let elapsed = started.elapsed();
    let response = SearchResponse {
        query: plan.query.text,
        results,
        engines: reports,
        took_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
    };
    Json(response).into_response()
}

struct RequestPlan {
    query: SearchQuery,
    engines: Vec<Arc<dyn Engine>>,
    dedup: bool,
    timeout: Duration,
}

fn build_request(state: &AppState, params: &SearchParams) -> Result<RequestPlan, DobukasuError> {
    let text = params
        .q
        .as_ref()
        .map(Param::text)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| DobukasuError::InvalidRequest("q is required".to_owned()))?;

    if text.chars().count() > MAX_QUERY_CHARS {
        return Err(DobukasuError::InvalidRequest(format!(
            "q is longer than {MAX_QUERY_CHARS} characters"
        )));
    }

    let max_limit = state.config.search.max_limit;
    let limit = match &params.limit {
        Some(value) => parse_usize("limit", &value.text())?,
        None => state.config.search.limit,
    }
    .clamp(1, max_limit);

    let requested = params.engines.as_ref().map(|value| {
        value
            .text()
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    });
    let engines = state.registry.resolve(requested.as_deref())?;

    let lang = params.lang.as_ref().map(Param::text);
    let region = params.region.as_ref().map(Param::text);
    let time_range = match &params.time_range {
        Some(value) => Some(parse_time_range(&value.text())?),
        None => None,
    };
    let safe_search = match &params.safesearch {
        Some(value) => parse_bool("safesearch", &value.text())?,
        None => false,
    };
    let dedup = match &params.dedup {
        Some(value) => parse_bool("dedup", &value.text())?,
        None => state.config.search.dedup,
    };
    let timeout_ms = match &params.timeout_ms {
        Some(value) => parse_u64("timeout_ms", &value.text())?,
        None => state.config.search.timeout_ms,
    }
    .clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS);

    let query = SearchQuery {
        text,
        limit,
        lang,
        region,
        time_range,
        safe_search,
    };
    Ok(RequestPlan {
        query,
        engines,
        dedup,
        timeout: Duration::from_millis(timeout_ms),
    })
}

async fn run_engines(
    state: &AppState,
    query: &SearchQuery,
    engines: Vec<Arc<dyn Engine>>,
    timeout: Duration,
) -> (Vec<EngineHits>, Vec<EngineReport>) {
    let tasks = engines.into_iter().map(|engine| {
        let query = query.clone();
        let http = state.http.clone();
        let user_agent = state.config.search.user_agent.clone();
        let language = query.lang.as_ref().map(|lang| format!("{lang},en;q=0.8"));
        let context = EngineContext::new(http, timeout, user_agent, language);
        async move {
            let started = Instant::now();
            let outcome = tokio::time::timeout(timeout, engine.search(&query, &context)).await;
            let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            let id = engine.id().to_owned();

            let (status, hits, error) = match outcome {
                Ok(Ok(hits)) => (EngineStatus::Ok, hits, None),
                Ok(Err(EngineError::Timeout)) | Err(_) => (EngineStatus::Timeout, Vec::new(), None),
                Ok(Err(error)) => (EngineStatus::Error, Vec::new(), Some(error.to_string())),
            };

            let report = EngineReport {
                id: id.clone(),
                status,
                count: hits.len(),
                duration_ms,
                error,
            };
            (EngineHits::new(id, hits), report)
        }
    });

    let completed = futures::future::join_all(tasks).await;
    let mut sources = Vec::with_capacity(completed.len());
    let mut reports = Vec::with_capacity(completed.len());
    for (source, report) in completed {
        sources.push(source);
        reports.push(report);
    }
    (sources, reports)
}

fn parse_usize(name: &str, value: &str) -> Result<usize, DobukasuError> {
    value
        .parse()
        .map_err(|_| DobukasuError::InvalidRequest(format!("{name} must be a positive integer")))
}

fn parse_u64(name: &str, value: &str) -> Result<u64, DobukasuError> {
    value
        .parse()
        .map_err(|_| DobukasuError::InvalidRequest(format!("{name} must be a positive integer")))
}

fn parse_bool(name: &str, value: &str) -> Result<bool, DobukasuError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(DobukasuError::InvalidRequest(format!(
            "{name} must be true or false"
        ))),
    }
}

fn parse_time_range(value: &str) -> Result<TimeRange, DobukasuError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "day" => Ok(TimeRange::Day),
        "week" => Ok(TimeRange::Week),
        "month" => Ok(TimeRange::Month),
        "year" => Ok(TimeRange::Year),
        _ => Err(DobukasuError::InvalidRequest(
            "time_range must be day, week, month or year".to_owned(),
        )),
    }
}
