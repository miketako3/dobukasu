//! The set of engines a search can reach.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::engine::{Engine, plugins};
use crate::error::DobukasuError;

#[derive(Clone, Default)]
pub struct Registry {
    engines: BTreeMap<&'static str, Arc<dyn Engine>>,
    order: Vec<&'static str>,
    defaults: Vec<String>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Registry")
            .field("engines", &self.order)
            .field("defaults", &self.defaults)
            .finish()
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
        }
    }

    /// Instantiate every plug-in linked into this binary, honouring each
    /// plug-in's own default.
    pub fn from_plugins() -> Result<Self, DobukasuError> {
        let mut engines = BTreeMap::new();
        let mut order = Vec::new();

        for plugin in plugins() {
            if !plugin.enabled_by_default {
                continue;
            }
            let engine = (plugin.build)().map_err(|error| {
                DobukasuError::InvalidRequest(format!("engine {}: {error}", plugin.id))
            })?;
            order.push(plugin.id);
            engines.insert(plugin.id, engine);
        }

        if order.is_empty() {
            return Err(DobukasuError::NoEngineEnabled);
        }

        let defaults = order.iter().map(|id| (*id).to_owned()).collect();
        Ok(Self {
            engines,
            order,
            defaults,
        })
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
                None => return Err(DobukasuError::UnknownEngine(id.clone())),
            }
        }
        Ok(engines)
    }
}
