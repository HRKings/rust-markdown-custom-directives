//! Structured diagnostics with severity, code, span, and optional cause chain.

use serde::{Deserialize, Serialize};

use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// Stylistic advisories (e.g. YAML body fallback).
    Lint,
    /// Informational messages that don't affect correctness.
    Info,
    /// Recoverable issues — parsing continued with fallback.
    Warning,
    /// Unrecoverable issues or strict-mode failures.
    Error,
}

/// Stable diagnostic codes. Keep in sync with the plan's `MDXnnn` table.
pub mod codes {
    pub const INVALID_FRONTMATTER: &str = "MDX001";
    pub const MALFORMED_BLOCK_DIRECTIVE: &str = "MDX101";
    pub const MALFORMED_INLINE_DIRECTIVE: &str = "MDX102";
    pub const MALFORMED_WIKI_LINK: &str = "MDX103";
    pub const UNKNOWN_HANDLER: &str = "MDX201";
    pub const RUNTIME_EXECUTION_FAILURE: &str = "MDX202";
    pub const INVALID_RUNTIME_RETURN: &str = "MDX203";
    pub const YAML_BODY_FALLBACK: &str = "MDX301";
    pub const REPARSE_DEPTH_EXCEEDED: &str = "MDX401";
    pub const DIRECTIVE_LIMIT_EXCEEDED: &str = "MDX402";
    pub const EXTENSION_FAILURE: &str = "MDX501";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: std::borrow::Cow<'static, str>,
    pub message: String,
    pub span: Option<Span>,
    /// Script or handler name, when relevant.
    pub source: Option<String>,
    pub cause: Option<Box<Diagnostic>>,
}

impl Diagnostic {
    pub fn new(severity: Severity, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity,
            code: std::borrow::Cow::Borrowed(code),
            message: message.into(),
            span: None,
            source: None,
            cause: None,
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_cause(mut self, cause: Diagnostic) -> Self {
        self.cause = Some(Box::new(cause));
        self
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, code, message)
    }

    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, code, message)
    }

    pub fn lint(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(Severity::Lint, code, message)
    }
}
