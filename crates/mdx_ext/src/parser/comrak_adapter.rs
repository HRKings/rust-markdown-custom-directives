//! Walk a `comrak` arena AST and project it into our owned [`Document`]/[`Node`]
//! tree, restoring placeholder tokens into first-class directive and wiki-link
//! nodes using the [`PlaceholderRegistry`].
//!
//! The adapter does not modify `comrak` nodes in place. It reads the arena,
//! builds owned values, and drops the arena when the projection is complete.
//! This matches the plan's "projection" approach: public API never exposes
//! `comrak` types.

use std::cell::RefCell;

use comrak::nodes::{AstNode, ListType, NodeValue, Sourcepos};
use comrak::{parse_document, Arena, Options};

use crate::ast::{
    AttributeMap, DirectiveBody, DirectiveNode, Document, Frontmatter, InlineDirectiveNode,
    LinkKind, LinkNode, Node,
};
use crate::diagnostics::{codes, Diagnostic, Severity};
use crate::parser::registry::{peek_inline_placeholder, InlineKind, PlaceholderRegistry};
use crate::span::Span;

/// Parse `rewritten` with comrak and project the arena into `doc`.
pub fn parse_into(rewritten: &str, registry: &PlaceholderRegistry, doc: &mut Document) {
    let arena = Arena::new();
    let options = build_options();
    let root: &AstNode = parse_document(&arena, rewritten, &options);

    let line_starts = compute_line_starts(rewritten);
    let ctx = Ctx {
        line_starts: &line_starts,
        registry,
    };

    // Walk direct children of the root Document node.
    for child in root.children() {
        let value = &child.data.borrow().value;
        // Comrak emits FrontMatter as a direct child of the document root.
        if let NodeValue::FrontMatter(raw) = value {
            handle_frontmatter(raw, child, &ctx, doc);
            continue;
        }
        if let Some(n) = convert_block(child, &ctx, doc) {
            doc.children.push(n);
        }
    }
}

fn build_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.front_matter_delimiter = Some("---".to_string());
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    options.extension.footnotes = true;
    options.extension.multiline_block_quotes = true;
    options.parse.smart = false;
    options.render.r#unsafe = true; // allow raw HTML placeholders to survive
    options.render.hardbreaks = false;
    options
}

struct Ctx<'a> {
    line_starts: &'a [usize],
    registry: &'a PlaceholderRegistry,
}

fn compute_line_starts(src: &str) -> Vec<usize> {
    let mut v = Vec::with_capacity(src.len() / 40 + 1);
    v.push(0);
    for (i, b) in src.bytes().enumerate() {
        if b == b'\n' {
            v.push(i + 1);
        }
    }
    v
}

/// Convert a comrak `Sourcepos` to a byte-range `Span` within the rewritten source.
///
/// Approximation: end is the byte offset of the start of the last character;
/// we treat it as exclusive. Multi-byte characters may produce spans that are
/// a few bytes short, which is acceptable for diagnostic reporting.
fn span_of(sp: Sourcepos, ctx: &Ctx) -> Span {
    let start_line_idx = sp.start.line.saturating_sub(1);
    let end_line_idx = sp.end.line.saturating_sub(1);
    let start = ctx.line_starts.get(start_line_idx).copied().unwrap_or(0)
        + sp.start.column.saturating_sub(1);
    let end = ctx.line_starts.get(end_line_idx).copied().unwrap_or(start) + sp.end.column;
    Span::new(start, end.max(start))
}

fn handle_frontmatter(raw: &str, node: &AstNode, ctx: &Ctx, doc: &mut Document) {
    let span = span_of(node.data.borrow().sourcepos, ctx);
    // `raw` is the full block including fences and the trailing newline. Strip them.
    let inner = strip_frontmatter_fences(raw);
    match serde_yml::from_str::<serde_yml::Value>(inner) {
        Ok(yv) => {
            let value = serde_json::to_value(&yv).unwrap_or(serde_json::Value::Null);
            doc.frontmatter = Some(Frontmatter { value, span });
        }
        Err(e) => {
            doc.diagnostics.push(
                Diagnostic::new(
                    Severity::Warning,
                    codes::INVALID_FRONTMATTER,
                    format!("invalid frontmatter YAML: {e}"),
                )
                .with_span(span),
            );
        }
    }
}

fn strip_frontmatter_fences(raw: &str) -> &str {
    let trimmed = raw.trim_start_matches('\u{FEFF}');
    let after_open = match trimmed.find('\n') {
        Some(i) => &trimmed[i + 1..],
        None => return trimmed,
    };
    if let Some(close) = after_open.rfind("\n---") {
        &after_open[..close]
    } else if let Some(close) = after_open.rfind("---") {
        &after_open[..close]
    } else {
        after_open
    }
}

// ----------------------------------------------------------------------------
// Block conversion
// ----------------------------------------------------------------------------

