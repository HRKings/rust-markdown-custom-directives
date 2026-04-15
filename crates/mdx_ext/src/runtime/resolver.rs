//! Content resolver — optional host-supplied hook for link resolution and data lookup.

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ResolvedLink {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("unknown target: {0}")]
    Unknown(String),
    #[error("resolver error: {0}")]
    Other(String),
}

pub trait ContentResolver: Send + Sync {
    fn resolve_link(&self, target: &str) -> Option<ResolvedLink>;
    fn lookup_entity(&self, key: &str) -> Option<serde_json::Value>;
    fn query(&self, expr: &str) -> Result<Vec<serde_json::Value>, ResolveError>;
}
