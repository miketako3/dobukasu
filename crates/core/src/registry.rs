//! The set of engines a search can reach.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::engine::{Engine, Plugin, plugins};
use crate::error::{DobukasuError, EngineError};

#[derive(Clone, Default)]
pub struct Registry {
    engines: BTreeMap<&'static str, Arc<dyn Engine>>,
    order: Vec<&'static str>,
    defaults: Vec<String>,
    /// Plug-ins that are linked in but could not be built, with the reason.
    unavailable: BTreeMap<&'static str, String>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Registry")
            .field("engines", &self.order)
            .field("defaults", &self.defaults)
            .field("unavailable", &self.unavailable)
            .finish()
    }
}

/// A failure message without the `engine is misconfigured` wrapper: the
/// registry adds its own context.
fn reason(error: &EngineError) -> String {
    match error {
        EngineError::Config(message) => message.clone(),
        other => other.to_string(),
    }
}

impl Registry {
    /// Build a registry from engines supplied directly. Every engine becomes a
    /// default, in argument order. Used by embedders and tests.
    #[must_use]
    pub fn new(engines: Vec<Arc<dyn Engine>>) -> Self {
        let mut order = Vec::with_capacity(engines.len());
        let mut map = BTreeMap::new();
        for engine in engines {
            let id = engine.id();
            order.push(id);
            map.insert(id, engine);
        }
        let defaults = order.iter().map(|id| (*id).to_owned()).collect();
        Self {
            engines: map,
            order,
            defaults,
            unavailable: BTreeMap::new(),
        }
    }

    /// Instantiate every plug-in linked into this binary, honouring each
    /// plug-in's own default.
    pub fn from_plugins() -> Result<Self, DobukasuError> {
        Self::from_plugins_of(plugins())
    }

    /// The same, over an explicit plug-in list.
    ///
    /// A plug-in that cannot build — an engine whose API key is missing — is
    /// left out with its reason rather than failing the whole program: search
    /// should keep working with what is configured.
    pub fn from_plugins_of<'a>(
        plugins: impl IntoIterator<Item = &'a Plugin>,
    ) -> Result<Self, DobukasuError> {
        let mut engines = BTreeMap::new();
        let mut order = Vec::new();
        let mut unavailable = BTreeMap::new();

        for plugin in plugins {
            if !plugin.enabled_by_default {
                continue;
            }
            match (plugin.build)() {
                Ok(engine) => {
                    order.push(plugin.id);
                    engines.insert(plugin.id, engine);
                }
                Err(error) => {
                    unavailable.insert(plugin.id, reason(&error));
                }
            }
        }

        if order.is_empty() {
            return Err(DobukasuError::NoEngineEnabled);
        }

        let defaults = order.iter().map(|id| (*id).to_owned()).collect();
        Ok(Self {
            engines,
            order,
            defaults,
            unavailable,
        })
    }

    /// Plug-ins that are linked in but not usable, and why.
    #[must_use]
    pub fn unavailable(&self) -> &BTreeMap<&'static str, String> {
        &self.unavailable
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<Arc<dyn Engine>> {
        self.engines.get(id).cloned()
    }

    #[must_use]
    pub fn ids(&self) -> &[&'static str] {
        &self.order
    }

    #[must_use]
    pub fn default_ids(&self) -> &[String] {
        &self.defaults
    }

    #[must_use]
    pub fn is_default(&self, id: &str) -> bool {
        self.defaults.iter().any(|default| default == id)
    }

    /// Turn a requested id list (or `None` for the defaults) into engines.
    pub fn resolve(
        &self,
        requested: Option<&[String]>,
    ) -> Result<Vec<Arc<dyn Engine>>, DobukasuError> {
        let ids: &[String] = match requested {
            None => &self.defaults,
            Some(ids) => {
                if ids.is_empty() {
                    return Err(DobukasuError::InvalidRequest(
                        "the --engines list is empty".to_owned(),
                    ));
                }
                ids
            }
        };

        if ids.is_empty() {
            return Err(DobukasuError::NoEngineEnabled);
        }

        let mut engines = Vec::with_capacity(ids.len());
        for id in ids {
            match self.get(id) {
                Some(engine) => engines.push(engine),
                None => {
                    return Err(match self.unavailable.get(id.as_str()) {
                        Some(reason) => DobukasuError::InvalidRequest(format!(
                            "engine {id} is unavailable: {reason}"
                        )),
                        None => DobukasuError::UnknownEngine(id.clone()),
                    });
                }
            }
        }
        Ok(engines)
    }
}
