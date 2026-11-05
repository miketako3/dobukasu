//! `DuckDuckGo`'s contract with its upstream HTML.
//!
//! `results.html` is a captured SERP for "rust programming language";
//! `throttled.html` is the block page `DuckDuckGo` serves when it rate-limits.

use dobukasu_engine_duckduckgo::parse;
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

    assert_eq!(hits.len(), 10, "every organic result is kept");
    assert_eq!(hits[0].title, "Rust Programming Language");
    assert_eq!(hits[0].url, "https://rust-lang.org/");
    assert!(
        hits[0]
            .snippet
            .starts_with("A language empowering everyone"),
        "{:?}",
        hits[0]
    );
    assert_eq!(
        hits[1].url,
        "https://en.wikipedia.org/wiki/Rust_(programming_language)"
    );
}

#[test]
fn redirect_links_are_resolved_to_their_target() {
    let hits = parse(&fixture("results.html"));

    for hit in &hits {
        assert!(
            !hit.url.contains("duckduckgo.com"),
            "a redirect leaked into the results: {}",
            hit.url
        );
        assert!(hit.url.starts_with("https://"), "{}", hit.url);
    }
}

#[test]
fn a_throttled_page_yields_no_hits() {
    assert_eq!(parse(&fixture("throttled.html")).len(), 0);
}

#[test]
fn sponsored_blocks_are_skipped() {
    let html = r#"
        <div class="result result--ad">
          <h2><a class="result__a" href="https://advert.test/">Buy something</a></h2>
        </div>
        <div class="result">
          <h2><a class="result__a"
                 href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.test%2Fpage">Example</a></h2>
          <a class="result__snippet">A snippet</a>
        </div>
    "#;

    let hits = parse(html);

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].url, "https://example.test/page");
    assert_eq!(hits[0].snippet, "A snippet");
}

#[test]
fn a_result_without_a_snippet_is_still_reported() {
    let html = r#"
        <div class="result">
          <h2><a class="result__a" href="https://example.test/">Title only</a></h2>
        </div>
    "#;

    let hits = parse(html);

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].snippet, "");
}

#[test]
fn results_without_a_link_are_dropped() {
    let html = r#"
        <div class="result"><h2><a class="result__a">No href</a></h2></div>
        <div class="result"><h2><a class="result__a" href="https://example.test/">Fine</a></h2></div>
    "#;

    let hits = parse(html);

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].url, "https://example.test/");
}
