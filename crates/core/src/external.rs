//! Plug-ins that are executables rather than linked crates.
//!
//! A plug-in is any program named `dobukasu-engine-<id>` that reads one JSON
//! request on stdin and writes one JSON response on stdout. That contract is
//! the whole ABI: no Rust version coupling, no shared library, no rebuild of
//! the host — an engine can be written in Python, Go, Node or Rust and dropped
//! into a directory.
//!
//! ```text
//! {"v":1,"query":{"text":"rust","limit":10,"lang":null,"region":null,
//!                 "time_range":null,"safe_search":false}}
//! ->
//! {"v":1,"hits":[{"title":"…","url":"https://…","snippet":"…"}]}
//! ```
//!
//! An engine that cannot answer writes `{"v":1,"error":"…"}` or exits
//! non-zero; either way the search reports the failure instead of hiding it.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::engine::{Engine, EngineContext};
use crate::error::EngineError;
use crate::model::{Hit, SearchQuery};

/// File name prefix that marks an executable as an engine.
pub const PLUGIN_PREFIX: &str = "dobukasu-engine-";

/// One external engine, addressed by the executable that implements it.
#[derive(Debug, Clone)]
pub struct ExternalEngine {
    id: &'static str,
    path: PathBuf,
}

impl ExternalEngine {
    /// Build an engine from an executable and the id it should answer to.
    #[must_use]
    pub fn new(id: &'static str, path: impl Into<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            id,
            path: path.into(),
        })
    }

    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[derive(Serialize)]
struct Request<'a> {
    v: u8,
    query: Query<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_agent: Option<&'a str>,
}

#[derive(Serialize)]
struct Query<'a> {
    text: &'a str,
    limit: usize,
    lang: Option<&'a str>,
    region: Option<&'a str>,
    time_range: Option<&'a str>,
    safe_search: bool,
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    hits: Vec<Hit>,
    #[serde(default)]
    error: Option<String>,
}

#[async_trait]
impl Engine for ExternalEngine {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn search(
        &self,
        query: &SearchQuery,
        context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError> {
        let payload = serde_json::to_vec(&Request {
            v: 1,
            query: Query {
                text: &query.text,
                limit: query.limit,
                lang: query.lang.as_deref(),
                region: query.region.as_deref(),
                time_range: query.time_range.map(|range| match range {
                    crate::model::TimeRange::Day => "day",
                    crate::model::TimeRange::Week => "week",
                    crate::model::TimeRange::Month => "month",
                    crate::model::TimeRange::Year => "year",
                }),
                safe_search: query.safe_search,
            },
            user_agent: Some(context.user_agent()),
        })
        .map_err(|error| EngineError::Protocol(error.to_string()))?;

        let mut child = Command::new(&self.path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| {
                EngineError::Transport(format!("cannot run {}: {error}", self.path.display()))
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&payload).await.map_err(|error| {
                EngineError::Transport(format!("cannot write to the plug-in: {error}"))
            })?;
        }

        let output = tokio::time::timeout(context.timeout(), child.wait_with_output())
            .await
            .map_err(|_| EngineError::Timeout)?
            .map_err(|error| EngineError::Transport(error.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr.trim().lines().next().unwrap_or("no output");
            return Err(EngineError::Protocol(format!(
                "plug-in exited with {}: {detail}",
                output.status.code().unwrap_or(-1)
            )));
        }

        let response: Response = serde_json::from_slice(&output.stdout).map_err(|error| {
            EngineError::Protocol(format!("plug-in sent invalid JSON: {error}"))
        })?;
        if let Some(error) = response.error {
            return Err(EngineError::Protocol(error));
        }
        Ok(response.hits.into_iter().filter(Hit::is_complete).collect())
    }
}

/// Every executable in `directory` named `dobukasu-engine-<id>`.
///
/// The id comes from the file name, so installing an engine is literally
/// copying a file: `dobukasu-engine-mojeek` answers to `--engines mojeek`.
#[must_use]
pub fn discover(directory: &std::path::Path) -> Vec<Arc<dyn Engine>> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(id) = name.strip_prefix(PLUGIN_PREFIX) else {
            continue;
        };
        if id.is_empty() || !is_executable(&path) {
            continue;
        }
        found.push(ExternalEngine::new(leak(id), path) as Arc<dyn Engine>);
    }
    found.sort_by_key(|engine| engine.id());
    found
}

/// Engine ids are `&'static str` throughout the core. A discovered plug-in's id
/// lives as long as the process, so leaking the file-name suffix is honest
/// about that lifetime rather than threading a lifetime through every engine.
fn leak(id: &str) -> &'static str {
    Box::leak(id.to_owned().into_boxed_str())
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &std::path::Path) -> bool {
    path.is_file()
}
