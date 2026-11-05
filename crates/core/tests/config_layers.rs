//! Config precedence and validation.

use dobukasu_core::Config;

fn overrides<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |key| {
        pairs
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| (*value).to_owned())
    }
}

#[test]
fn built_in_defaults_are_used_without_a_file() {
    let config = Config::default();

    assert_eq!(config.server.bind, "127.0.0.1:8080");
    assert_eq!(config.search.limit, 10);
    assert_eq!(config.search.max_limit, 50);
    assert!(config.search.dedup);
    assert_eq!(config.search.engines, None);
}

#[test]
fn a_toml_file_sets_server_search_and_engine_options() {
    let config = Config::from_toml(
        r#"
        [server]
        bind = "0.0.0.0:9000"

        [search]
        limit = 5
        max_limit = 20
        dedup = false
        engines = ["bing"]

        [engines.bing]
        enabled = true

        [engines.google]
        enabled = false
        mode = "cse"
        api_key = "key"
        cx = "cx"
        "#,
    )
    .expect("valid config");

    assert_eq!(config.server.bind, "0.0.0.0:9000");
    assert_eq!(config.search.limit, 5);
    assert_eq!(config.search.max_limit, 20);
    assert!(!config.search.dedup);
    assert_eq!(config.search.engines, Some(vec!["bing".to_owned()]));
    assert_eq!(config.engines["google"].enabled, Some(false));
    assert_eq!(config.engine_options("google").string("mode"), Some("cse"));
    assert_eq!(
        config.engine_options("google").string("api_key"),
        Some("key")
    );
    assert!(config.engine_options("bing").string("mode").is_none());
}

#[test]
fn unknown_top_level_keys_are_rejected() {
    let error = Config::from_toml("[server]\nbind = \"127.0.0.1:1\"\n[typo]\nx = 1\n")
        .expect_err("must reject a typo");

    assert!(error.to_string().contains("typo"), "{error}");
}

#[test]
fn environment_overrides_win_over_the_file() {
    let mut config = Config::from_toml(
        r#"
        [server]
        bind = "127.0.0.1:9000"
        [search]
        limit = 5
        dedup = false
        "#,
    )
    .expect("valid config");

    config
        .apply_overrides(overrides(&[
            ("DOBUKASU_BIND", "127.0.0.1:9100"),
            ("DOBUKASU_LIMIT", "7"),
            ("DOBUKASU_DEDUP", "true"),
            ("DOBUKASU_ENGINES", "bing, duckduckgo ,"),
        ]))
        .expect("valid overrides");

    assert_eq!(config.server.bind, "127.0.0.1:9100");
    assert_eq!(config.search.limit, 7);
    assert!(config.search.dedup);
    assert_eq!(
        config.search.engines,
        Some(vec!["bing".to_owned(), "duckduckgo".to_owned()])
    );
}

#[test]
fn an_unparseable_override_is_a_configuration_error() {
    let mut config = Config::default();

    let error = config
        .apply_overrides(overrides(&[("DOBUKASU_LIMIT", "many")]))
        .expect_err("must reject");

    assert!(error.to_string().contains("DOBUKASU_LIMIT"), "{error}");
}

#[test]
fn a_limit_above_the_cap_or_zero_is_rejected() {
    let mut config = Config::default();
    config.search.limit = 10;
    config.search.max_limit = 5;
    assert!(config.validate().is_err());

    let mut config = Config::default();
    config.search.limit = 0;
    assert!(config.validate().is_err());

    let mut config = Config::default();
    config.search.timeout_ms = 0;
    assert!(config.validate().is_err());
}

#[test]
fn an_explicit_path_is_read_and_validated() {
    let directory = std::env::temp_dir().join(format!("dobukasu-config-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("temp dir");
    let path = directory.join("dobukasu.toml");
    std::fs::write(
        &path,
        "[server]\nbind = \"127.0.0.1:8555\"\n[search]\nlimit = 3\n",
    )
    .expect("write config");

    let config = Config::load(Some(&path)).expect("loads");

    assert_eq!(config.server.bind, "127.0.0.1:8555");
    assert_eq!(config.search.limit, 3);

    std::fs::remove_file(&path).expect("cleanup");
    std::fs::remove_dir(&directory).expect("cleanup");
}

#[test]
fn a_missing_path_is_a_configuration_error() {
    let path = std::env::temp_dir().join("dobukasu-does-not-exist.toml");

    let error = Config::load(Some(&path)).expect_err("must not load");

    assert!(error.to_string().contains("cannot read"), "{error}");
}

#[test]
fn a_cap_equal_to_the_limit_is_accepted() {
    let mut config = Config::default();
    config.search.limit = 5;
    config.search.max_limit = 5;

    assert!(config.validate().is_ok());
}

#[test]
fn engine_options_expose_configured_and_default_values() {
    let config = Config::from_toml("[engines.google]\nmode = \"cse\"\napi_key = \"k\"\n")
        .expect("valid config");
    let options = config.engine_options("google");

    assert_eq!(options.string_or("mode", "auto"), "cse");
    assert_eq!(options.string_or("cx", "none"), "none");
    assert_eq!(options.string("api_key"), Some("k"));
}
