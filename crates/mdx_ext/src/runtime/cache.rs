//! Optional cache trait for directive outputs.

use serde::{Deserialize, Serialize};

use super::DirectiveOutput;
use crate::ast::{AttributeMap, DirectiveBody};
use crate::config::ResolutionMode;

/// Key for a cacheable directive invocation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey {
    pub name: String,
    pub attributes_canonical: String,
    pub body_canonical: String,
    pub mode: ResolutionMode,
    pub generation: u64,
}

impl CacheKey {
    pub fn new(
        name: &str,
        attrs: &AttributeMap,
        body: &DirectiveBody,
        mode: ResolutionMode,
        generation: u64,
    ) -> Self {
        let attributes_canonical = serde_json::to_string(&attrs.0).unwrap_or_default();
        let body_canonical = match body {
            DirectiveBody::None => String::new(),
            DirectiveBody::Raw(s) => format!("raw:{s}"),
            DirectiveBody::Attributes(a) => {
                format!("attrs:{}", serde_json::to_string(&a.0).unwrap_or_default())
            }
        };
        CacheKey {
            name: name.to_string(),
            attributes_canonical,
            body_canonical,
            mode,
            generation,
        }
    }
}

pub trait DirectiveCache: Send + Sync {
    fn get(&self, key: &CacheKey) -> Option<DirectiveOutput>;
    fn set(&self, key: CacheKey, value: DirectiveOutput);
}
