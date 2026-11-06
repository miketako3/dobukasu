//! Which plug-ins load, which join the default set, and what the rest report.

use std::sync::Arc;

use async_trait::async_trait;
use dobukasu_core::{
    DobukasuError, Engine, EngineContext, EngineError, Hit, Plugin, Registry, SearchQuery,
};

struct Named(&'static str);

#[async_trait]
impl Engine for Named {
    fn id(&self) -> &'static str {
        self.0
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
fn free_engine() -> Result<Arc<dyn Engine>, EngineError> {
    Ok(Arc::new(Named("free")))
}

#[allow(clippy::unnecessary_wraps)]
fn keyed_engine() -> Result<Arc<dyn Engine>, EngineError> {
    Ok(Arc::new(Named("keyed")))
}

fn needs_a_key() -> Result<Arc<dyn Engine>, EngineError> {
    Err(EngineError::Config("SEARCH_API_KEY is not set".to_owned()))
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
fn the_default_set_holds_only_the_engines_that_ask_to_be_in_it() {
    let registry = Registry::from_plugins_of([
        plugin("free", true, free_engine),
        plugin("keyed", false, keyed_engine),
    ])
    .expect("registry");

    assert_eq!(registry.ids(), &["free", "keyed"]);
    assert_eq!(registry.default_ids(), &["free".to_owned()]);
    assert!(registry.is_default("free"));
    assert!(!registry.is_default("keyed"));
}

#[test]
fn a_keyed_engine_is_still_reachable_when_named() {
    let registry = Registry::from_plugins_of([
        plugin("free", true, free_engine),
        plugin("keyed", false, keyed_engine),
    ])
    .expect("registry");

    let resolved = registry
        .resolve(Some(&["keyed".to_owned()]))
        .expect("keyed engine resolves");

    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].id(), "keyed");
}

#[test]
fn a_plugin_that_cannot_build_is_left_out_with_its_reason() {
    let registry = Registry::from_plugins_of([
        plugin("free", true, free_engine),
        plugin("keyed", false, needs_a_key),
    ])
    .expect("one usable engine is enough");

    assert_eq!(registry.ids(), &["free"]);
    assert_eq!(
        registry.unavailable().get("keyed").map(String::as_str),
        Some("SEARCH_API_KEY is not set")
    );
}

#[test]
fn asking_for_an_unavailable_engine_says_why() {
    let registry = Registry::from_plugins_of([
        plugin("free", true, free_engine),
        plugin("keyed", false, needs_a_key),
    ])
    .expect("registry");

    let Err(error) = registry.resolve(Some(&["keyed".to_owned()])) else {
        panic!("the engine cannot be built");
    };

    assert!(
        matches!(&error, DobukasuError::InvalidRequest(reason) if reason.contains("SEARCH_API_KEY")),
        "{error:?}"
    );
}

#[test]
fn an_engine_nobody_bundled_is_unknown_rather_than_unavailable() {
    let registry =
        Registry::from_plugins_of([plugin("free", true, free_engine)]).expect("registry");

    let Err(error) = registry.resolve(Some(&["nope".to_owned()])) else {
        panic!("nope is not an engine");
    };

    assert_eq!(error, DobukasuError::UnknownEngine("nope".to_owned()));
}

#[test]
fn a_default_search_needs_at_least_one_default_engine() {
    let registry =
        Registry::from_plugins_of([plugin("keyed", false, keyed_engine)]).expect("engine builds");

    let Err(error) = registry.resolve(None) else {
        panic!("nothing is in the default set");
    };
    assert_eq!(error, DobukasuError::NoDefaultEngine);
}

#[test]
fn a_binary_with_no_usable_engine_says_so() {
    let error = Registry::from_plugins_of([plugin("keyed", false, needs_a_key)])
        .expect_err("nothing builds");

    assert_eq!(error, DobukasuError::NoEngineEnabled);
}
