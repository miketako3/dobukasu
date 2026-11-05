//! Google's contract with its two upstreams: the HTML page and the Custom
//! Search JSON API.
//!
//! Google serves a JavaScript interstitial instead of results to most
//! non-browser clients, so `results.html` is a shape fixture built from the
//! markup the HTML backend targets, while `blocked.html` is the real
//! interstitial.

use dobukasu_engine_google::{is_blocked_page, parse, parse_cse};
use std::path::Path;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(path).expect("fixture")
}

#[test]
fn a_results_page_yields_the_ranked_hits() {
    let hits = parse(&fixture("results.html"));

    assert_eq!(hits.len(), 4, "{hits:#?}");
    assert_eq!(hits[0].title, "Rust Programming Language");
    assert_eq!(hits[0].url, "https://rust-lang.org/");
    assert_eq!(
        hits[0].snippet,
        "A language empowering everyone to build reliable and efficient software."
    );
    assert_eq!(
        hits[1].url,
        "https://en.wikipedia.org/wiki/Rust_(programming_language)"
    );
    assert_eq!(hits[2].url, "https://doc.rust-lang.org/book/");
    assert_eq!(
        hits[2].snippet,
        "The book is the primary learning resource."
    );
}

#[test]
fn the_same_link_is_reported_once() {
    let hits = parse(&fixture("results.html"));

    assert_eq!(
        hits.iter()
            .filter(|hit| hit.url == "https://duplicate.test/page")
            .count(),
        1
    );
}

#[test]
fn navigation_links_are_not_results() {
    let hits = parse(&fixture("results.html"));

    assert!(hits.iter().all(|hit| !hit.url.contains("/preferences")));
}

#[test]
fn the_javascript_interstitial_is_recognised() {
    assert!(is_blocked_page(&fixture("blocked.html")));
    assert!(!is_blocked_page(&fixture("results.html")));
}

#[test]
fn custom_search_items_become_hits() {
    let hits = parse_cse(&fixture("cse.json")).expect("valid payload");

    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].url, "https://rust-lang.org/");
    assert_eq!(hits[1].title, "Documentation");
}

#[test]
fn a_payload_without_items_is_an_empty_result() {
    let hits = parse_cse(r#"{"kind": "customsearch#search"}"#).expect("valid payload");

    assert_eq!(hits.len(), 0);
}

#[test]
fn a_payload_that_is_not_json_is_a_protocol_error() {
    let error = parse_cse("<html>not json</html>").expect_err("must not accept html");

    assert!(error.to_string().contains("unusable page"), "{error}");
}
