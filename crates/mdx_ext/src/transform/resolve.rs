//! Directive resolution pass.
//!
//! Walks the document AST and invokes `DirectiveRuntime::execute` for each
//! `Directive` / `InlineDirective` node. The handler's `DirectiveOutput`
//! is then spliced back into the tree per the configured `ResolutionMode`:
//!
//! * **Strict**: unknown handler or runtime failure is an error-severity
//!   diagnostic; `engine::resolve` will refuse to render.
//! * **Lenient** (default): unknown/failed directives emit warnings and the
//!   original directive is retained as fallback.
//! * **Passthrough**: the resolver is not called — directive nodes remain
//!   unchanged. (The engine short-circuits this mode before calling `resolve`,
//!   so this function assumes one of the other two modes.)
//!
//! Markdown-returning directives (`DirectiveOutput::Markdown(s)`) are reparsed
//! with the same engine configuration, with a bounded recursion depth and a
//! total directive budget enforced by `ReparseLimits`.

use crate::ast::{DirectiveKind, Document, Node};
use crate::config::{ReparseLimits, ResolutionMode};
use crate::diagnostics::{codes, Diagnostic, Severity};
use crate::runtime::{
    DirectiveInvocation, DirectiveOutput, DirectiveRuntime, RuntimeContext, RuntimeError,
};

/// Resolve all directives in `doc` using `runtime`.
pub fn resolve(
    doc: &mut Document,
    runtime: &dyn DirectiveRuntime,
    mode: ResolutionMode,
    limits: ReparseLimits,
    ctx: &RuntimeContext,
) {
    if matches!(mode, ResolutionMode::Passthrough) {
        return;
    }
    let mut state = State {
        mode,
        limits,
        total_directives: 0,
    };
    let mut children = std::mem::take(&mut doc.children);
    resolve_nodes(&mut children, runtime, ctx, &mut state, doc, 0);
    doc.children = children;
}

struct State {
    mode: ResolutionMode,
    limits: ReparseLimits,
    total_directives: usize,
}

fn resolve_nodes(
    nodes: &mut Vec<Node>,
    runtime: &dyn DirectiveRuntime,
    ctx: &RuntimeContext,
    state: &mut State,
    doc: &mut Document,
    depth: usize,
) {
    let mut i = 0usize;
    while i < nodes.len() {
        // First recurse into any children.
        recurse_into(&mut nodes[i], runtime, ctx, state, doc, depth);

        // Then, if this node is itself a directive, attempt resolution.
        let replacement = match &nodes[i] {
            Node::Directive(d) => Some(invoke(
                &d.name,
                DirectiveKind::Block,
                d.attributes.clone(),
                d.body.clone(),
                children_summary(&d.children),
                d.span,
                runtime,
                ctx,
                state,
                doc,
                depth,
            )),
            Node::InlineDirective(d) => Some(invoke(
                &d.name,
                DirectiveKind::Inline,
                d.attributes.clone(),
                crate::ast::DirectiveBody::None,
                d.raw.clone(),
                d.span,
                runtime,
                ctx,
                state,
                doc,
                depth,
            )),
            _ => None,
        };
        match replacement {
            Some(Some(new_nodes)) => {
                let len = new_nodes.len();
                nodes.splice(i..=i, new_nodes);
                i += len;
            }
            _ => {
                i += 1;
            }
        }
    }
}

fn recurse_into(
    node: &mut Node,
    runtime: &dyn DirectiveRuntime,
    ctx: &RuntimeContext,
    state: &mut State,
    doc: &mut Document,
    depth: usize,
) {
    match node {
        Node::Paragraph { children, .. }
        | Node::Heading { children, .. }
        | Node::Emphasis { children, .. }
        | Node::Strong { children, .. }
        | Node::BlockQuote { children, .. }
        | Node::ListItem { children, .. }
        | Node::Component { children, .. } => {
            resolve_nodes(children, runtime, ctx, state, doc, depth)
        }
        Node::List { items, .. } => resolve_nodes(items, runtime, ctx, state, doc, depth),
        Node::Link(link) => {
            let _ = &link.kind;
            resolve_nodes(&mut link.children, runtime, ctx, state, doc, depth);
        }
        Node::Directive(d) => resolve_nodes(&mut d.children, runtime, ctx, state, doc, depth),
        _ => {}
    }
}

