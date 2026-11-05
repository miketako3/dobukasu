//! Bing engine plug-in (keyless HTML endpoint).

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as _;
use dobukasu_core::{Engine, EngineContext, EngineError, Hit, Plugin, PluginOptions, SearchQuery};
use percent_encoding::percent_decode_str;
use scraper::{ElementRef, Html, Selector};
use url::Url;

pub const ID: &str = "bing";

inventory::submit! {
    Plugin {
        id: ID,
        label: "Bing",
        description: "Keyless. Uses the bing.com/search HTML endpoint.",
        enabled_by_default: true,
        build,
    }
}

#[allow(clippy::unnecessary_wraps)]
fn build(_options: &PluginOptions) -> Result<Arc<dyn Engine>, EngineError> {
    Ok(Arc::new(Bing))
}

#[derive(Debug, Default)]
pub struct Bing;

#[async_trait]
impl Engine for Bing {
    fn id(&self) -> &'static str {
        ID
    }

    async fn search(
        &self,
        query: &SearchQuery,
        context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError> {
        let mut url = Url::parse("https://www.bing.com/search")
            .map_err(|error| EngineError::Config(error.to_string()))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", &query.text);
            pairs.append_pair("count", &query.limit.to_string());
            if let Some(language) = query.lang.as_deref() {
                pairs.append_pair("setlang", language);
            }
            if let Some(region) = query.region.as_deref() {
                pairs.append_pair("cc", region);
            }
            pairs.append_pair("adlt", if query.safe_search { "strict" } else { "off" });
            if let Some(range) = query.time_range {
                let filter = match range {
                    dobukasu_core::TimeRange::Day => "ex1:\"ez1\"",
                    dobukasu_core::TimeRange::Week => "ex1:\"ez2\"",
                    dobukasu_core::TimeRange::Month => "ex1:\"ez3\"",
                    dobukasu_core::TimeRange::Year => "ex1:\"ez5\"",
                };
                pairs.append_pair("filters", filter);
            }
        }

        let html = context.fetch_text(url).await?;
        Ok(parse(&html))
    }
}

/// Parse a Bing SERP. Public because it is the engine's contract with its
/// upstream HTML.
#[must_use]
pub fn parse(html: &str) -> Vec<Hit> {
    let document = Html::parse_document(html);
    let Ok(result) = Selector::parse("li.b_algo") else {
        return Vec::new();
    };
    let Ok(title_link) = Selector::parse("h2 a") else {
        return Vec::new();
    };
    let Ok(snippet) = Selector::parse(".b_caption p, .b_lineclamp2, .b_lineclamp3, .b_lineclamp4")
    else {
        return Vec::new();
    };

    document
        .select(&result)
        .filter(|node| !is_ad(node))
        .filter_map(|node| {
            let link = node.select(&title_link).next()?;
            let title = text_of(link);
            let href = link.value().attr("href")?;
            let url = resolve(href)?;
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
    node.value().classes().any(|class| class == "b_ad")
}

fn text_of(node: ElementRef<'_>) -> String {
    node.text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn resolve(href: &str) -> Option<String> {
    if !(href.starts_with("http://") || href.starts_with("https://")) {
        return None;
    }
    let url = Url::parse(href).ok()?;

    if url
        .host_str()
        .is_some_and(|host| host.ends_with("bing.com"))
        && url.path().starts_with("/ck/a")
    {
        if let Some(target) = url
            .query_pairs()
            .find(|(key, _)| key == "u")
            .and_then(|(_, value)| decode_redirect(&value))
        {
            return Some(target);
        }
    }
    Some(url.to_string())
}

fn decode_redirect(raw: &str) -> Option<String> {
    let value = percent_decode_str(raw).decode_utf8_lossy();
    let payload = value.strip_prefix("a1")?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let target = String::from_utf8(bytes).ok()?;
    let url = Url::parse(target.trim()).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| url.to_string())
}
