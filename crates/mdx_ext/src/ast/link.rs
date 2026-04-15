//! Link AST nodes.

use serde::{Deserialize, Serialize};

use super::Node;
use crate::span::Span;

/// Distinguishes normal markdown links from wiki/custom link forms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkKind {
    /// Standard markdown link with a URL (`[text](https://…)`).
    StandardUrl { url: String, title: Option<String> },
    /// `[[Page]]` or `[[Page|Label]]`.
    WikiLink { target: String },
    /// `[[namespace:target]]` — link with a resolver-specific namespace.
    NamespacedLink { namespace: String, target: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkNode {
    pub kind: LinkKind,
    /// Display children. For a bare wiki link `[[Page]]` this is a single text node "Page";
    /// for `[[Page|Label]]` it is "Label".
    pub children: Vec<Node>,
    pub span: Span,
}
