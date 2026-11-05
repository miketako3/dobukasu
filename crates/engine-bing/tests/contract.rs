//! Bing's contract with its upstream HTML.
//!
//! `results.html` is a captured SERP for "rust programming language". Bing
//! wraps every organic link in a `bing.com/ck/a?u=a1<base64>` redirect.

use dobukasu_engine_bing::parse;
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
    assert!(hits[0].snippet.contains("blazingly fast"), "{:?}", hits[0]);
}

#[test]
fn redirect_links_are_decoded() {
    let hits = parse(&fixture("results.html"));

    for hit in &hits {
        assert!(
            !hit.url.contains("bing.com/ck/a"),
            "a redirect leaked into the results: {}",
            hit.url
        );
    }
}

#[test]
fn the_advertised_redirect_shape_is_decoded() {
    // "https://example.test/page" in base64url, as Bing encodes it.
    let html = r#"
        <ol>
          <li class="b_algo">
            <h2><a href="https://www.bing.com/ck/a?!&&p=abc&u=a1aHR0cHM6Ly9leGFtcGxlLnRlc3QvcGFnZQ&ntb=1">Example</a></h2>
            <div class="b_caption"><p>Caption text</p></div>
          </li>
        </ol>
    "#;

    let hits = parse(html);

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].url, "https://example.test/page");
    assert_eq!(hits[0].snippet, "Caption text");
}

#[test]
fn direct_links_are_kept() {
    let html = r#"
        <li class="b_algo">
          <h2><a href="https://example.test/direct">Direct</a></h2>
          <p class="b_lineclamp2">Snippet</p>
        </li>
    "#;

    let hits = parse(html);

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].url, "https://example.test/direct");
}

#[test]
fn results_outside_bing_and_without_a_title_are_dropped() {
    let html = r#"
        <li class="b_algo"><h2><a href="/relative">Relative</a></h2></li>
        <li class="b_algo"><h2><a>No link</a></h2></li>
        <li class="b_ad b_algo"><h2><a href="https://advert.test/">Ad</a></h2></li>
        <li class="b_algo"><h2><a href="https://example.test/">Kept</a></h2></li>
    "#;

    let hits = parse(html);

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].url, "https://example.test/");
}
