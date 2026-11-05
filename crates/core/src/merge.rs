//! URL canonicalisation and rank fusion.

use std::collections::HashMap;

use url::Url;

use crate::model::{EngineHits, SearchResult};

/// Reciprocal-rank-fusion constant. 60 is the value from the original paper.
pub const RRF_K: f64 = 60.0;

const TRACKING_PARAMS: &[&str] = &[
    "dclid", "fbclid", "gclid", "gclsrc", "igshid", "mc_cid", "mc_eid", "msclkid", "ref_src",
    "s_kwcid", "spm", "twclid", "ved", "yclid", "_hsenc", "_hsmi",
];

/// Reduce a URL to the key used for de-duplication.
///
/// The transformation is deliberately conservative: scheme, path spelling and
/// every non-tracking query parameter are preserved, because two URLs that
/// differ there may serve different documents.
#[must_use]
pub fn canonical_url(raw: &str) -> String {
    let trimmed = raw.trim();
    let Ok(mut url) = Url::parse(trimmed) else {
        return trimmed.to_owned();
    };
    if !matches!(url.scheme(), "http" | "https") {
        return trimmed.to_owned();
    }
    if let Some(host) = url.host_str() {
        let host = host.strip_prefix("www.").unwrap_or(host);
        let host = host.to_ascii_lowercase();
        let _ = url.set_host(Some(&host));
    }
    url.set_fragment(None);

    let path = url.path().to_owned();
    if path.len() > 1 && path.ends_with('/') {
        url.set_path(&path[..path.len() - 1]);
    }

    let mut pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !is_tracking(key))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    pairs.sort();
    pairs.dedup();
    if pairs.is_empty() {
        url.set_query(None);
    } else {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        serializer.extend_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        url.set_query(Some(&serializer.finish()));
    }

    url.to_string()
}

fn is_tracking(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.starts_with("utm_") || TRACKING_PARAMS.contains(&lower.as_str())
}

struct Accumulated {
    url: String,
    title: String,
    snippet: String,
    engines: Vec<String>,
    score: f64,
    best_rank: usize,
    best_engine: usize,
}

/// Merge per-engine rankings into one list.
///
/// Each engine contributes `1 / (RRF_K + rank)` per hit, so agreement between
/// engines outranks depth within a single engine. When `dedup` is false every
/// occurrence is kept.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn fuse(sources: &[EngineHits], limit: usize, dedup: bool) -> Vec<SearchResult> {
    let mut accumulated: Vec<Accumulated> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    for (engine_index, source) in sources.iter().enumerate() {
        for (position, hit) in source.hits.iter().enumerate() {
            if !hit.is_complete() {
                continue;
            }
            let rank = position + 1;
            let score = 1.0 / (RRF_K + rank as f64);
            let key = if dedup {
                canonical_url(&hit.url)
            } else {
                format!("{}#{engine_index}:{position}", canonical_url(&hit.url))
            };

            if let Some(&slot) = seen.get(&key) {
                let entry = &mut accumulated[slot];
                entry.score += score;
                if !entry.engines.iter().any(|known| known == &source.engine) {
                    entry.engines.push(source.engine.clone());
                }
                let better_rank = rank < entry.best_rank;
                let same_rank_earlier_engine =
                    rank == entry.best_rank && engine_index < entry.best_engine;
                if better_rank || same_rank_earlier_engine {
                    entry.url.clone_from(&hit.url);
                    entry.title.clone_from(&hit.title);
                    entry.snippet.clone_from(&hit.snippet);
                    entry.best_rank = rank;
                    entry.best_engine = engine_index;
                } else if entry.snippet.trim().is_empty() && !hit.snippet.trim().is_empty() {
                    entry.snippet.clone_from(&hit.snippet);
                }
            } else {
                seen.insert(key, accumulated.len());
                accumulated.push(Accumulated {
                    url: hit.url.clone(),
                    title: hit.title.clone(),
                    snippet: hit.snippet.clone(),
                    engines: vec![source.engine.clone()],
                    score,
                    best_rank: rank,
                    best_engine: engine_index,
                });
            }
        }
    }

    accumulated.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.best_rank.cmp(&right.best_rank))
            .then_with(|| left.best_engine.cmp(&right.best_engine))
            .then_with(|| left.url.cmp(&right.url))
    });
    accumulated.truncate(limit);

    accumulated
        .into_iter()
        .map(|entry| SearchResult {
            title: entry.title,
            url: entry.url,
            snippet: entry.snippet,
            engines: entry.engines,
            score: entry.score,
        })
        .collect()
}
