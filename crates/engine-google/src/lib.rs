//! Google engine plug-in.
//!
//! Two backends share one id: the keyless HTML endpoint, and the official
//! Custom Search JSON API when `api_key`/`cx` are configured. Google serves a
//! JavaScript interstitial to many clients, so the HTML backend reports
//! [`EngineError::Blocked`] instead of pretending it found nothing.

use std::sync::Arc;

use async_trait::async_trait;
use dobukasu_core::{Engine, EngineContext, EngineError, Hit, Plugin, PluginOptions, SearchQuery};
use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;
use url::Url;

pub const ID: &str = "google";

inventory::submit! {
    Plugin {
        id: ID,
        label: "Google",
        description: "HTML endpoint by default; Custom Search JSON API when api_key and cx are set.",
        enabled_by_default: true,
        build,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Auto,
    Html,
    Cse,
}

pub struct Google {
    mode: Mode,
    api_key: Option<String>,
    cx: Option<String>,
}

fn build(options: &PluginOptions) -> Result<Arc<dyn Engine>, EngineError> {
    let mode = match options
        .string_or("mode", "auto")
        .to_ascii_lowercase()
        .as_str()
    {
        "auto" => Mode::Auto,
        "html" => Mode::Html,
        "cse" => Mode::Cse,
        other => {
            return Err(EngineError::Config(format!(
                "unknown mode {other}; expected auto, html or cse"
            )));
        }
    };
    let api_key = value(options.string("api_key"), "GOOGLE_API_KEY");
    let cx = value(options.string("cx"), "GOOGLE_CX");

    if mode == Mode::Cse && (api_key.is_none() || cx.is_none()) {
        return Err(EngineError::Config(
            "mode = cse needs api_key and cx (or GOOGLE_API_KEY and GOOGLE_CX)".to_owned(),
        ));
    }

    Ok(Arc::new(Google { mode, api_key, cx }))
}

fn value(configured: Option<&str>, env: &str) -> Option<String> {
    configured
        .map(ToOwned::to_owned)
        .or_else(|| std::env::var(env).ok())
        .filter(|value| !value.trim().is_empty())
}

impl Google {
    fn uses_cse(&self) -> bool {
        match self.mode {
            Mode::Cse => true,
            Mode::Html => false,
            Mode::Auto => self.api_key.is_some() && self.cx.is_some(),
        }
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
            self.search_html(query, context).await
        }
    }
}

impl Google {
    async fn search_html(
        &self,
        query: &SearchQuery,
        context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError> {
        let mut url = Url::parse("https://www.google.com/search")
            .map_err(|error| EngineError::Config(error.to_string()))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", &query.text);
            pairs.append_pair("num", &query.limit.to_string());
            pairs.append_pair("gbv", "1");
            if let Some(language) = query.lang.as_deref() {
                pairs.append_pair("hl", language);
            }
            if let Some(region) = query.region.as_deref() {
                pairs.append_pair("gl", region);
            }
            pairs.append_pair("safe", if query.safe_search { "active" } else { "off" });
        }

        let html = context.fetch_text(url).await?;
        if is_blocked_page(&html) {
            return Err(EngineError::Blocked(
                "Google served the JavaScript interstitial; set api_key and cx for the CSE backend"
                    .to_owned(),
            ));
        }
        Ok(parse(&html))
    }

    async fn search_cse(
        &self,
        query: &SearchQuery,
        context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError> {
        let api_key = self
            .api_key
            .as_deref()
            .ok_or_else(|| EngineError::Config("missing api_key".to_owned()))?;
        let cx = self
            .cx
            .as_deref()
            .ok_or_else(|| EngineError::Config("missing cx".to_owned()))?;

        let mut url = Url::parse("https://www.googleapis.com/customsearch/v1")
            .map_err(|error| EngineError::Config(error.to_string()))?;
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

/// Does this page say "run JavaScript or prove you are human"?
#[must_use]
pub fn is_blocked_page(html: &str) -> bool {
    html.contains("enablejs")
        || html.contains("unusual traffic")
        || html.contains("/httpservice/retry/")
}

/// Parse a Google results page. Public because it is the engine's contract
/// with its upstream markup.
#[must_use]
pub fn parse(html: &str) -> Vec<Hit> {
    let document = Html::parse_document(html);
    let Ok(container) = Selector::parse("div.g, div.MjjYud") else {
        return Vec::new();
    };
    let Ok(link) = Selector::parse("div.yuRUbf a, a[href]") else {
        return Vec::new();
    };
    let Ok(heading) = Selector::parse("h3") else {
        return Vec::new();
    };
    let Ok(snippet) = Selector::parse(".VwiC3b, .IsZvec, span.aCOpRe") else {
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

fn resolve(href: &str) -> Option<String> {
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_owned());
    }
    if !href.starts_with('/') {
        return None;
    }
    let absolute = Url::parse(&format!("https://www.google.com{href}")).ok()?;
    if absolute.path() != "/url" {
        return None;
    }
    let target = absolute
        .query_pairs()
        .find(|(key, _)| key == "q")
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
