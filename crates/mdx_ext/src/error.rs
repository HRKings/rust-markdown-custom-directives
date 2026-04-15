//! Error types for the engine boundary.
//!
//! User-authored markdown never produces `Error` values — those become
//! `Diagnostic`s attached to the `Document`. `Error` is reserved for
//! host-side failures (I/O, engine misuse, runtime load failures).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("engine not configured: {0}")]
    Config(String),

    #[error("runtime error: {0}")]
    Runtime(String),

    #[error("render error: {0}")]
    Render(String),

    #[error("strict mode: document has {count} error diagnostic(s)")]
    StrictDiagnostics { count: usize },

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
