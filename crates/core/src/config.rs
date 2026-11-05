//! Layered configuration: built-in defaults, an optional TOML file, then
//! environment overrides.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::DobukasuError;

pub const DEFAULT_BIND: &str = "127.0.0.1:8080";
pub const DEFAULT_LIMIT: usize = 10;
pub const DEFAULT_MAX_LIMIT: usize = 50;
pub const DEFAULT_TIMEOUT_MS: u64 = 8_000;
pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub server: ServerConfig,
    pub search: SearchConfig,
    pub engines: BTreeMap<String, EngineConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    pub bind: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: DEFAULT_BIND.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SearchConfig {
    pub limit: usize,
    pub max_limit: usize,
    pub timeout_ms: u64,
    pub dedup: bool,
    pub user_agent: String,
    /// Restrict the default engine set. `None` means every enabled engine.
    pub engines: Option<Vec<String>>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            limit: DEFAULT_LIMIT,
            max_limit: DEFAULT_MAX_LIMIT,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            dedup: true,
            user_agent: DEFAULT_USER_AGENT.to_owned(),
            engines: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EngineConfig {
    pub enabled: Option<bool>,
    #[serde(flatten)]
    pub options: BTreeMap<String, toml::Value>,
}

impl Config {
    /// Read a configuration file, then apply environment overrides.
    ///
    /// `explicit` wins over `DOBUKASU_CONFIG`, which wins over `./dobukasu.toml`.
    pub fn load(explicit: Option<&Path>) -> Result<Self, DobukasuError> {
        let from_env = std::env::var("DOBUKASU_CONFIG").ok().map(PathBuf::from);
        let candidate = explicit.map(Path::to_path_buf).or(from_env);
        let path = candidate.or_else(|| {
            let default = PathBuf::from("dobukasu.toml");
            default.is_file().then_some(default)
        });

        let mut config = match path {
            Some(path) => Self::from_toml(&std::fs::read_to_string(&path).map_err(|error| {
                DobukasuError::Config(format!("cannot read {}: {error}", path.display()))
            })?)?,
            None => Self::default(),
        };
        config.apply_overrides(|key| std::env::var(key).ok())?;
        config.validate()?;
        Ok(config)
    }

    pub fn from_toml(text: &str) -> Result<Self, DobukasuError> {
        toml::from_str(text).map_err(|error| DobukasuError::Config(error.to_string()))
    }

    /// Apply `DOBUKASU_*` overrides through an injected lookup, so tests never
    /// touch process-wide environment state.
    pub fn apply_overrides(
        &mut self,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<(), DobukasuError> {
        if let Some(value) = lookup("DOBUKASU_BIND") {
            self.server.bind = value;
        }
        if let Some(value) = lookup("DOBUKASU_LIMIT") {
            self.search.limit = parse("DOBUKASU_LIMIT", &value)?;
        }
        if let Some(value) = lookup("DOBUKASU_MAX_LIMIT") {
            self.search.max_limit = parse("DOBUKASU_MAX_LIMIT", &value)?;
        }
        if let Some(value) = lookup("DOBUKASU_TIMEOUT_MS") {
            self.search.timeout_ms = parse("DOBUKASU_TIMEOUT_MS", &value)?;
        }
        if let Some(value) = lookup("DOBUKASU_USER_AGENT") {
            self.search.user_agent = value;
        }
        if let Some(value) = lookup("DOBUKASU_DEDUP") {
            self.search.dedup = parse("DOBUKASU_DEDUP", &value)?;
        }
        if let Some(value) = lookup("DOBUKASU_ENGINES") {
            self.search.engines = Some(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .map(ToOwned::to_owned)
                    .collect(),
            );
        }
        Ok(())
    }

    pub fn validate(&mut self) -> Result<(), DobukasuError> {
        if self.search.limit == 0 {
            return Err(DobukasuError::Config(
                "search.limit must be positive".to_owned(),
            ));
        }
        if self.search.max_limit < self.search.limit {
            return Err(DobukasuError::Config(
                "search.max_limit must be at least search.limit".to_owned(),
            ));
        }
        if self.search.timeout_ms == 0 {
            return Err(DobukasuError::Config(
                "search.timeout_ms must be positive".to_owned(),
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn engine_options(&self, id: &str) -> crate::engine::PluginOptions {
        let options = self
            .engines
            .get(id)
            .map(|engine| engine.options.clone())
            .unwrap_or_default();
        crate::engine::PluginOptions::new(options)
    }
}

fn parse<T>(key: &str, value: &str) -> Result<T, DobukasuError>
where
    T: std::str::FromStr,
{
    value
        .parse()
        .map_err(|_| DobukasuError::Config(format!("{key} is not a valid value: {value}")))
}
