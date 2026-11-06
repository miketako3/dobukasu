//! Brave's contract with its JSON API.

use std::path::Path;

use dobukasu_engine_brave::parse;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(path).expect("fixture")
}

#[test]
fn a_response_yields_the_ranked_hits() {
    let hits = parse(&fixture("results.json")).expect("valid payload");

    assert_eq!(hits.len(), 2, "{hits:#?}");
    assert_eq!(hits[0].url, "https://tokio.rs/");
    assert_eq!(
        hits[0].title, "Tokio — an asynchronous Rust runtime",
        "markup inside the title is not part of the title"
    );
    assert_eq!(
        hits[0].snippet,
        "Tokio is a runtime for writing reliable network applications & more."
    );
    assert_eq!(hits[1].url, "https://rust-lang.github.io/async-book/");
}

#[test]
fn a_result_without_a_url_is_dropped() {
    let hits = parse(&fixture("results.json")).expect("valid payload");

    assert!(hits.iter().all(|hit| hit.url != "No link here"));
    assert!(hits.iter().all(|hit| !hit.title.is_empty()));
}

#[test]
fn a_payload_without_a_web_section_is_an_empty_result() {
    let hits = parse(r#"{"type": "search"}"#).expect("valid payload");

    assert_eq!(hits.len(), 0);
}

#[test]
fn a_payload_that_is_not_json_is_a_protocol_error() {
    let error = parse("<html>nope</html>").expect_err("must not accept html");

    assert!(error.to_string().contains("unusable page"), "{error}");
}
