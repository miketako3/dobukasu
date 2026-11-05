//! dobukasu — a minimal, plug-in based meta-search API for AI agents.

pub mod api;
pub mod config;
pub mod engine;
pub mod error;
pub mod merge;
pub mod model;
pub mod registry;

pub use config::Config;
pub use engine::{Engine, EngineContext, Plugin, PluginOptions};
pub use error::{DobukasuError, EngineError};
pub use merge::{canonical_url, fuse};
pub use model::{
    EngineHits, EngineReport, EngineStatus, Hit, SearchQuery, SearchResponse, SearchResult,
    TimeRange,
};
pub use registry::Registry;