fn convert_block<'a>(node: &'a AstNode<'a>, ctx: &Ctx, doc: &mut Document) -> Option<Node> {
    let ast = node.data.borrow();
    let span = span_of(ast.sourcepos, ctx);
    match &ast.value {
        NodeValue::Paragraph => Some(Node::Paragraph {
            children: convert_children_inline(node, ctx, doc),
            span,
        }),
        NodeValue::Heading(h) => Some(Node::Heading {
            level: h.level,
            children: convert_children_inline(node, ctx, doc),
            span,
        }),
        NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => Some(Node::BlockQuote {
            children: convert_children_block(node, ctx, doc),
            span,
        }),
        NodeValue::List(list) => {
            let ordered = matches!(list.list_type, ListType::Ordered);
            let start = if ordered {
                Some(list.start as u64)
            } else {
                None
            };
            let items: Vec<Node> = node
                .children()
                .filter_map(|c| convert_block(c, ctx, doc))
                .collect();
            Some(Node::List {
                ordered,
                start,
                items,
                span,
            })
        }
        NodeValue::Item(_) | NodeValue::TaskItem(_) => Some(Node::ListItem {
            children: convert_children_block(node, ctx, doc),
            span,
        }),
        NodeValue::CodeBlock(cb) => {
            let lang = if cb.info.is_empty() {
                None
            } else {
                Some(cb.info.clone())
            };
            Some(Node::CodeBlock {
                language: lang,
                value: cb.literal.clone(),
                span,
            })
        }
        NodeValue::HtmlBlock(html) => {
            let raw = html.literal.trim();
            if let Some(id) = parse_block_placeholder(raw) {
                if let Some(cap) = ctx.registry.block(id) {
                    let (body, extra) = decode_block_body(&cap.raw_body);
                    if let Some(d) = extra {
                        doc.diagnostics.push(d);
                    }
                    return Some(Node::Directive(DirectiveNode {
                        name: cap.name.clone(),
                        attributes: cap.attributes.clone(),
                        body,
                        children: Vec::new(),
                        span: cap.span,
                    }));
                }
            }
            Some(Node::Html {
                value: html.literal.clone(),
                span,
            })
        }
        NodeValue::ThematicBreak => Some(Node::ThematicBreak { span }),
        NodeValue::Table(_) | NodeValue::TableRow(_) | NodeValue::TableCell => {
            // v1: flatten table content into a paragraph; a future milestone can
            // add first-class table nodes.
            Some(Node::Paragraph {
                children: convert_children_inline(node, ctx, doc),
                span,
            })
        }
        NodeValue::FootnoteDefinition(_) => Some(Node::Paragraph {
            children: convert_children_block(node, ctx, doc),
            span,
        }),
        NodeValue::Document | NodeValue::FrontMatter(_) => None,
        // Inline nodes appearing at the block level are wrapped in a paragraph.
        _ => Some(Node::Paragraph {
            children: convert_one_inline(node, ctx, doc).into_iter().collect(),
            span,
        }),
    }
}

fn convert_children_block<'a>(node: &'a AstNode<'a>, ctx: &Ctx, doc: &mut Document) -> Vec<Node> {
    node.children()
        .filter_map(|c| convert_block(c, ctx, doc))
        .collect()
}

// ----------------------------------------------------------------------------
// Inline conversion
// ----------------------------------------------------------------------------

fn convert_children_inline<'a>(node: &'a AstNode<'a>, ctx: &Ctx, doc: &mut Document) -> Vec<Node> {
    let mut out = Vec::new();
    for child in node.children() {
        out.extend(convert_one_inline(child, ctx, doc));
    }
    out
}

fn convert_one_inline<'a>(node: &'a AstNode<'a>, ctx: &Ctx, doc: &mut Document) -> Vec<Node> {
    let ast = node.data.borrow();
    let span = span_of(ast.sourcepos, ctx);
    match &ast.value {
        NodeValue::Text(t) => split_text_for_placeholders(t.as_ref(), span, ctx, doc),
        NodeValue::Code(c) => vec![Node::Code {
            value: c.literal.clone(),
            span,
        }],
        NodeValue::HtmlInline(raw) => vec![Node::Html {
            value: raw.clone(),
            span,
        }],
        NodeValue::SoftBreak => vec![Node::SoftBreak { span }],
        NodeValue::LineBreak => vec![Node::HardBreak { span }],
        NodeValue::Emph => vec![Node::Emphasis {
            children: convert_children_inline(node, ctx, doc),
            span,
        }],
        NodeValue::Strong => vec![Node::Strong {
            children: convert_children_inline(node, ctx, doc),
            span,
        }],
        NodeValue::Strikethrough => vec![Node::Emphasis {
            children: convert_children_inline(node, ctx, doc),
            span,
        }],
        NodeValue::Link(link) => vec![Node::Link(LinkNode {
            kind: LinkKind::StandardUrl {
                url: link.url.clone(),
                title: if link.title.is_empty() {
                    None
                } else {
                    Some(link.title.clone())
                },
            },
            children: convert_children_inline(node, ctx, doc),
            span,
        })],
        NodeValue::Image(link) => vec![Node::Image {
            url: link.url.clone(),
            alt: flatten_inline_text(node),
            title: if link.title.is_empty() {
                None
            } else {
                Some(link.title.clone())
            },
            span,
        }],
        NodeValue::Escaped => convert_children_inline(node, ctx, doc),
        // Unhandled inline variants degrade gracefully to empty (no output).
        _ => Vec::new(),
    }
}

