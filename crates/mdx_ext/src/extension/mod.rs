//! Native Rust extension hooks.
//!
//! Extensions are a thin side channel for host-supplied preprocessing and
//! post-parse transformation. The primary extension mechanism is the runtime
//! (`DirectiveRuntime`) — extensions here are for rare cases where an entire
//! Rust crate wants to hook into the pipeline without going through scripts.

use crate::ast::Document;
use crate::error::Error;

/// Host-side context passed to extensions during each pipeline phase.
/// Kept intentionally empty for v1 — future versions may carry mutable
/// references to diagnostics or configuration.
#[derive(Debug, Default)]
pub struct ParseContext;

#[derive(Debug, Default)]
pub struct TransformContext;

#[derive(Debug, Default)]
pub struct ValidationContext;

pub trait MarkdownExtension: Send + Sync {
    fn name(&self) -> &str;

    /// Source-level preprocessing. Returning `Ok(Some(new))` replaces the
    /// source; `Ok(None)` leaves it unchanged.
    fn preprocess(&self, _input: &str, _ctx: &mut ParseContext) -> Result<Option<String>, Error> {
        Ok(None)
    }

    /// AST-level transform. Runs after normalization but before resolution.
    fn transform_ast(&self, _doc: &mut Document, _ctx: &mut TransformContext) -> Result<(), Error> {
        Ok(())
    }

    /// Validation pass. Runs after resolution.
    fn validate(&self, _doc: &Document, _ctx: &mut ValidationContext) -> Result<(), Error> {
        Ok(())
    }
}
