//! What counts as "the same page" when rankings are merged.

use dobukasu_core::canonical_url;

/// (input, expected identity key)
const CASES: &[(&str, &str)] = &[
    ("https://example.com", "https://example.com/"),
    ("https://example.com/", "https://example.com/"),
    ("https://www.example.com/a", "https://example.com/a"),
    ("https://EXAMPLE.com/A", "https://example.com/A"),
    ("https://example.com/a/", "https://example.com/a"),
    ("https://example.com/a#section", "https://example.com/a"),
    (
        "https://example.com/a?utm_source=news&utm_medium=mail",
        "https://example.com/a",
    ),
    ("https://example.com/a?gclid=123", "https://example.com/a"),
    (
        "https://example.com/a?b=2&a=1",
        "https://example.com/a?a=1&b=2",
    ),
    (
        "https://example.com/a?a=1&utm_campaign=x&b=2",
        "https://example.com/a?a=1&b=2",
    ),
    ("https://example.com/a?a=1&a=1", "https://example.com/a?a=1"),
    ("  https://example.com/a  ", "https://example.com/a"),
    // Different schemes and paths stay distinct: they can serve different bodies.
    ("http://example.com/a", "http://example.com/a"),
    ("https://example.com/a?x=1", "https://example.com/a?x=1"),
    ("https://example.com/a?x=2", "https://example.com/a?x=2"),
    ("https://example.com/a/b", "https://example.com/a/b"),
    // Non-http schemes and unparseable input are passed through.
    ("mailto:hello@example.com", "mailto:hello@example.com"),
    ("not a url", "not a url"),
];

#[test]
fn urls_that_point_at_the_same_page_share_one_identity() {
    for (input, expected) in CASES {
        assert_eq!(canonical_url(input), *expected, "input: {input}");
    }
}

#[test]
fn identity_is_stable_when_applied_twice() {
    for (input, _) in CASES {
        let once = canonical_url(input);
        assert_eq!(canonical_url(&once), once, "input: {input}");
    }
}
