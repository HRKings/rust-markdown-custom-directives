//! Attribute maps for directives.
//!
//! Backed by a `BTreeMap<String, serde_json::Value>` for deterministic iteration
//! order — matters for snapshot tests and cache key stability.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Ordered map of attribute name to JSON-typed value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttributeMap(pub BTreeMap<String, serde_json::Value>);

impl AttributeMap {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn insert(&mut self, k: impl Into<String>, v: impl Into<serde_json::Value>) {
        self.0.insert(k.into(), v.into());
    }

    pub fn get(&self, k: &str) -> Option<&serde_json::Value> {
        self.0.get(k)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, String, serde_json::Value> {
        self.0.iter()
    }
}

impl<K: Into<String>, V: Into<serde_json::Value>> FromIterator<(K, V)> for AttributeMap {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        AttributeMap(iter.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
    }
}
