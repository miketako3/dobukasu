//! What an agent gets when it runs the binary: JSON on stdout, a message on
//! stderr, and an exit code it can branch on.

use std::sync::Arc;

use async_trait::async_trait;
use dobukasu::{Cli, Output};
use dobukasu_core::{Engine, EngineContext, EngineError, Hit, Registry, SearchQuery};
use serde_json::Value;

struct Stub {
    id: &'static str,
    urls: Vec<&'static str>,
}

impl Stub {
    fn engine(id: &'static str, urls: &[&'static str]) -> Arc<dyn Engine> {
        Arc::new(Self {
            id,
            urls: urls.to_vec(),
        })
    }
}

#[async_trait]
impl Engine for Stub {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn search(
        &self,
        _query: &SearchQuery,
        _context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError> {
        Ok(self
            .urls
            .iter()
            .map(|url| Hit::new(format!("title of {url}"), *url, "snippet"))
            .collect())
    }
}

struct Broken;

#[async_trait]
impl Engine for Broken {
    fn id(&self) -> &'static str {
        "broken"
    }

    async fn search(
        &self,
        _query: &SearchQuery,
        _context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError> {
        Err(EngineError::Blocked("turned away".to_owned()))
    }
}

async fn run_with(engines: Vec<Arc<dyn Engine>>, argv: &[&str]) -> Output {
    let cli = Cli::new(Registry::new(engines), reqwest::Client::new());
    let argv: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
    cli.run(&argv).await
}

fn json(output: &Output) -> Value {
    serde_json::from_str(&output.stdout).expect("stdout is JSON")
}

#[tokio::test]
async fn a_search_prints_merged_results_and_how_each_engine_did() {
    let output = run_with(
        vec![
            Stub::engine("alpha", &["https://a.test/1", "https://a.test/2"]),
            Stub::engine("beta", &["https://b.test/1"]),
        ],
        &["rust async", "runtime"],
    )
    .await;

    assert_eq!(output.code, 0, "{}", output.stderr);
    let body = json(&output);
    assert_eq!(body["query"], "rust async runtime");
    assert_eq!(body["results"].as_array().map(Vec::len), Some(3));
    assert_eq!(body["engines"].as_array().map(Vec::len), Some(2));
}

#[tokio::test]
async fn the_limit_flag_truncates_the_merged_list() {
    let output = run_with(
        vec![Stub::engine(
            "alpha",
            &["https://a.test/1", "https://a.test/2", "https://a.test/3"],
        )],
        &["rust", "--limit", "2"],
    )
    .await;

    assert_eq!(json(&output)["results"].as_array().map(Vec::len), Some(2));
}

#[tokio::test]
async fn the_engines_flag_restricts_who_is_asked() {
    let output = run_with(
        vec![
            Stub::engine("alpha", &["https://a.test/1"]),
            Stub::engine("beta", &["https://b.test/1"]),
        ],
        &["rust", "--engines", "beta"],
    )
    .await;

    let body = json(&output);
    assert_eq!(body["engines"][0]["id"], "beta");
    assert_eq!(body["results"].as_array().map(Vec::len), Some(1));
}

#[tokio::test]
async fn an_unknown_engine_is_a_usage_error() {
    let output = run_with(
        vec![Stub::engine("alpha", &["https://a.test/1"])],
        &["rust", "--engines", "nope"],
    )
    .await;

    assert_eq!(output.code, 2);
    assert_eq!(output.stdout, "");
    assert!(
        output.stderr.contains("unknown engine: nope"),
        "{}",
        output.stderr
    );
}

#[tokio::test]
async fn a_search_that_lost_every_engine_exits_one_but_still_reports() {
    let output = run_with(vec![Arc::new(Broken)], &["rust"]).await;

    assert_eq!(output.code, 1);
    let body = json(&output);
    assert_eq!(body["results"].as_array().map(Vec::len), Some(0));
    assert_eq!(body["engines"][0]["status"], "error");
    assert!(
        body["engines"][0]["error"]
            .as_str()
            .is_some_and(|text| text.contains("turned away"))
    );
}

#[tokio::test]
async fn a_missing_query_is_refused_before_any_engine_runs() {
    let output = run_with(vec![Stub::engine("alpha", &["https://a.test/1"])], &[]).await;

    assert_eq!(output.code, 2);
    assert!(output.stderr.contains("a query is required"));
}

#[tokio::test]
async fn the_engines_command_lists_what_can_be_asked() {
    let output = run_with(
        vec![
            Stub::engine("alpha", &["https://a.test/1"]),
            Stub::engine("beta", &[]),
        ],
        &["engines"],
    )
    .await;

    let body = json(&output);
    let ids: Vec<&str> = body["engines"]
        .as_array()
        .expect("engines")
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect();
    assert_eq!(ids, vec!["alpha", "beta"]);
    assert_eq!(body["engines"][0]["default"], true);
}

#[tokio::test]
async fn version_and_help_answer_without_a_query() {
    let version = run_with(Vec::new(), &["--version"]).await;
    assert_eq!(version.code, 0);
    assert!(version.stdout.contains(env!("CARGO_PKG_VERSION")));

    let help = run_with(Vec::new(), &["--help"]).await;
    assert_eq!(help.code, 0);
    assert!(help.stdout.contains("USAGE"));
}
