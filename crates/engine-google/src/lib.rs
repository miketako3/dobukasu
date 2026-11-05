//! Google engine plug-in.
//!
//! Two keyless-hostile backends share one id:
//!
//! * the WML endpoint `www.google.com/wml/search`, requested with a Symbian
//!   Nokia user agent — this is what `SearXNG` does, and it is the only scraped
//!   layout Google still serves without JavaScript;
//! * the official Custom Search JSON API when `GOOGLE_API_KEY` and `GOOGLE_CX`
//!   are set.
//!
//! Google blocks clients it does not like (403, or a page that says "enable
//! JavaScript"), so the scraped backend reports [`EngineError::Blocked`] rather
//! than pretending it found nothing.

use std::sync::Arc;

use async_trait::async_trait;
use dobukasu_core::{Engine, EngineContext, EngineError, Hit, Plugin, SearchQuery};
use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;
use url::Url;

pub const ID: &str = "google";

/// User agents that make Google serve the WML layout. Rotated like `SearXNG`.
const NOKIA_AGENTS: [&str; 5] = [
    "Nokia7610/2.0 (5.0509.0) SymbianOS/7.0s Series60/2.1 Profile/MIDP-2.0 Configuration/CLDC-1.0",
    "Nokia6230/2.0 (05.50) Profile/MIDP-2.0 Configuration/CLDC-1.1",
    "Nokia6230i/2.0 (03.80) Profile/MIDP-2.0 Configuration/CLDC-1.1",
    "Nokia6280/2.0 (03.60) Profile/MIDP-2.0 Configuration/CLDC-1.1",
    "NokiaN72/2.0617.1.0.3 Series60/2.8 Profile/MIDP-2.0 Configuration/CLDC-1.1",
];

inventory::submit! {
    Plugin {
        id: ID,
        label: "Google",
        description: "WML endpoint by default; Custom Search JSON API when GOOGLE_API_KEY and GOOGLE_CX are set.",
        enabled_by_default: true,
        build,
    }
}

pub struct Google {
    api_key: Option<String>,
    cx: Option<String>,
}

