//! Executable plug-ins: the JSON-over-stdio contract, and what a broken
//! plug-in reports.

use std::path::{Path, PathBuf};
use std::time::Duration;

use dobukasu_core::{Engine, EngineContext, EngineError, ExternalEngine, SearchQuery, discover};

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("dobukasu-plugin-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        Self { dir }
    }

    /// Write an executable plug-in and return its path.
    fn engine(&self, name: &str, body: &str) -> PathBuf {
        let path = self.dir.join(name);
        std::fs::write(&path, body).expect("write plug-in");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&path).expect("stat").permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&path, permissions).expect("chmod");
        }
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn context(timeout_ms: u64) -> EngineContext {
    EngineContext::new(
        reqwest::Client::new(),
        Duration::from_millis(timeout_ms),
        "dobukasu-test",
        None,
    )
}

fn query(text: &str) -> SearchQuery {
    SearchQuery::new(text)
}

fn engine(name: &'static str, path: &Path) -> std::sync::Arc<ExternalEngine> {
    ExternalEngine::new(name, path)
}

const GOOD: &str = r#"#!/bin/sh
read -r request
printf '%s' '{"v":1,"hits":[{"title":"Example","url":"https://example.test/","snippet":"hello from a plug-in"}]}'
"#;

#[tokio::test]
async fn a_plug_in_that_answers_becomes_hits() {
    let fixture = Fixture::new("good");
    let path = fixture.engine("dobukasu-engine-example", GOOD);

    let hits = engine("example", &path)
        .search(&query("rust"), &context(2_000))
        .await
        .expect("hits");

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "Example");
    assert_eq!(hits[0].url, "https://example.test/");
}

#[tokio::test]
async fn the_request_carries_the_query_as_json() {
    let fixture = Fixture::new("echo");
    let path = fixture.engine(
        "dobukasu-engine-echo",
        r#"#!/bin/sh
read -r request
printf '%s' "$request" > "$(dirname "$0")/seen.json"
printf '%s' '{"v":1,"hits":[]}'
"#,
    );

    engine("echo", &path)
        .search(&query("tokio runtime"), &context(2_000))
        .await
        .expect("answered");

    let seen = std::fs::read_to_string(fixture.dir.join("seen.json")).expect("request captured");
    assert!(seen.contains("\"text\":\"tokio runtime\""), "{seen}");
    assert!(seen.contains("\"v\":1"), "{seen}");
}

#[tokio::test]
async fn a_plug_in_that_reports_an_error_is_reported() {
    let fixture = Fixture::new("error-json");
    let path = fixture.engine(
        "dobukasu-engine-error",
        r#"#!/bin/sh
read -r request
printf '%s' '{"v":1,"error":"quota exceeded"}'
"#,
    );

    let error = engine("error", &path)
        .search(&query("rust"), &context(2_000))
        .await
        .expect_err("must fail");

    assert!(matches!(error, EngineError::Protocol(message) if message.contains("quota exceeded")));
}

#[tokio::test]
async fn a_plug_in_that_crashes_is_reported_with_its_message() {
    let fixture = Fixture::new("crash");
    let path = fixture.engine(
        "dobukasu-engine-crash",
        "#!/bin/sh\necho 'no api key configured' >&2\nexit 3\n",
    );

    let error = engine("crash", &path)
        .search(&query("rust"), &context(2_000))
        .await
        .expect_err("must fail");

    let text = error.to_string();
    assert!(text.contains("no api key configured"), "{text}");
    assert!(text.contains('3'), "{text}");
}

#[tokio::test]
async fn a_plug_in_that_sends_junk_is_reported() {
    let fixture = Fixture::new("junk");
    let path = fixture.engine(
        "dobukasu-engine-junk",
        "#!/bin/sh\nread -r request\necho 'not json'\n",
    );

    let error = engine("junk", &path)
        .search(&query("rust"), &context(2_000))
        .await
        .expect_err("must fail");

    assert!(matches!(error, EngineError::Protocol(message) if message.contains("invalid JSON")));
}

#[tokio::test]
async fn a_plug_in_that_hangs_times_out() {
    let fixture = Fixture::new("slow");
    let path = fixture.engine(
        "dobukasu-engine-slow",
        "#!/bin/sh\nread -r request\nsleep 5\n",
    );

    let error = engine("slow", &path)
        .search(&query("rust"), &context(120))
        .await
        .expect_err("must fail");

    assert_eq!(error, EngineError::Timeout);
}

#[test]
fn discovery_takes_executables_and_names_them_after_the_file() {
    let fixture = Fixture::new("discovery");
    fixture.engine("dobukasu-engine-alpha", GOOD);
    fixture.engine("dobukasu-engine-beta", GOOD);
    fixture.engine("notes.txt", "not a plug-in");

    let found = discover(&fixture.dir);
    let ids: Vec<&str> = found.iter().map(|engine| engine.id()).collect();

    assert_eq!(ids, vec!["alpha", "beta"]);
}

#[test]
fn a_directory_without_plug_ins_is_simply_empty() {
    let fixture = Fixture::new("empty");

    assert!(discover(&fixture.dir).is_empty());
    assert!(discover(&fixture.dir.join("missing")).is_empty());
}