fn children_summary(children: &[Node]) -> String {
    let mut out = String::new();
    for n in children {
        match n {
            Node::Text { value, .. } | Node::Code { value, .. } => out.push_str(value),
            Node::SoftBreak { .. } | Node::HardBreak { .. } => out.push('\n'),
            Node::Paragraph { children, .. }
            | Node::Heading { children, .. }
            | Node::Emphasis { children, .. }
            | Node::Strong { children, .. } => out.push_str(&children_summary(children)),
            _ => {}
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn invoke(
    name: &str,
    kind: DirectiveKind,
    attributes: crate::ast::AttributeMap,
    body: crate::ast::DirectiveBody,
    children_text: String,
    span: crate::span::Span,
    runtime: &dyn DirectiveRuntime,
    ctx: &RuntimeContext,
    state: &mut State,
    doc: &mut Document,
    depth: usize,
) -> Option<Vec<Node>> {
    // Document-wide directive budget guard.
    state.total_directives += 1;
    if state.total_directives > state.limits.max_directives_per_document {
        doc.diagnostics.push(
            Diagnostic::warning(
                codes::DIRECTIVE_LIMIT_EXCEEDED,
                format!(
                    "directive budget of {} exceeded — remaining directives left unresolved",
                    state.limits.max_directives_per_document
                ),
            )
            .with_span(span),
        );
        return None;
    }

    let invocation = DirectiveInvocation {
        name: name.to_string(),
        kind,
        attributes,
        body,
        children_text,
        span,
    };

    match runtime.execute(name, invocation, ctx) {
        Ok(output) => Some(map_output(output, span, state, runtime, ctx, doc, depth)),
        Err(e) => Some(handle_error(e, name, span, state.mode, doc)),
    }
}

fn map_output(
    output: DirectiveOutput,
    span: crate::span::Span,
    state: &mut State,
    runtime: &dyn DirectiveRuntime,
    ctx: &RuntimeContext,
    doc: &mut Document,
    depth: usize,
) -> Vec<Node> {
    match output {
        DirectiveOutput::Text(value) => vec![Node::Text { value, span }],
        DirectiveOutput::Html(value) => vec![Node::Html { value, span }],
        DirectiveOutput::Nodes(nodes) => nodes,
        DirectiveOutput::Component { name, props } => vec![Node::Component {
            name,
            props,
            children: Vec::new(),
            span,
        }],
        DirectiveOutput::Data(value) => {
            doc.diagnostics.push(
                Diagnostic::new(
                    Severity::Info,
                    codes::INVALID_RUNTIME_RETURN,
                    "directive returned Data variant; rendering as text",
                )
                .with_span(span),
            );
            vec![Node::Text {
                value: serde_json::to_string(&value).unwrap_or_default(),
                span,
            }]
        }
        DirectiveOutput::Error { message } => handle_error(
            RuntimeError::Execution(message.clone()),
            "<returned error>",
            span,
            state.mode,
            doc,
        ),
        DirectiveOutput::Markdown(source) => {
            if depth >= state.limits.max_reparse_depth {
                doc.diagnostics.push(
                    Diagnostic::warning(
                        codes::REPARSE_DEPTH_EXCEEDED,
                        format!(
                            "maximum reparse depth of {} reached — further markdown output left as plain text",
                            state.limits.max_reparse_depth
                        ),
                    )
                    .with_span(span),
                );
                return vec![Node::Text {
                    value: source,
                    span,
                }];
            }
            let mut sub_doc = crate::parser::parse(&source);
            // Carry diagnostics from the sub-parse up to the parent document.
            doc.diagnostics
                .extend(std::mem::take(&mut sub_doc.diagnostics));
            // Recursively resolve the sub-document at depth + 1.
            let mut sub_children = std::mem::take(&mut sub_doc.children);
            resolve_nodes(&mut sub_children, runtime, ctx, state, doc, depth + 1);
            sub_children
        }
    }
}

fn handle_error(
    err: RuntimeError,
    name: &str,
    span: crate::span::Span,
    mode: ResolutionMode,
    doc: &mut Document,
) -> Vec<Node> {
    let (code, msg) = match &err {
        RuntimeError::UnknownHandler(n) => (
            codes::UNKNOWN_HANDLER,
            format!("unknown directive handler: {n}"),
        ),
        RuntimeError::Execution(m) => (
            codes::RUNTIME_EXECUTION_FAILURE,
            format!("directive handler failed: {m}"),
        ),
        RuntimeError::InvalidReturn(m) => (
            codes::INVALID_RUNTIME_RETURN,
            format!("invalid return value: {m}"),
        ),
        RuntimeError::Load(m) => (
            codes::RUNTIME_EXECUTION_FAILURE,
            format!("script load failed: {m}"),
        ),
        RuntimeError::Other(m) => (codes::RUNTIME_EXECUTION_FAILURE, m.clone()),
    };
    let severity = match mode {
        ResolutionMode::Strict => Severity::Error,
        _ => Severity::Warning,
    };
    doc.diagnostics.push(
        Diagnostic::new(severity, code, msg.clone())
            .with_span(span)
            .with_source(name.to_string()),
    );
    match mode {
        ResolutionMode::Strict => Vec::new(),
        _ => {
            // Lenient: leave a visible fallback text so unresolved directives
            // aren't silently dropped.
            vec![Node::Text {
                value: format!("[{name}]"),
                span,
            }]
        }
    }
}
