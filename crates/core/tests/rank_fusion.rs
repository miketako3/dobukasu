//! How per-engine rankings become one ranking.

use dobukasu_core::{EngineHits, Hit, fuse};

fn source(engine: &str, urls: &[&str]) -> EngineHits {
    EngineHits::new(
        engine,
        urls.iter()
            .enumerate()
            .map(|(index, url)| Hit::new(format!("{engine} #{}", index + 1), *url, ""))
            .collect(),
    )
}

#[test]
fn one_engine_keeps_its_own_order() {
    let merged = fuse(
        &[source(
            "a",
            &["https://e.test/1", "https://e.test/2", "https://e.test/3"],
        )],
        10,
        true,
    );

    assert_eq!(
        merged
            .iter()
            .map(|hit| hit.url.as_str())
            .collect::<Vec<_>>(),
        ["https://e.test/1", "https://e.test/2", "https://e.test/3"]
    );
    assert_eq!(merged[0].engines, ["a"]);
}

#[test]
fn agreement_between_engines_outranks_depth_in_one() {
    let merged = fuse(
        &[
            source("a", &["https://e.test/deep", "https://e.test/shared"]),
            source("b", &["https://e.test/shared", "https://e.test/other"]),
        ],
        10,
        true,
    );

    assert_eq!(merged[0].url, "https://e.test/shared");
    assert_eq!(merged[0].engines, ["a", "b"]);
    assert_eq!(merged.len(), 3, "the duplicate must collapse");
}

#[test]
fn the_best_ranked_occurrence_supplies_the_title() {
    let merged = fuse(
        &[
            EngineHits::new(
                "a",
                vec![Hit::new("first from a", "https://e.test/x", "a snippet")],
            ),
            EngineHits::new(
                "b",
                vec![
                    Hit::new("other", "https://e.test/y", ""),
                    Hit::new("second from b", "https://e.test/x", "b snippet"),
                ],
            ),
        ],
        10,
        true,
    );

    assert_eq!(merged[0].title, "first from a");
    assert_eq!(merged[0].snippet, "a snippet");
    assert_eq!(merged[0].url, "https://e.test/x");
}

#[test]
fn a_missing_snippet_is_filled_from_another_engine() {
    let merged = fuse(
        &[
            EngineHits::new("a", vec![Hit::new("title", "https://e.test/x", "")]),
            EngineHits::new(
                "b",
                vec![Hit::new("title", "https://e.test/x", "b snippet")],
            ),
        ],
        10,
        true,
    );

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].snippet, "b snippet");
}

#[test]
fn tracking_parameters_do_not_create_duplicates() {
    let merged = fuse(
        &[
            source("a", &["https://e.test/page?utm_source=newsletter&id=7"]),
            source("b", &["https://e.test/page?id=7"]),
        ],
        10,
        true,
    );

    assert_eq!(merged.len(), 1, "{merged:#?}");
    assert_eq!(merged[0].engines, ["a", "b"]);
}

#[test]
fn deduplication_can_be_switched_off() {
    let merged = fuse(
        &[
            source("a", &["https://e.test/x"]),
            source("b", &["https://e.test/x"]),
        ],
        10,
        false,
    );

    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].engines, ["a"]);
    assert_eq!(merged[1].engines, ["b"]);
}

#[test]
fn the_limit_truncates_after_ranking() {
    let merged = fuse(
        &[source(
            "a",
            &["https://e.test/1", "https://e.test/2", "https://e.test/3"],
        )],
        2,
        true,
    );

    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].url, "https://e.test/1");
}

#[test]
fn hits_without_a_title_or_url_are_dropped() {
    let merged = fuse(
        &[EngineHits::new(
            "a",
            vec![
                Hit::new("", "https://e.test/no-title", ""),
                Hit::new("no url", "", ""),
                Hit::new("kept", "https://e.test/kept", ""),
            ],
        )],
        10,
        true,
    );

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].url, "https://e.test/kept");
}

#[test]
fn the_documented_reciprocal_rank_fusion_score_is_reported() {
    // 1 / (60 + rank), the RRF constant from the original publication.
    let merged = fuse(
        &[source("a", &["https://e.test/1", "https://e.test/2"])],
        10,
        true,
    );

    assert!(
        (merged[0].score - 1.0 / 61.0).abs() < 1e-12,
        "{:?}",
        merged[0]
    );
    assert!(
        (merged[1].score - 1.0 / 62.0).abs() < 1e-12,
        "{:?}",
        merged[1]
    );
}

#[test]
fn equal_evidence_is_ordered_deterministically() {
    let sources = || {
        [
            source("a", &["https://e.test/a"]),
            source("b", &["https://e.test/b"]),
        ]
    };

    let first = fuse(&sources(), 10, true);
    let second = fuse(&sources(), 10, true);

    assert_eq!(
        first.iter().map(|hit| hit.url.as_str()).collect::<Vec<_>>(),
        second
            .iter()
            .map(|hit| hit.url.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn no_sources_means_no_results() {
    assert_eq!(fuse(&[], 10, true).len(), 0);
}
