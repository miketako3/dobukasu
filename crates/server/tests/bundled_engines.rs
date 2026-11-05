//! The binary bundles every engine plug-in and lets configuration remove one.

// Link every bundled plug-in into this test binary, exactly like `main` does.
use dobukasu_engine_bing as _;
use dobukasu_engine_duckduckgo as _;
use dobukasu_engine_google as _;

use dobukasu_core::Config;
use dobukasu_core::config::EngineConfig;
use dobukasu_core::registry::Registry;

fn with_engine(mut config: Config, id: &str, settings: EngineConfig) -> Config {
    config.engines.insert(id.to_owned(), settings);
    config
}

#[test]
fn every_bundled_engine_is_registered() {
    let registry = Registry::from_plugins(&Config::default()).expect("defaults are valid");

    assert_eq!(registry.ids(), ["bing", "duckduckgo", "google"]);
    assert_eq!(
        registry.default_ids(),
        ["bing", "duckduckgo", "google"].map(str::to_owned)
    );
}

#[test]
fn google_can_be_turned_off_from_configuration() {
    let config = with_engine(
        Config::default(),
        "google",
        EngineConfig {
            enabled: Some(false),
            ..EngineConfig::default()
        },
    );

    let registry = Registry::from_plugins(&config).expect("valid");

    assert_eq!(registry.ids(), ["bing", "duckduckgo"]);
    assert!(registry.get("google").is_none());
}

#[test]
fn the_default_set_can_be_restricted_without_disabling_engines() {
    let mut config = Config::default();
    config.search.engines = Some(vec!["bing".to_owned()]);

    let registry = Registry::from_plugins(&config).expect("valid");

    assert_eq!(registry.default_ids(), ["bing".to_owned()]);
    assert_eq!(registry.ids().len(), 3, "the others stay callable");
}

#[test]
fn listing_an_engine_that_is_not_enabled_is_rejected() {
    let mut config = with_engine(
        Config::default(),
        "google",
        EngineConfig {
            enabled: Some(false),
            ..EngineConfig::default()
        },
    );
    config.search.engines = Some(vec!["google".to_owned()]);

    let error = Registry::from_plugins(&config).expect_err("must not start");

    assert!(error.to_string().contains("google"), "{error}");
}

#[test]
fn an_unknown_engine_in_configuration_is_rejected() {
    let config = with_engine(Config::default(), "altavista", EngineConfig::default());

    let error = Registry::from_plugins(&config).expect_err("must not start");

    assert!(error.to_string().contains("altavista"), "{error}");
}

#[test]
fn disabling_every_engine_is_rejected() {
    let mut config = Config::default();
    for id in ["bing", "duckduckgo", "google"] {
        config.engines.insert(
            id.to_owned(),
            EngineConfig {
                enabled: Some(false),
                ..EngineConfig::default()
            },
        );
    }

    let error = Registry::from_plugins(&config).expect_err("must not start");

    assert!(error.to_string().contains("no engine"), "{error}");
}
