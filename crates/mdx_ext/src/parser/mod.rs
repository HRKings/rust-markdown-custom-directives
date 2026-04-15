//! Markdown parsing pipeline.
//!
//! 1. [`preprocess::rewrite`] — scan the source for custom syntax
//!    (`:::block:::`, `{{inline}}`, `[[wiki]]`) and replace each occurrence
//!    with a deterministic placeholder token, recording the original payload
//!    in a [`registry::PlaceholderRegistry`].
//! 2. [`comrak_adapter::parse_into`] — feed the rewritten source to `comrak`,
//!    walk the arena AST, translate into our owned [`Document`], and restore
//!    placeholders into first-class `Directive*` / `Link(Wiki/Namespaced)`
//!    nodes using the registry.
//!
//! `comrak` owns all of the standard markdown parsing. We only scan for the
//! custom syntax that `comrak` does not understand, plus YAML frontmatter
//! (delegated to `comrak`'s native front-matter extension but with our own
//! diagnostic layer on top).

pub mod comrak_adapter;
pub mod preprocess;
pub mod registry;

use crate::ast::Document;

/// Parse `source` into a `Document`. Never returns `Err` for user-authored
/// content; recoverable problems become diagnostics on the document.
pub fn parse(source: &str) -> Document {
    let mut doc = Document::new(source);
    let (rewritten, registry) = preprocess::rewrite(source, &mut doc);
    comrak_adapter::parse_into(&rewritten, &registry, &mut doc);
    doc
}
