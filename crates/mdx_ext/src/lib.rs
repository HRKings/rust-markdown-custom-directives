//! `mdx_ext` — a comrak-based markdown engine extension.
//!
//! Adds first-class block directives (`:::name ... :::`), inline directives
//! (`{{name k="v"}}`), and wiki/namespaced links (`[[Page|Label]]`,
//! `[[ns:target]]`) on top of standard CommonMark/GFM handled by `comrak`.
//! Directive handlers are supplied by a runtime-loaded script runtime (see
//! `mdx_lua`) or any implementation of `runtime::DirectiveRuntime`.
//!
//! Entry point: [`MarkdownEngine`].
#![forbid(unsafe_code)]

pub mod ast;
pub mod config;
pub mod diagnostics;
pub mod engine;
pub mod error;
pub mod extension;
pub mod parser;
pub mod render;
pub mod runtime;
pub mod span;
pub mod syntax;
pub mod transform;

pub use ast::{
    AttributeMap, DirectiveBody, DirectiveNode, Document, Frontmatter, InlineDirectiveNode,
    LinkKind, LinkNode, Node,
};
pub use config::{EngineConfig, ReparseLimits, ResolutionMode};
pub use diagnostics::{Diagnostic, Severity};
pub use engine::{MarkdownEngine, MarkdownEngineBuilder};
pub use error::Error;
pub use runtime::{
    ContentResolver, DirectiveCache, DirectiveInvocation, DirectiveOutput, DirectiveRuntime,
    NullRuntime, RuntimeContext, RuntimeError, ScriptId, ScriptSource,
};
pub use span::Span;
