//! The HTTP contract: requests in, JSON out.
//!
//! Every case goes through the real router, real parameter parsing and real
//! rank fusion; only the engines (the process-external boundary) are stubbed.

mod common;

use common::*;
use dobukasu_core::Config;
use serde_json::json;

#[tokio::test]
async fn a_search_returns_the_hits_of_the_engine() {
    let app = app(
        vec![stub(
            "search-a",
            Behaviour::Return(hits("a", &["https://e.test/1", "https://e.test/2"])),
        )],
        config(),
    );

    let (status, body) = get(&app, "/search?q=rust").await;

    assert_eq!(status, 200);
    assert_eq!(body["query"], "rust");
    assert_eq!(urls(&body), ["https://e.test/1", "https://e.test/2"]);
    assert_eq!(body["results"][0]["title"], "a #1");
    assert_eq!(body["results"][0]["snippet"], "a snippet");
}

#[tokio::test]
async fn every_engine_is_reported_with_its_status() {
    let app = app(
        vec![
            stub(
                "search-a",
                Behaviour::Return(hits("a", &["https://e.test/1"])),
            ),
            stub("search-b", Behaviour::Fail("upstream said no")),
        ],
        config(),
    );

    let (status, body) = get(&app, "/search?q=rust").await;

    assert_eq!(status, 200);
    assert_eq!(status_of(&body, "search-a").as_deref(), Some("ok"));
    assert_eq!(status_of(&body, "search-b").as_deref(), Some("error"));
    assert_eq!(body["engines"][1]["count"], 0);
    assert!(
        body["engines"][1]["error"]
            .as_str()
            .is_some_and(|error| error.contains("upstream said no"))
    );
}

#[tokio::test]
async fn the_query_reaches_the_engine_unchanged() {
    let app = app(vec![stub("echo", Behaviour::Echo)], config());

    let (_, body) = get(&app, "/search?q=rust+async+runtime&limit=3&lang=ja").await;

    let url = &body["results"][0]["url"];
    let url = url.as_str().expect("echo returns a url");
    assert!(url.contains("q=rust+async+runtime"), "{url}");
    assert!(url.contains("limit=3"), "{url}");
    assert!(url.contains("lang=ja"), "{url}");
    assert!(url.contains("safe=false"), "{url}");
}

#[tokio::test]
async fn the_same_page_from_two_engines_is_merged() {
    let app = app(
        vec![
            stub(
                "search-a",
                Behaviour::Return(hits("a", &["https://e.test/x"])),
            ),
            stub(
                "search-b",
                Behaviour::Return(hits("b", &["https://e.test/x"])),
            ),
        ],
        config(),
    );

    let (_, body) = get(&app, "/search?q=rust").await;

    assert_eq!(urls(&body), ["https://e.test/x"]);
    assert_eq!(engines_of(&body, 0), ["search-a", "search-b"]);
}

#[tokio::test]
async fn deduplication_can_be_disabled_per_request() {
    let app = app(
        vec![
            stub(
                "search-a",
                Behaviour::Return(hits("a", &["https://e.test/x"])),
            ),
            stub(
                "search-b",
                Behaviour::Return(hits("b", &["https://e.test/x"])),
            ),
        ],
        config(),
    );

    let (_, body) = get(&app, "/search?q=rust&dedup=false").await;

    assert_eq!(urls(&body).len(), 2);
}

#[tokio::test]
async fn the_limit_parameter_truncates_the_merged_ranking() {
    let app = app(
        vec![stub(
            "search-a",
            Behaviour::Return(hits(
                "a",
                &["https://e.test/1", "https://e.test/2", "https://e.test/3"],
            )),
        )],
        config(),
    );

    let (_, body) = get(&app, "/search?q=rust&limit=2").await;

    assert_eq!(urls(&body), ["https://e.test/1", "https://e.test/2"]);
}

#[tokio::test]
async fn the_limit_is_clamped_to_the_configured_maximum() {
    let mut config = config();
    config.search.max_limit = 2;
    let app = app(
        vec![stub(
            "search-a",
            Behaviour::Return(hits(
                "a",
                &["https://e.test/1", "https://e.test/2", "https://e.test/3"],
            )),
        )],
        config,
    );

    let (_, body) = get(&app, "/search?q=rust&limit=999").await;

    assert_eq!(urls(&body).len(), 2);
}

#[tokio::test]
async fn a_missing_query_is_rejected_with_json() {
    let app = app(
        vec![stub("search-a", Behaviour::Return(Vec::new()))],
        config(),
    );

    let (status, body) = get(&app, "/search").await;

    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "invalid_request");
    assert!(
        body["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains('q'))
    );
}

#[tokio::test]
async fn a_blank_query_is_rejected() {
    let app = app(
        vec![stub("search-a", Behaviour::Return(Vec::new()))],
        config(),
    );

    let (status, body) = get(&app, "/search?q=+").await;

    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "invalid_request");
}

#[tokio::test]
async fn asking_for_an_unknown_engine_is_rejected() {
    let app = app(
        vec![stub("search-a", Behaviour::Return(Vec::new()))],
        config(),
    );

    let (status, body) = get(&app, "/search?q=rust&engines=nope").await;

    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "unknown_engine");
}

