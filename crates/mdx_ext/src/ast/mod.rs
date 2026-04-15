//! Abstract syntax tree for parsed markdown documents.

mod attr;
mod directive;
mod link;

pub use attr::AttributeMap;
pub use directive::{DirectiveBody, DirectiveKind, DirectiveNode, InlineDirectiveNode};
pub use link::{LinkKind, LinkNode};

use serde::{Deserialize, Serialize};

use crate::diagnostics::Diagnostic;
use crate::span::Span;

/// A fully-parsed markdown document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Original source text, retained for span-based slicing and `Passthrough` rendering.
    pub source: String,
    pub frontmatter: Option<Frontmatter>,
    pub children: Vec<Node>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Document {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            frontmatter: None,
            children: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

/// YAML frontmatter block. Value is retained as a `serde_json::Value` for
/// downstream extensibility without binding callers to a YAML crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frontmatter {
    pub value: serde_json::Value,
    pub span: Span,
}

/// The primary AST node variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Node {
    Paragraph {
        children: Vec<Node>,
        span: Span,
    },
    Heading {
        level: u8,
        children: Vec<Node>,
        span: Span,
    },
    Text {
        value: String,
        span: Span,
    },
    Emphasis {
        children: Vec<Node>,
        span: Span,
    },
    Strong {
        children: Vec<Node>,
        span: Span,
    },
    /// Inline code span.
    Code {
        value: String,
        span: Span,
    },
    CodeBlock {
        language: Option<String>,
        value: String,
        span: Span,
    },
    List {
        ordered: bool,
        start: Option<u64>,
        items: Vec<Node>,
        span: Span,
    },
    ListItem {
        children: Vec<Node>,
        span: Span,
    },
    BlockQuote {
        children: Vec<Node>,
        span: Span,
    },
    Link(LinkNode),
    Image {
        url: String,
        alt: String,
        title: Option<String>,
        span: Span,
    },
    Directive(DirectiveNode),
    InlineDirective(InlineDirectiveNode),
    /// Raw HTML block or inline fragment.
    Html {
        value: String,
        span: Span,
    },
    ThematicBreak {
        span: Span,
    },
    /// A resolved component placeholder produced by directive resolution.
    Component {
        name: String,
        props: AttributeMap,
        children: Vec<Node>,
        span: Span,
    },
    /// A soft line break inside a paragraph.
    SoftBreak {
        span: Span,
    },
    /// A hard line break inside a paragraph.
    HardBreak {
        span: Span,
    },
}

impl Node {
    pub fn span(&self) -> Span {
        match self {
            Node::Paragraph { span, .. }
            | Node::Heading { span, .. }
            | Node::Text { span, .. }
            | Node::Emphasis { span, .. }
            | Node::Strong { span, .. }
            | Node::Code { span, .. }
            | Node::CodeBlock { span, .. }
            | Node::List { span, .. }
            | Node::ListItem { span, .. }
            | Node::BlockQuote { span, .. }
            | Node::Image { span, .. }
            | Node::Html { span, .. }
            | Node::ThematicBreak { span }
            | Node::Component { span, .. }
            | Node::SoftBreak { span }
            | Node::HardBreak { span } => *span,
            Node::Link(l) => l.span,
            Node::Directive(d) => d.span,
            Node::InlineDirective(d) => d.span,
        }
    }
}
