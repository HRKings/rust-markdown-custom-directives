//! Runtime abstraction for directive handlers.
//!
//! The engine calls `DirectiveRuntime::execute` during the resolution pass.
//! Implementations (e.g. `mdx_lua::LuaRuntime`) may be script-based or native
//! Rust. The trait is `Send` but NOT `Sync` — `mlua::Lua` is single-threaded,
//! so the engine serializes calls behind `&mut` for loading and holds `&`
//! for execution (implementations use interior mutability as needed).

pub mod cache;
pub mod resolver;

pub use cache::{CacheKey, DirectiveCache};
pub use resolver::{ContentResolver, ResolveError, ResolvedLink};

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ast::{AttributeMap, DirectiveBody, DirectiveKind, Node};
use crate::span::Span;

/// Where to load a script from.
#[derive(Debug, Clone)]
pub enum ScriptSource {
    File(PathBuf),
    Text(String),
    NamedText { name: String, content: String },
}

/// Opaque handle to a loaded script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScriptId(pub u64);

/// Metadata about a registered handler.
#[derive(Debug, Clone)]
pub struct HandlerDescriptor {
    pub name: String,
    pub script: ScriptId,
    /// Optional restriction to one directive kind, or `None` for either.
    pub kind: Option<DirectiveKind>,
}

/// Input passed to a directive handler.
#[derive(Debug, Clone)]
pub struct DirectiveInvocation {
    pub name: String,
    pub kind: DirectiveKind,
    pub attributes: AttributeMap,
    pub body: DirectiveBody,
    /// A simplified textual summary of children, for handlers that only need a preview.
    pub children_text: String,
    pub span: Span,
}

/// Structured output returned by a directive handler.
#[derive(Debug, Clone)]
pub enum DirectiveOutput {
    /// Plain text, escaped on output.
    Text(String),
    /// Trusted raw HTML, emitted verbatim.
    Html(String),
    /// A markdown string to be reparsed by the engine and spliced back in.
    /// Subject to `ReparseLimits::max_reparse_depth` and the document-wide
    /// directive count cap.
    Markdown(String),
    /// Pre-built AST nodes inserted in place of the directive.
    Nodes(Vec<Node>),
    /// An opaque component reference, rendered as a deterministic placeholder.
    Component { name: String, props: AttributeMap },
    /// Arbitrary structured data, not directly rendered. Emits an `Info` diagnostic.
    Data(serde_json::Value),
    /// A non-panicking handler failure, honoured per `ResolutionMode`.
    Error { message: String },
}

/// Read-only host context exposed to directive handlers.
#[derive(Debug, Default, Clone)]
pub struct RuntimeContext {
    /// Document-level frontmatter value, if any.
    pub document_metadata: Option<serde_json::Value>,
    /// Free-form variables supplied by the host at resolution time.
    pub variables: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("script load failed: {0}")]
    Load(String),
    #[error("unknown handler: {0}")]
    UnknownHandler(String),
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("invalid return value: {0}")]
    InvalidReturn(String),
    #[error("{0}")]
    Other(String),
}

/// The runtime trait. Implementations must be `Send` (but not necessarily `Sync`).
pub trait DirectiveRuntime: Send {
    fn load_script(&mut self, source: ScriptSource) -> Result<ScriptId, RuntimeError>;
    fn unload_script(&mut self, id: ScriptId) -> Result<(), RuntimeError>;
    fn list_handlers(&self) -> Vec<HandlerDescriptor>;
    fn execute(
        &self,
        handler: &str,
        invocation: DirectiveInvocation,
        ctx: &RuntimeContext,
    ) -> Result<DirectiveOutput, RuntimeError>;
    /// Generation counter — increments on every successful load/unload so that
    /// `DirectiveCache` implementations can include it in `CacheKey` for invalidation.
    fn generation(&self) -> u64;
}

/// A no-op runtime used when the host wants to parse without executing anything.
/// Every `execute` call returns `UnknownHandler`; `list_handlers` is empty.
pub struct NullRuntime;

impl DirectiveRuntime for NullRuntime {
    fn load_script(&mut self, _source: ScriptSource) -> Result<ScriptId, RuntimeError> {
        Err(RuntimeError::Load("NullRuntime cannot load scripts".into()))
    }
    fn unload_script(&mut self, _id: ScriptId) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn list_handlers(&self) -> Vec<HandlerDescriptor> {
        Vec::new()
    }
    fn execute(
        &self,
        handler: &str,
        _invocation: DirectiveInvocation,
        _ctx: &RuntimeContext,
    ) -> Result<DirectiveOutput, RuntimeError> {
        Err(RuntimeError::UnknownHandler(handler.to_string()))
    }
    fn generation(&self) -> u64 {
        0
    }
}
