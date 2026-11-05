//! `DuckDuckGo` engine plug-in (keyless HTML endpoint).

use std::sync::Arc;

use async_trait::async_trait;
use dobukasu_core::{Engine, EngineContext, EngineError, Hit, Plugin, SearchQuery};
use percent_encoding::percent_decode_str;
use scraper::{ElementRef, Html, Selector};
use url::Url;

pub const ID: &str = "duckduckgo";

inventory::submit! {
    Plugin {
        id: ID,
        label: "DuckDuckGo",
        description: "Keyless. Uses the html.duckduckgo.com endpoint.",
        enabled_by_default: true,
        build,
    }
}

#[allow(clippy::unnecessary_wraps)]
fn build() -> Result<Arc<dyn Engine>, EngineError> {
    Ok(Arc::new(DuckDuckGo))
}

#[derive(Debug, Default)]
pub struct DuckDuckGo;

#[async_trait]
impl Engine for DuckDuckGo {
    fn id(&self) -> &'static str {
        ID
    }

    async fn search(
        &self,
        query: &SearchQuery,
        context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError> {
        let mut url = Url::parse("https://html.duckduckgo.com/html/")
            .map_err(|error| EngineError::Config(error.to_string()))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", &query.text);
            pairs.append_pair("kp", if query.safe_search { "1" } else { "-1" });
            if let Some(region) = region_code(query) {
                pairs.append_pair("kl", &region);
            }
            if let Some(range) = query.time_range {
                let df = match range {
                    dobukasu_core::TimeRange::Day => "d",
                    dobukasu_core::TimeRange::Week => "w",
                    dobukasu_core::TimeRange::Month => "m",
                    dobukasu_core::TimeRange::Year => "y",
                };
                pairs.append_pair("df", df);
            }
        }

        let html = context.fetch_text(url).await?;
        if is_throttled(&html) {
            return Err(EngineError::Blocked(
                "DuckDuckGo rate-limited this client (anomaly challenge)".to_owned(),
            ));
        }
        Ok(parse(&html))
    }
}

fn region_code(query: &SearchQuery) -> Option<String> {
    let region = query.region.as_deref()?.to_ascii_lowercase();
    let language = query.lang.as_deref().map_or_else(
        || "en".to_owned(),
        |lang| lang.split('-').next().unwrap_or(lang).to_ascii_lowercase(),
    );
    Some(format!("{region}-{language}"))
}

/// Whether this page is the challenge `DuckDuckGo` serves when it rate-limits
/// a client. Reported as a failure rather than an empty result set, because
/// "no hits" and "we were turned away" are different answers.
#[must_use]
pub fn is_throttled(html: &str) -> bool {
    html.contains("anomaly-modal") || html.contains("bots use DuckDuckGo too")
}

/// Parse a `DuckDuckGo` HTML results page. Public because it is the engine's
/// contract with its upstream: given this markup, these hits.
#[must_use]
pub fn parse(html: &str) -> Vec<Hit> {
    let document = Html::parse_document(html);
    let Ok(result) = Selector::parse("div.result") else {
        return Vec::new();
    };
    let Ok(title_link) = Selector::parse("a.result__a") else {
        return Vec::new();
    };
    let Ok(snippet) = Selector::parse("a.result__snippet, .result__snippet") else {
        return Vec::new();
    };

    document
        .select(&result)
        .filter(|node| !is_ad(node))
        .filter_map(|node| {
            let link = node.select(&title_link).next()?;
            let title = text_of(link);
            let url = resolve(link.value().attr("href")?)?;
            let snippet = node
                .select(&snippet)
                .next()
                .map(text_of)
                .unwrap_or_default();
            Some(Hit::new(title, url, snippet))
        })
        .filter(Hit::is_complete)
        .collect()
}

fn is_ad(node: &ElementRef<'_>) -> bool {
    node.value().classes().any(|class| class == "result--ad")
}

fn text_of(node: ElementRef<'_>) -> String {
    node.text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn resolve(href: &str) -> Option<String> {
    let normalized = if let Some(rest) = href.strip_prefix("//") {
        format!("https:{rest}")
    } else {
        href.to_owned()
    };
    let url = Url::parse(&normalized).ok()?;
    if url
        .host_str()
        .is_some_and(|host| host.ends_with("duckduckgo.com"))
        && url.path().starts_with("/l/")
    {
        let target = url
            .query_pairs()
            .find(|(key, _)| key == "uddg")
            .map(|(_, value)| percent_decode_str(&value).decode_utf8_lossy().into_owned())?;
        return Some(target);
    }
    Some(url.to_string())
}
