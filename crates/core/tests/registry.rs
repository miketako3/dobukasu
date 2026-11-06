//! Which plug-ins make it into a registry, and what happens to the rest.

use std::sync::Arc;

use async_trait::async_trait;
use dobukasu_core::{
    DobukasuError, Engine, EngineContext, EngineError, Hit, Plugin, Registry, SearchQuery,
};

struct Always;

#[async_trait]
impl Engine for Always {
    fn id(&self) -> &'static str {
        "always"
    }

    async fn search(
        &self,
        _query: &SearchQuery,
        _context: &EngineContext,
    ) -> Result<Vec<Hit>, EngineError> {
        Ok(Vec::new())
    }
}

#[allow(clippy::unnecessary_wraps)]
fn builds() -> Result<Arc<dyn Engine>, EngineError> {
    Ok(Arc::new(Always))
}

fn needs_a_key() -> Result<Arc<dyn Engine>, EngineError> {
    Err(EngineError::Config("BRAVE_API_KEY is not set".to_owned()))
}

fn plugin(
    id: &'static str,
    enabled_by_default: bool,
    build: fn() -> Result<Arc<dyn Engine>, EngineError>,
) -> &'static Plugin {
    Box::leak(Box::new(Plugin {
        id,
        label: id,
        description: "",
        enabled_by_default,
        build,
    }))
}

#[test]
fn a_plugin_that_cannot_build_is_left_out_with_its_reason() {
    let registry = Registry::from_plugins_of([
        plugin("keyless", true, builds),
        plugin("brave", true, needs_a_key),
    ])
    .expect("one usable engine is enough");

    assert_eq!(registry.ids(), &["keyless"]);
    assert_eq!(
        registry.unavailable().get("brave").map(String::as_str),
        Some("BRAVE_API_KEY is not set")
    );
}

#[test]
fn asking_for_an_unavailable_engine_says_why() {
    let registry = Registry::from_plugins_of([
        plugin("keyless", true, builds),
        plugin("brave", true, needs_a_key),
    ])
    .expect("registry");

    let Err(error) = registry.resolve(Some(&["brave".to_owned()])) else {
        panic!("brave cannot be built");
    };

    assert!(
        matches!(&error, DobukasuError::InvalidRequest(reason) if reason.contains("BRAVE_API_KEY")),
        "{error:?}"
    );
    assert!(
        matches!(&error, DobukasuError::InvalidRequest(reason) if reason.contains("unavailable")),
        "{error:?}"
    );
}

#[test]
fn a_plugin_that_is_off_by_default_is_not_even_built() {
    let registry = Registry::from_plugins_of([plugin("opt-in", false, needs_a_key)])
        .expect_err("nothing left");
    assert_eq!(registry, DobukasuError::NoEngineEnabled);

    let registry = Registry::from_plugins_of([
        plugin("keyless", true, builds),
        plugin("opt-in", false, needs_a_key),
    ])
    .expect("registry");

    assert_eq!(registry.ids(), &["keyless"]);
    assert!(registry.unavailable().is_empty());
}
