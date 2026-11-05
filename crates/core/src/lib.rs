//! The parts that never change when an engine does: what a result is, how
//! rankings are merged, and how an engine is registered.

mod engine;
mod error;
mod merge;
mod model;
mod registry;
mod search;

pub use engine::{Engine, EngineContext, Plugin, plugins};
pub use error::{DobukasuError, EngineError};
pub use merge::{canonical_url, fuse};
pub use model::{
    EngineHits, EngineReport, EngineStatus, Hit, SearchOutcome, SearchQuery, SearchResult,
    TimeRange,
};
pub use registry::Registry;
pub use search::search;
