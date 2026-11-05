//! The runner: fan out, report every engine honestly, merge what survived.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dobukasu_core::{
    Engine, EngineContext, EngineError, EngineStatus, Hit, SearchQuery, search as run_search,
};

#[derive(Clone)]
enum Behaviour {
    Return(Vec<Hit>),
    Fail,
    Hang,
}

struct Stub {
    id: &'static str,
    behaviour: Behaviour,
}

impl Stub {
    fn engine(id: &'static str, behaviour: Behaviour) -> Arc<dyn Engine> {
        Arc::new(Self { id, behaviour })
    }

    fn returning(id: &'static str, urls: &[&'static str]) -> Arc<dyn Engine> {
        Self::engine(
            id,
            Behaviour::Return(
                urls.iter()
                    .map(|url| Hit::new(format!("title of {url}"), *url, "snippet"))
                    .collect(),
            ),
        )
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
        match &self.behaviour {
            Behaviour::Return(hits) => Ok(hits.clone()),
            Behaviour::Fail => Err(EngineError::Blocked("turned away".to_owned())),
            Behaviour::Hang => {
                tokio::time::sleep(Duration::from_secs(30)).await;
                Ok(Vec::new())
            }
        }
    }
}

async fn search_with(engines: Vec<Arc<dyn Engine>>, limit: usize) -> dobukasu_core::SearchOutcome {
    let client = reqwest::Client::new();
    let mut query = SearchQuery::new("rust");
    query.limit = limit;
    run_search(&client, engines, &query, Duration::from_millis(100), true).await
}

#[tokio::test]
async fn a_failing_engine_does_not_stop_the_search() {
    let outcome = search_with(
        vec![
            Stub::returning("good", &["https://a.test/1", "https://a.test/2"]),
            Stub::engine("bad", Behaviour::Fail),
        ],
        10,
    )
    .await;

    assert_eq!(outcome.results.len(), 2);
    assert!(!outcome.all_failed());

    let bad = outcome
        .engines
        .iter()
        .find(|report| report.id == "bad")
        .expect("bad engine reported");
    assert_eq!(bad.status, EngineStatus::Error);
    assert_eq!(bad.count, 0);
    assert!(
        bad.error
            .as_deref()
            .is_some_and(|text| text.contains("turned away"))
    );
}

#[tokio::test]
async fn an_engine_that_never_answers_is_reported_as_a_timeout() {
    let outcome = search_with(
        vec![
            Stub::returning("good", &["https://a.test/1"]),
            Stub::engine("slow", Behaviour::Hang),
        ],
        10,
    )
    .await;

    let slow = outcome
        .engines
        .iter()
        .find(|report| report.id == "slow")
        .expect("slow engine reported");
    assert_eq!(slow.status, EngineStatus::Timeout);
    assert!(slow.error.is_some());
    assert_eq!(outcome.results.len(), 1);
}

#[tokio::test]
async fn a_search_where_everything_fails_says_so() {
    let outcome = search_with(
        vec![
            Stub::engine("a", Behaviour::Fail),
            Stub::engine("b", Behaviour::Fail),
        ],
        10,
    )
    .await;

    assert!(outcome.all_failed());
    assert_eq!(outcome.results.len(), 0);
    assert_eq!(outcome.engines.len(), 2);
}

#[tokio::test]
async fn the_limit_is_applied_after_merging() {
    let outcome = search_with(
        vec![
            Stub::returning("a", &["https://a.test/1", "https://a.test/2"]),
            Stub::returning("b", &["https://b.test/1", "https://b.test/2"]),
        ],
        3,
    )
    .await;

    assert_eq!(outcome.results.len(), 3);
}

#[tokio::test]
async fn the_same_page_from_two_engines_is_one_result_listing_both() {
    let outcome = search_with(
        vec![
            Stub::returning("a", &["https://e.test/shared"]),
            Stub::returning("b", &["https://e.test/shared"]),
        ],
        10,
    )
    .await;

    assert_eq!(outcome.results.len(), 1);
    assert_eq!(
        outcome.results[0].engines,
        vec!["a".to_owned(), "b".to_owned()]
    );
    assert!((outcome.results[0].score - 2.0 / 61.0).abs() < 1e-12);
}
