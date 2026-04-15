//! Directive AST node types.

use serde::{Deserialize, Serialize};

use super::attr::AttributeMap;
use super::Node;
use crate::span::Span;

/// Whether a directive is block- or inline-level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DirectiveKind {
    Block,
    Inline,
}

/// The body of a block directive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DirectiveBody {
    /// No body (opened and closed with nothing in between).
    None,
    /// Raw text — either parse failed or the body is not a mapping.
    Raw(String),
    /// Body successfully parsed as a YAML mapping into attributes.
    Attributes(AttributeMap),
}

impl DirectiveBody {
    pub fn is_none(&self) -> bool {
        matches!(self, DirectiveBody::None)
    }
}

/// Block directive (`:::name key: value\nbody\n:::`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectiveNode {
    pub name: String,
    pub attributes: AttributeMap,
    pub body: DirectiveBody,
    pub children: Vec<Node>,
    pub span: Span,
}

/// Inline directive (`{{name key="value" n=3}}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineDirectiveNode {
    pub name: String,
    pub attributes: AttributeMap,
    /// Verbatim source text of the directive, for debugging and `Passthrough` rendering.
    pub raw: String,
    pub span: Span,
}
