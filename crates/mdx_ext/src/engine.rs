//! Public entry point: [`MarkdownEngine`] and its builder.
//!
//! Typical usage:
//!
//! ```no_run
//! use mdx_ext::{MarkdownEngine, ResolutionMode, RuntimeContext, NullRuntime};
//!
//! let mut engine = MarkdownEngine::builder()
//!     .with_runtime(Box::new(NullRuntime))
//!     .with_resolution_mode(ResolutionMode::Passthrough)
//!     .build()
//!     .unwrap();
//!
//! let doc = engine.parse("# Hello");
//! let resolved = engine.resolve(doc, &RuntimeContext::default()).unwrap();
//! let html = engine.render_html(&resolved);
//! assert!(html.contains("<h1>Hello</h1>"));
//! ```

use crate::ast::Document;
use crate::config::{EngineConfig, ReparseLimits, ResolutionMode};
use crate::error::{Error, Result};
use crate::extension::MarkdownExtension;
use crate::parser;
use crate::render;
use crate::runtime::{
    DirectiveRuntime, HandlerDescriptor, NullRuntime, RuntimeContext, RuntimeError, ScriptId,
    ScriptSource,
};
use crate::transform;

/// Builder for [`MarkdownEngine`].
#[derive(Default)]
pub struct MarkdownEngineBuilder {
    config: EngineConfig,
    runtime: Option<Box<dyn DirectiveRuntime>>,
    extensions: Vec<Box<dyn MarkdownExtension>>,
}

impl MarkdownEngineBuilder {
    pub fn with_runtime(mut self, runtime: Box<dyn DirectiveRuntime>) -> Self {
        self.runtime = Some(runtime);
        self
    }

    pub fn with_resolution_mode(mut self, mode: ResolutionMode) -> Self {
        self.config.resolution = mode;
        self
    }

    pub fn with_reparse_limits(mut self, limits: ReparseLimits) -> Self {
        self.config.limits = limits;
        self
    }

    pub fn with_extension(mut self, ext: Box<dyn MarkdownExtension>) -> Self {
        self.extensions.push(ext);
        self
    }

    pub fn build(self) -> Result<MarkdownEngine> {
        let runtime = self
            .runtime
            .unwrap_or_else(|| Box::new(NullRuntime) as Box<dyn DirectiveRuntime>);
        Ok(MarkdownEngine {
            config: self.config,
            runtime,
            extensions: self.extensions,
        })
    }
}

/// The primary entry point. Owns a [`DirectiveRuntime`] and a configuration.
pub struct MarkdownEngine {
    config: EngineConfig,
    runtime: Box<dyn DirectiveRuntime>,
    extensions: Vec<Box<dyn MarkdownExtension>>,
}

impl MarkdownEngine {
    pub fn builder() -> MarkdownEngineBuilder {
        MarkdownEngineBuilder::default()
    }

    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    pub fn runtime(&self) -> &dyn DirectiveRuntime {
        self.runtime.as_ref()
    }

    // ---- script management (delegated to the runtime) --------------------

    pub fn load_script(
        &mut self,
        source: ScriptSource,
    ) -> std::result::Result<ScriptId, RuntimeError> {
        self.runtime.load_script(source)
    }

    pub fn unload_script(&mut self, id: ScriptId) -> std::result::Result<(), RuntimeError> {
        self.runtime.unload_script(id)
    }

    pub fn reload_script(
        &mut self,
        id: ScriptId,
        source: ScriptSource,
    ) -> std::result::Result<ScriptId, RuntimeError> {
        self.runtime.unload_script(id)?;
        self.runtime.load_script(source)
    }

    pub fn list_handlers(&self) -> Vec<HandlerDescriptor> {
        self.runtime.list_handlers()
    }

    // ---- pipeline ---------------------------------------------------------

    /// Parse `source` into a `Document`. Runs host extensions' `preprocess`
    /// and `transform_ast` hooks but does not resolve directives.
    pub fn parse(&self, source: &str) -> Document {
        let mut src_owned: Option<String> = None;
        let mut parse_ctx = crate::extension::ParseContext;
        for ext in &self.extensions {
            let input = src_owned.as_deref().unwrap_or(source);
            if let Ok(Some(rewritten)) = ext.preprocess(input, &mut parse_ctx) {
                src_owned = Some(rewritten);
            }
        }
        let effective = src_owned.as_deref().unwrap_or(source);
        let mut doc = parser::parse(effective);
        transform::normalize(&mut doc);
        let mut tctx = crate::extension::TransformContext;
        for ext in &self.extensions {
            if let Err(e) = ext.transform_ast(&mut doc, &mut tctx) {
                doc.diagnostics
                    .push(crate::diagnostics::Diagnostic::warning(
                        crate::diagnostics::codes::EXTENSION_FAILURE,
                        format!("extension {} transform failed: {}", ext.name(), e),
                    ));
            }
        }
        doc
    }

    /// Run the resolution pass. Honors [`ResolutionMode`]. In `Strict` mode,
    /// returns `Error::StrictDiagnostics` if any error-severity diagnostics
    /// were emitted; the document is still returned via the error type's
    /// count, but the document is dropped — callers who want the document
    /// even on error should use `resolve_keep`.
    pub fn resolve(&self, doc: Document, ctx: &RuntimeContext) -> Result<Document> {
        let (doc, err_count) = self.resolve_keep(doc, ctx);
        if matches!(self.config.resolution, ResolutionMode::Strict) && err_count > 0 {
            return Err(Error::StrictDiagnostics { count: err_count });
        }
        Ok(doc)
    }

    /// Run the resolution pass and return the document along with the number
    /// of error-severity diagnostics emitted during resolution.
    pub fn resolve_keep(&self, mut doc: Document, ctx: &RuntimeContext) -> (Document, usize) {
        transform::resolve(
            &mut doc,
            self.runtime.as_ref(),
            self.config.resolution,
            self.config.limits,
            ctx,
        );
        let mut vctx = crate::extension::ValidationContext;
        for ext in &self.extensions {
            if let Err(e) = ext.validate(&doc, &mut vctx) {
                doc.diagnostics
                    .push(crate::diagnostics::Diagnostic::warning(
                        crate::diagnostics::codes::EXTENSION_FAILURE,
                        format!("extension {} validation failed: {}", ext.name(), e),
                    ));
            }
        }
        let errs = doc
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, crate::diagnostics::Severity::Error))
            .count();
        (doc, errs)
    }

    // ---- rendering --------------------------------------------------------

    pub fn render_html(&self, doc: &Document) -> String {
        render::html::render(doc)
    }

    pub fn render_text(&self, doc: &Document) -> String {
        render::text::render(doc)
    }

    pub fn render_debug(&self, doc: &Document) -> String {
        render::debug::render(doc)
    }
}
