//! Google's contract with its two upstreams: the WML page and the Custom
//! Search JSON API.
//!
//! Google answers this host with 403 or a JavaScript interstitial instead of a
//! results page, so `wml.html` is a shape fixture built from the layout the
//! WML backend targets (the same selectors `SearXNG` parses), while
//! `blocked.html` is the real interstitial that Google serves.

use std::path::Path;

use dobukasu_engine_google::{is_blocked_page, parse, parse_cse};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(path).expect("fixture")
}

#[test]
fn a_wml_page_yields_the_ranked_hits() {
    let hits = parse(&fixture("wml.html"));

    assert_eq!(hits.len(), 3, "{hits:#?}");
    assert_eq!(hits[0].title, "Tokio — an asynchronous Rust runtime");
    assert_eq!(hits[0].url, "https://tokio.rs/");
    assert_eq!(
        hits[0].snippet,
        "Tokio is a runtime for writing reliable network applications without the boilerplate."
    );
    assert_eq!(hits[1].url, "https://docs.rs/tokio/latest/tokio/");
    assert_eq!(hits[2].url, "https://tokio.rs/tokio/tutorial");
    assert_eq!(hits[2].snippet, "");
}

#[test]
fn a_result_without_a_destination_is_dropped() {
    let hits = parse(&fixture("wml.html"));

    assert!(
        hits.iter().all(|hit| !hit.title.contains("nowhere to go")),
        "{hits:#?}"
    );
}

#[test]
fn markup_outside_the_result_container_is_not_a_hit() {
    let hits = parse(&fixture("wml.html"));

    assert!(
        hits.iter()
            .all(|hit| hit.url != "https://example.com/not-wml")
    );
}

#[test]
fn the_javascript_interstitial_is_recognised() {
    assert!(is_blocked_page(&fixture("blocked.html")));
    assert!(!is_blocked_page(&fixture("wml.html")));
}

#[test]
fn a_forbidden_page_is_recognised() {
    assert!(is_blocked_page("<title>Error 403 (Forbidden)!!1</title>"));
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
