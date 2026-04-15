//! Engine configuration.

use serde::{Deserialize, Serialize};

/// How the resolution pass treats unknown or failing directives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ResolutionMode {
    /// Unknown handler or runtime failure → hard error diagnostic; engine refuses to render.
    Strict,
    /// Unknown/failed directives become fallback text, warning diagnostic emitted. Default.
    #[default]
    Lenient,
    /// Resolution pass is skipped entirely; directive nodes remain in the AST.
    /// Intended for tooling (linters, indexers) that parse without a runtime.
    Passthrough,
}

/// Guards against runaway markdown reparse and directive explosion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReparseLimits {
    /// Maximum nesting depth for `DirectiveOutput::Markdown` reparses.
    /// A value of `4` means a directive may return markdown containing directives
    /// that return markdown containing directives … up to four layers deep.
    pub max_reparse_depth: usize,
    /// Maximum number of directive nodes (of any kind) observed in a single
    /// document across the full resolution pass, including reparsed expansions.
    /// When exceeded, further directives are left unresolved with a diagnostic.
    pub max_directives_per_document: usize,
}

impl Default for ReparseLimits {
    fn default() -> Self {
        Self {
            max_reparse_depth: 4,
            max_directives_per_document: 1024,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct EngineConfig {
    pub resolution: ResolutionMode,
    pub limits: ReparseLimits,
}
