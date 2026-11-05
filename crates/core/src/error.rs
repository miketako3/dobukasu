use thiserror::Error;

/// Errors raised while talking to an upstream search engine.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum EngineError {
    #[error("request failed: {0}")]
    Transport(String),
    #[error("upstream answered with status {0}")]
    Status(u16),
    #[error("upstream returned an unusable page: {0}")]
    Protocol(String),
    #[error("upstream blocked this client: {0}")]
    Blocked(String),
    #[error("timed out")]
    Timeout,
    #[error("engine is misconfigured: {0}")]
    Config(String),
}

impl From<url::ParseError> for EngineError {
    fn from(error: url::ParseError) -> Self {
        Self::Config(error.to_string())
    }
}

/// Errors raised while serving a request.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum DobukasuError {
    #[error("{0}")]
    InvalidRequest(String),
    #[error("unknown engine: {0}")]
    UnknownEngine(String),
    #[error("no engine is enabled")]
    NoEngineEnabled,
    #[error("every engine failed")]
    AllEnginesFailed,
    #[error("{0}")]
    Config(String),
}

impl DobukasuError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "invalid_request",
            Self::UnknownEngine(_) => "unknown_engine",
            Self::NoEngineEnabled => "no_engine_enabled",
            Self::AllEnginesFailed => "all_engines_failed",
            Self::Config(_) => "config",
        }
    }

    #[must_use]
    pub fn status(&self) -> u16 {
        match self {
            Self::InvalidRequest(_) | Self::UnknownEngine(_) => 400,
            Self::NoEngineEnabled | Self::Config(_) => 500,
            Self::AllEnginesFailed => 502,
        }
    }
}