#[allow(clippy::unnecessary_wraps)]
fn build() -> Result<Arc<dyn Engine>, EngineError> {
    Ok(Arc::new(Google {
        api_key: env_value("GOOGLE_API_KEY"),
        cx: env_value("GOOGLE_CX"),
    }))
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

impl Google {
    fn uses_cse(&self) -> bool {
        self.api_key.is_some() && self.cx.is_some()
    }
}

#[async_trait]
impl Engine for Google {
    fn id(&self) -> &'static str {
        ID
    }

    async fn search(
        &self,
        query: &SearchQuery,
        context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError> {
        if self.uses_cse() {
            self.search_cse(query, context).await
        } else {
            self.search_wml(query, context).await
        }
    }
}

impl Google {
    async fn search_wml(
        &self,
        query: &SearchQuery,
        context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError> {
        let mut url = Url::parse("https://www.google.com/wml/search")?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", &query.text);
            pairs.append_pair("sca_esv", "1");
            if let Some(language) = query.lang.as_deref() {
                pairs.append_pair("hl", language);
            }
            if let Some(region) = query.region.as_deref() {
                pairs.append_pair("gl", region);
            }
            pairs.append_pair("safe", if query.safe_search { "active" } else { "off" });
            if let Some(range) = query.time_range {
                let span = match range {
                    dobukasu_core::TimeRange::Day => "d",
                    dobukasu_core::TimeRange::Week => "w",
                    dobukasu_core::TimeRange::Month => "m",
                    dobukasu_core::TimeRange::Year => "y",
                };
                pairs.append_pair("tbs", &format!("qdr:{span}"));
            }
        }

        let agent = NOKIA_AGENTS[query.text.len() % NOKIA_AGENTS.len()];
        let body = context.fetch_text_as(url, agent).await?;
        if is_blocked_page(&body) {
            return Err(EngineError::Blocked(
                "Google refused a JavaScript-free request; set GOOGLE_API_KEY and GOOGLE_CX for the Custom Search backend"
                    .to_owned(),
            ));
        }
        Ok(parse(&body))
    }

    async fn search_cse(
        &self,
        query: &SearchQuery,
        context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError> {
        let api_key = self
            .api_key
            .as_deref()
            .ok_or_else(|| EngineError::Config("missing GOOGLE_API_KEY".to_owned()))?;
        let cx = self
            .cx
            .as_deref()
            .ok_or_else(|| EngineError::Config("missing GOOGLE_CX".to_owned()))?;

        let mut url = Url::parse("https://www.googleapis.com/customsearch/v1")?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("key", api_key);
            pairs.append_pair("cx", cx);
            pairs.append_pair("q", &query.text);
            pairs.append_pair("num", &query.limit.min(10).to_string());
            if let Some(language) = query.lang.as_deref() {
                pairs.append_pair("hl", language);
            }
            pairs.append_pair("safe", if query.safe_search { "active" } else { "off" });
            if let Some(range) = query.time_range {
                let restrict = match range {
                    dobukasu_core::TimeRange::Day => "d1",
                    dobukasu_core::TimeRange::Week => "w1",
                    dobukasu_core::TimeRange::Month => "m1",
                    dobukasu_core::TimeRange::Year => "y1",
                };
                pairs.append_pair("dateRestrict", restrict);
            }
        }

        let body = context.fetch_text(url).await?;
        parse_cse(&body)
    }
}

/// Does this page say "run JavaScript" or "you are a robot"?
#[must_use]
pub fn is_blocked_page(html: &str) -> bool {
    html.contains("enablejs")
        || html.contains("unusual traffic")
        || html.contains("/httpservice/retry/")
        || html.contains("Error 403")
}

/// Parse the WML results layout.
///
/// The selectors are the contract `SearXNG` parses too: `div.zMzFAb` per result,
/// `a.fuLhoc` around the link, `span.CVA68e` around the title and
/// `div.taTFJ span.FrIlee` around the snippet.
#[must_use]
pub fn parse(html: &str) -> Vec<Hit> {
    let document = Html::parse_document(html);
    let Ok(container) = Selector::parse("div.zMzFAb") else {
        return Vec::new();
    };
    let Ok(link) = Selector::parse("a.fuLhoc") else {
        return Vec::new();
    };
    let Ok(heading) = Selector::parse("span.CVA68e") else {
        return Vec::new();
    };
    let Ok(snippet) = Selector::parse("div.taTFJ span.FrIlee") else {
        return Vec::new();
    };

    let mut hits = Vec::new();
    for node in document.select(&container) {
        let Some(anchor) = node.select(&link).next() else {
            continue;
        };
        let Some(href) = anchor.value().attr("href").and_then(resolve) else {
            continue;
        };
        let title = node
            .select(&heading)
            .next()
            .map(text_of)
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| text_of(anchor));
        let snippet = node
            .select(&snippet)
            .next()
            .map(text_of)
            .unwrap_or_default();
        let hit = Hit::new(title, href, snippet);
        if hit.is_complete() {
            hits.push(hit);
        }
    }
    dedup(&mut hits);
    hits
}

/// Parse a Custom Search JSON API response.
pub fn parse_cse(body: &str) -> Result<Vec<Hit>, EngineError> {
    #[derive(Deserialize)]
    struct Response {
        #[serde(default)]
        items: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        #[serde(default)]
        title: String,
        #[serde(default)]
        link: String,
        #[serde(default)]
        snippet: String,
    }

    let response: Response =
        serde_json::from_str(body).map_err(|error| EngineError::Protocol(error.to_string()))?;
    Ok(response
        .items
        .into_iter()
        .map(|item| Hit::new(item.title, item.link, item.snippet))
        .filter(Hit::is_complete)
        .collect())
}

/// Turn a Google href into the page it points at: absolute, or `/url?q=…`.
fn resolve(href: &str) -> Option<String> {
    let href = href.trim();
    if href.starts_with("http://") || href.starts_with("https://") {
        let url = Url::parse(href).ok()?;
        return matches!(url.scheme(), "http" | "https").then(|| url.to_string());
    }
    if !href.starts_with('/') {
        return None;
    }
    let absolute = Url::parse(&format!("https://www.google.com{href}")).ok()?;
    let target = absolute
        .query_pairs()
        .find(|(key, _)| key == "q" || key == "url")
        .map(|(_, value)| value.into_owned())?;
    let url = Url::parse(&target).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| url.to_string())
}

fn text_of(node: ElementRef<'_>) -> String {
    node.text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn dedup(hits: &mut Vec<Hit>) {
    let mut seen = std::collections::HashSet::new();
    hits.retain(|hit| seen.insert(hit.url.clone()));
}
