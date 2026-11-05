//! Fan out to the engines, then merge what came back.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::future::join_all;

use crate::engine::{Engine, EngineContext};
use crate::merge::fuse;
use crate::model::{EngineHits, EngineReport, EngineStatus, SearchOutcome, SearchQuery};

/// Default user agent. Engines override it where their upstream cares.
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Ask every engine in parallel and fuse the rankings.
///
/// A failing engine never fails the search: it is reported in `engines` and
/// the surviving rankings are still merged.
pub async fn search(
    client: &reqwest::Client,
    engines: Vec<Arc<dyn Engine>>,
    query: &SearchQuery,
    timeout: Duration,
    dedup: bool,
) -> SearchOutcome {
    let started = Instant::now();
    let language = query.lang.as_ref().map(|lang| format!("{lang},en;q=0.8"));

    let tasks = engines.into_iter().map(|engine| {
        let query = query.clone();
        let context = EngineContext::new(
            client.clone(),
            timeout,
            DEFAULT_USER_AGENT,
            language.clone(),
        );
        async move {
            let started = Instant::now();
            let outcome = tokio::time::timeout(timeout, engine.search(&query, &context)).await;
            let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            (engine.id(), outcome, duration_ms)
        }
    });

    let mut hits = Vec::new();
    let mut reports = Vec::new();
    for (id, outcome, duration_ms) in join_all(tasks).await {
        match outcome {
            Ok(Ok(found)) => {
                let count = found.len();
                hits.push(EngineHits::new(id, found));
                reports.push(EngineReport {
                    id: id.to_owned(),
                    status: EngineStatus::Ok,
                    count,
                    duration_ms,
                    error: None,
                });
            }
            Ok(Err(error)) => reports.push(EngineReport {
                id: id.to_owned(),
                status: if error == crate::error::EngineError::Timeout {
                    EngineStatus::Timeout
                } else {
                    EngineStatus::Error
                },
                count: 0,
                duration_ms,
                error: Some(error.to_string()),
            }),
            Err(_) => reports.push(EngineReport {
                id: id.to_owned(),
                status: EngineStatus::Timeout,
                count: 0,
                duration_ms,
                error: Some("no answer within the deadline".to_owned()),
            }),
        }
    }

    let results = fuse(&hits, query.limit, dedup);
    SearchOutcome {
        query: query.text.clone(),
        results,
        engines: reports,
        took_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    }
}