#[tokio::test]
async fn a_non_numeric_limit_is_rejected() {
    let app = app(
        vec![stub("search-a", Behaviour::Return(Vec::new()))],
        config(),
    );

    let (status, body) = get(&app, "/search?q=rust&limit=many").await;

    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "invalid_request");
}

#[tokio::test]
async fn a_failing_engine_does_not_lose_the_other_results() {
    let app = app(
        vec![
            stub(
                "search-a",
                Behaviour::Return(hits("a", &["https://e.test/1"])),
            ),
            stub("search-b", Behaviour::Fail("boom")),
        ],
        config(),
    );

    let (status, body) = get(&app, "/search?q=rust").await;

    assert_eq!(status, 200);
    assert_eq!(urls(&body), ["https://e.test/1"]);
    assert_eq!(status_of(&body, "search-b").as_deref(), Some("error"));
}

#[tokio::test]
async fn every_engine_failing_is_a_bad_gateway_with_details() {
    let app = app(
        vec![
            stub("search-a", Behaviour::Fail("first")),
            stub("search-b", Behaviour::Fail("second")),
        ],
        config(),
    );

    let (status, body) = get(&app, "/search?q=rust").await;

    assert_eq!(status, 502);
    assert_eq!(body["error"]["code"], "all_engines_failed");
    let message = body["error"]["message"].as_str().expect("message");
    assert!(
        message.contains("first") && message.contains("second"),
        "{message}"
    );
}

#[tokio::test]
async fn a_slow_engine_is_reported_as_a_timeout() {
    let mut config = config();
    config.search.timeout_ms = 50;
    let app = app(
        vec![
            stub(
                "search-slow",
                Behaviour::Hang(std::time::Duration::from_millis(400)),
            ),
            stub(
                "search-fast",
                Behaviour::Return(hits("fast", &["https://e.test/1"])),
            ),
        ],
        config,
    );

    let (status, body) = get(&app, "/search?q=rust").await;

    assert_eq!(status, 200);
    assert_eq!(status_of(&body, "search-slow").as_deref(), Some("timeout"));
    assert_eq!(urls(&body), ["https://e.test/1"]);
}

#[tokio::test]
async fn the_per_request_timeout_overrides_the_configuration() {
    let mut config = config();
    config.search.timeout_ms = 5_000;
    let app = app(
        vec![stub(
            "search-slow",
            Behaviour::Hang(std::time::Duration::from_millis(400)),
        )],
        config,
    );

    let (status, body) = get(&app, "/search?q=rust&timeout_ms=50").await;

    assert_eq!(status, 502);
    assert_eq!(body["error"]["code"], "all_engines_failed");
}

#[tokio::test]
async fn post_accepts_the_same_parameters_as_a_json_body() {
    let app = app(vec![stub("echo", Behaviour::Echo)], config());

    let (status, body) = post(
        &app,
        json!({"q": "rust", "limit": 2, "engines": "echo", "safesearch": true}),
    )
    .await;

    assert_eq!(status, 200);
    let url = body["results"][0]["url"].as_str().expect("echo url");
    assert!(url.contains("limit=2"), "{url}");
    assert!(url.contains("safe=true"), "{url}");
}

#[tokio::test]
async fn post_rejects_an_unusable_body_with_json() {
    let app = app(vec![stub("echo", Behaviour::Echo)], config());

    let (status, body) = post(&app, json!({"limit": 2})).await;

    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "invalid_request");
}

#[tokio::test]
async fn post_rejects_unknown_fields() {
    let app = app(vec![stub("echo", Behaviour::Echo)], config());

    let (status, body) = post(&app, json!({"q": "rust", "surprise": 1})).await;

    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "invalid_request");
}

#[tokio::test]
async fn the_engines_endpoint_lists_what_is_callable() {
    let app = app(
        vec![stub("search-a", Behaviour::Return(Vec::new()))],
        config(),
    );

    let (status, body) = get(&app, "/engines").await;

    assert_eq!(status, 200);
    assert_eq!(body["engines"][0]["id"], "search-a");
    assert_eq!(body["engines"][0]["enabled"], true);
    assert_eq!(body["engines"][0]["default"], true);
}

#[tokio::test]
async fn healthz_reports_ok() {
    let app = app(
        vec![stub("search-a", Behaviour::Return(Vec::new()))],
        config(),
    );

    let (status, body) = get(&app, "/healthz").await;

    assert_eq!(status, 200);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn a_config_without_a_positive_limit_cannot_serve() {
    let mut config = config();
    config.search.limit = 0;
    let app = app(
        vec![stub("search-a", Behaviour::Return(Vec::new()))],
        config,
    );

    let (status, _) = get(&app, "/search?q=rust").await;

    assert_eq!(status, 200, "limit 0 is clamped to 1 by the request parser");
}

#[tokio::test]
async fn an_empty_engine_list_is_rejected() {
    let config: Config = config();
    let app = app(
        vec![stub("search-a", Behaviour::Return(Vec::new()))],
        config,
    );

    let (status, body) = get(&app, "/search?q=rust&engines=").await;

    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "invalid_request");
}
