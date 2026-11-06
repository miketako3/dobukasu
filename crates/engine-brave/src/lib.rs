//! Brave Search engine plug-in.
//!
//! Brave runs its own index and sells an official JSON API with a free tier,
//! so this is the engine that behaves the same from any IP: no scraping, no
//! decoy pages, no JavaScript interstitial. It needs `BRAVE_API_KEY`; without
//! the key the plug-in reports that at build time and is left out of the
//! registry instead of failing every search.
//!
//! API reference: <https://api-dashboard.search.brave.com/app/documentation>

use std::sync::Arc;

use async_trait::async_trait;
use dobukasu_core::{Engine, EngineContext, EngineError, Hit, Plugin, SearchQuery};
use reqwest::header::{ACCEPT, HeaderValue};
use scraper::Html;
use serde::Deserialize;
use url::Url;

pub const ID: &str = "brave";
const ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";
const KEY_ENV: &str = "BRAVE_API_KEY";

inventory::submit! {
    Plugin {
        id: ID,
        label: "Brave",
        description: "Official Brave Search API (set BRAVE_API_KEY). Reliable from any network.",
        enabled_by_default: true,
        build,
    }
}

pub struct Brave {
    key: HeaderValue,
}

fn build() -> Result<Arc<dyn Engine>, EngineError> {
    let key = std::env::var(KEY_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| EngineError::Config(format!("{KEY_ENV} is not set")))?;
    let key = HeaderValue::from_str(&key)
        .map_err(|_| EngineError::Config(format!("{KEY_ENV} is not a valid header value")))?;
    Ok(Arc::new(Brave { key }))
}

#[async_trait]
impl Engine for Brave {
    fn id(&self) -> &'static str {
        ID
    }

    async fn search(
        &self,
        query: &SearchQuery,
        context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError> {
        let mut url = Url::parse(ENDPOINT)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", &query.text);
            pairs.append_pair("count", &query.limit.clamp(1, 20).to_string());
            pairs.append_pair("result_filter", "web");
            if let Some(language) = query.lang.as_deref() {
                pairs.append_pair("search_lang", language);
            }
            if let Some(region) = query.region.as_deref() {
                pairs.append_pair("country", &region.to_ascii_uppercase());
            }
            pairs.append_pair(
                "safesearch",
                if query.safe_search { "strict" } else { "off" },
            );
            if let Some(range) = query.time_range {
                let freshness = match range {
                    dobukasu_core::TimeRange::Day => "pd",
                    dobukasu_core::TimeRange::Week => "pw",
                    dobukasu_core::TimeRange::Month => "pm",
                    dobukasu_core::TimeRange::Year => "py",
                };
                pairs.append_pair("freshness", freshness);
            }
        }

        let request = context
            .request(url)
            .header(ACCEPT, "application/json")
            .header("X-Subscription-Token", self.key.clone());
        let body = context.fetch(request).await?;
        parse(&body)
    }
}

/// Parse a Brave web-search response.
///
/// Public because it is the engine's contract with its upstream: given this
/// payload, these hits.
pub fn parse(body: &str) -> Result<Vec<Hit>, EngineError> {
    #[derive(Deserialize)]
    struct Response {
        #[serde(default)]
        web: Option<Web>,
    }
    #[derive(Deserialize)]
    struct Web {
        #[serde(default)]
        results: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        #[serde(default)]
        title: String,
        #[serde(default)]
        url: String,
        #[serde(default)]
        description: String,
    }

    let response: Response =
        serde_json::from_str(body).map_err(|error| EngineError::Protocol(error.to_string()))?;
    let Some(web) = response.web else {
        return Ok(Vec::new());
    };

    Ok(web
        .results
        .into_iter()
        .map(|item| {
            Hit::new(
                plain_text(&item.title),
                item.url,
                plain_text(&item.description),
            )
        })
        .filter(Hit::is_complete)
        .collect())
}

/// Brave returns snippets with HTML (`<strong>` around matched terms). Keep
/// the words, drop the markup.
fn plain_text(markup: &str) -> String {
    let fragment = Html::parse_fragment(markup);
    fragment
        .root_element()
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