/// Split a comrak `Text` run on inline placeholder sentinels.
fn split_text_for_placeholders(
    text: &str,
    text_span: Span,
    ctx: &Ctx,
    doc: &mut Document,
) -> Vec<Node> {
    // Fast path: no sentinel bytes in this text.
    if !text.contains('\u{E000}') {
        return vec![Node::Text {
            value: text.to_string(),
            span: text_span,
        }];
    }
    let mut out = Vec::new();
    let mut cursor = 0usize;
    let mut i = 0usize;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        // Look for the 3-byte UTF-8 encoding of U+E000: EE 80 80.
        if bytes[i] == 0xEE && i + 2 < bytes.len() && bytes[i + 1] == 0x80 && bytes[i + 2] == 0x80 {
            if let Some((kind, id, consumed)) = peek_inline_placeholder(&text[i..]) {
                if i > cursor {
                    out.push(Node::Text {
                        value: text[cursor..i].to_string(),
                        span: text_span,
                    });
                }
                match kind {
                    InlineKind::Directive => {
                        if let Some(entry) = ctx.registry.inline_directive(id) {
                            out.push(Node::InlineDirective(InlineDirectiveNode {
                                name: entry.name.clone(),
                                attributes: entry.attributes.clone(),
                                raw: entry.raw.clone(),
                                span: entry.span,
                            }));
                        } else {
                            doc.diagnostics.push(Diagnostic::warning(
                                codes::MALFORMED_INLINE_DIRECTIVE,
                                "inline directive placeholder missing from registry",
                            ));
                        }
                    }
                    InlineKind::WikiLink => {
                        if let Some(entry) = ctx.registry.wiki_link(id) {
                            let kind = match &entry.namespace {
                                Some(ns) => LinkKind::NamespacedLink {
                                    namespace: ns.clone(),
                                    target: entry.target.clone(),
                                },
                                None => LinkKind::WikiLink {
                                    target: entry.target.clone(),
                                },
                            };
                            let display =
                                entry.label.clone().unwrap_or_else(|| entry.target.clone());
                            let children = vec![Node::Text {
                                value: display,
                                span: entry.span,
                            }];
                            out.push(Node::Link(LinkNode {
                                kind,
                                children,
                                span: entry.span,
                            }));
                        } else {
                            doc.diagnostics.push(Diagnostic::warning(
                                codes::MALFORMED_WIKI_LINK,
                                "wiki link placeholder missing from registry",
                            ));
                        }
                    }
                }
                i += consumed;
                cursor = i;
                continue;
            }
        }
        i += 1;
    }
    if cursor < bytes.len() {
        out.push(Node::Text {
            value: text[cursor..].to_string(),
            span: text_span,
        });
    }
    out
}

fn parse_block_placeholder(s: &str) -> Option<usize> {
    let inner = s.strip_prefix("<!--mdxb:")?.strip_suffix("-->")?;
    inner.parse().ok()
}

fn flatten_inline_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    collect_text(node, &mut out);
    out
}

fn collect_text<'a>(node: &'a AstNode<'a>, out: &mut String) {
    match &node.data.borrow().value {
        NodeValue::Text(t) => out.push_str(t.as_ref()),
        NodeValue::Code(c) => out.push_str(&c.literal),
        NodeValue::HtmlInline(h) => out.push_str(h),
        NodeValue::SoftBreak | NodeValue::LineBreak => out.push('\n'),
        _ => {
            for c in node.children() {
                collect_text(c, out);
            }
        }
    }
}

fn decode_block_body(raw: &str) -> (DirectiveBody, Option<Diagnostic>) {
    if raw.trim().is_empty() {
        return (DirectiveBody::None, None);
    }
    match serde_yml::from_str::<serde_yml::Value>(raw) {
        Ok(serde_yml::Value::Mapping(m)) => {
            let mut attrs = AttributeMap::new();
            for (k, v) in m {
                let key = match k {
                    serde_yml::Value::String(s) => s,
                    other => serde_json::to_string(&other).unwrap_or_default(),
                };
                let json = serde_json::to_value(&v).unwrap_or(serde_json::Value::Null);
                attrs.insert(key, json);
            }
            (DirectiveBody::Attributes(attrs), None)
        }
        _ => (
            DirectiveBody::Raw(raw.to_string()),
            Some(Diagnostic::lint(
                codes::YAML_BODY_FALLBACK,
                "block directive body is not a YAML mapping; kept as raw text",
            )),
        ),
    }
}

// Suppress unused warning for RefCell re-export check.
#[allow(dead_code)]
fn _assert_refcell_import(_: &RefCell<()>) {}
