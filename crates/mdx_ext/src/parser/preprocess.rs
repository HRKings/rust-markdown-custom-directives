//! Custom syntax preprocessor.
//!
//! Scans the raw source for `:::name ... :::` blocks, `{{name …}}` inline
//! directives, and `[[target|label]]` / `[[ns:target]]` wiki links.
//! Each occurrence is captured into a [`PlaceholderRegistry`] and the source
//! is rewritten to contain deterministic placeholder tokens that will survive
//! `comrak` parsing intact:
//!
//! * Block directives → `<!--mdxb:{id}-->` HTML comment on its own paragraph
//! * Inline directives → `\u{E000}MDXD{id}\u{E001}` private-use sentinel
//! * Wiki links → `\u{E000}MDXL{id}\u{E001}` private-use sentinel
//!
//! The preprocessor does not attempt to understand markdown structure itself —
//! it is a line- and byte-level scanner. Inline scanning is skipped inside
//! triple-backtick / tilde fenced code blocks and indented code blocks, since
//! those are the only places `comrak` would otherwise emit custom syntax as
//! verbatim text.

use crate::ast::AttributeMap;
use crate::diagnostics::{codes, Diagnostic, Severity};
use crate::parser::registry::{
    block_placeholder, inline_token, BlockEntry, InlineDirectiveEntry, InlineKind,
    PlaceholderRegistry, WikiLinkEntry,
};
use crate::span::Span;

/// Rewrite `source` and populate a placeholder registry.
pub fn rewrite(source: &str, doc: &mut crate::ast::Document) -> (String, PlaceholderRegistry) {
    let mut registry = PlaceholderRegistry::new();
    let mut out = String::with_capacity(source.len() + 32);

    // Iterate the source one line at a time so we can detect block directive
    // fences and fenced code block boundaries cheaply.
    let mut i = 0usize;
    let mut in_code_fence: Option<char> = None; // Some('`') or Some('~')
    let mut next_block_id = 0usize;
    let mut next_inline_id = 0usize;
    let mut next_wiki_id = 0usize;

    while i < source.len() {
        let line_start = i;
        let line_end = match source[i..].find('\n') {
            Some(n) => i + n + 1,
            None => source.len(),
        };
        let line_content_end = trim_trailing_newline(source, line_start, line_end);
        let line = &source[line_start..line_content_end];

        // Fenced code-block detection: `` ``` `` or `~~~` on its own line (with optional info).
        if let Some(fence_ch) = detect_fence(line) {
            match in_code_fence {
                None => in_code_fence = Some(fence_ch),
                Some(c) if c == fence_ch && line.trim_start_matches(fence_ch).trim().is_empty() => {
                    in_code_fence = None;
                }
                _ => {}
            }
            out.push_str(&source[line_start..line_end]);
            i = line_end;
            continue;
        }

        // Inside a fenced code block, emit verbatim.
        if in_code_fence.is_some() {
            out.push_str(&source[line_start..line_end]);
            i = line_end;
            continue;
        }

        // Block directive opening on its own line.
        if let Some(open) = parse_open_fence(line) {
            if let Some(end_abs) = find_block_close(source, line_end) {
                let raw_body = source[line_end..end_abs.body_end]
                    .trim_end_matches('\n')
                    .trim_end_matches('\r')
                    .to_string();
                let id = next_block_id;
                next_block_id += 1;
                registry.push_block(BlockEntry {
                    id,
                    name: open.name,
                    attributes: open.attributes,
                    raw_body,
                    span: Span::new(line_start, end_abs.fence_line_end),
                });
                // Emit placeholder on its own paragraph so comrak sees it as an HTML block.
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n');
                out.push_str(&block_placeholder(id));
                out.push_str("\n\n");
                i = end_abs.fence_line_end;
                continue;
            } else {
                doc.diagnostics.push(
                    Diagnostic::new(
                        Severity::Warning,
                        codes::MALFORMED_BLOCK_DIRECTIVE,
                        "unterminated block directive",
                    )
                    .with_span(Span::new(line_start, line_content_end)),
                );
                // Fall through as plain text.
            }
        }

        // Inline scan on the current line.
        rewrite_inline(
            &source[line_start..line_end],
            line_start,
            &mut out,
            &mut registry,
            &mut next_inline_id,
            &mut next_wiki_id,
            doc,
        );
        i = line_end;
    }

    (out, registry)
}

fn trim_trailing_newline(source: &str, start: usize, end: usize) -> usize {
    let mut e = end;
    let b = source.as_bytes();
    if e > start && b[e - 1] == b'\n' {
        e -= 1;
    }
    if e > start && b[e - 1] == b'\r' {
        e -= 1;
    }
    e
}

fn detect_fence(line: &str) -> Option<char> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") {
        return Some('`');
    }
    if trimmed.starts_with("~~~") {
        return Some('~');
    }
    None
}

struct OpenFence {
    name: String,
    attributes: AttributeMap,
}

fn parse_open_fence(line: &str) -> Option<OpenFence> {
    let rest = line.strip_prefix(":::")?;
    if rest.trim().is_empty() {
        return None; // bare ::: is a closer
    }
    let rest = rest.trim_start();
    let (name, after) = split_ident(rest)?;
    if name.is_empty() {
        return None;
    }
    let mut attrs = AttributeMap::new();
    for (k, v) in crate::syntax::parse_inline_attrs(after.trim()) {
        attrs.insert(k, v);
    }
    Some(OpenFence { name: name.to_string(), attributes: attrs })
}

struct BlockClose {
    body_end: usize,       // byte offset of the '\n' before the closing fence line (exclusive)
    fence_line_end: usize, // byte offset just after the closing fence line's newline
}

fn find_block_close(source: &str, start: usize) -> Option<BlockClose> {
    let mut i = start;
    while i < source.len() {
        let line_end = match source[i..].find('\n') {
            Some(n) => i + n + 1,
            None => source.len(),
        };
        let content_end = trim_trailing_newline(source, i, line_end);
        let line = &source[i..content_end];
        if line.trim_end() == ":::" {
            return Some(BlockClose {
                body_end: i,
                fence_line_end: line_end,
            });
        }
        i = line_end;
    }
    None
}

fn split_ident(s: &str) -> Option<(&str, &str)> {
    let end = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(s.len());
    if end == 0 {
        None
    } else {
        Some((&s[..end], &s[end..]))
    }
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn rewrite_inline(
    line: &str,
    line_abs_start: usize,
    out: &mut String,
    registry: &mut PlaceholderRegistry,
    next_inline_id: &mut usize,
    next_wiki_id: &mut usize,
    doc: &mut crate::ast::Document,
) {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    let mut cursor = 0usize;

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(consumed) = try_inline_directive(
                line,
                i,
                line_abs_start,
                registry,
                next_inline_id,
                doc,
                out,
                &mut cursor,
            ) {
                i += consumed;
                continue;
            }
        }
        if i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(consumed) = try_wiki_link(
                line,
                i,
                line_abs_start,
                registry,
                next_wiki_id,
                doc,
                out,
                &mut cursor,
            ) {
                i += consumed;
                continue;
            }
        }
        // Advance by one UTF-8 char.
        i += 1;
        while i < bytes.len() && (bytes[i] & 0b1100_0000) == 0b1000_0000 {
            i += 1;
        }
    }
    // Flush trailing text.
    out.push_str(&line[cursor..]);
}

#[allow(clippy::too_many_arguments)]
fn try_inline_directive(
    line: &str,
    at: usize,
    line_abs_start: usize,
    registry: &mut PlaceholderRegistry,
    next_id: &mut usize,
    doc: &mut crate::ast::Document,
    out: &mut String,
    cursor: &mut usize,
) -> Option<usize> {
    debug_assert_eq!(&line[at..at + 2], "{{");
    let rest = &line[at + 2..];
    let close = rest.find("}}")?;
    let inner = &rest[..close];
    let inner_trim = inner.trim_start();
    let (name, after) = split_ident(inner_trim)?;
    if name.is_empty() {
        doc.diagnostics.push(
            Diagnostic::new(
                Severity::Warning,
                codes::MALFORMED_INLINE_DIRECTIVE,
                "inline directive missing identifier",
            )
            .with_span(Span::new(line_abs_start + at, line_abs_start + at + 2)),
        );
        return None;
    }
    let mut attrs = AttributeMap::new();
    for (k, v) in crate::syntax::parse_inline_attrs(after.trim()) {
        attrs.insert(k, v);
    }
    let consumed = 2 + close + 2;
    let raw = line[at..at + consumed].to_string();
    let id = *next_id;
    *next_id += 1;
    registry.push_inline_directive(InlineDirectiveEntry {
        id,
        name: name.to_string(),
        attributes: attrs,
        raw,
        span: Span::new(line_abs_start + at, line_abs_start + at + consumed),
    });
    out.push_str(&line[*cursor..at]);
    out.push_str(&inline_token(InlineKind::Directive, id));
    *cursor = at + consumed;
    Some(consumed)
}

#[allow(clippy::too_many_arguments)]
fn try_wiki_link(
    line: &str,
    at: usize,
    line_abs_start: usize,
    registry: &mut PlaceholderRegistry,
    next_id: &mut usize,
    doc: &mut crate::ast::Document,
    out: &mut String,
    cursor: &mut usize,
) -> Option<usize> {
    debug_assert_eq!(&line[at..at + 2], "[[");
    let rest = &line[at + 2..];
    let close = rest.find("]]")?;
    let inner = &rest[..close];
    if inner.is_empty() {
        doc.diagnostics.push(
            Diagnostic::new(Severity::Warning, codes::MALFORMED_WIKI_LINK, "empty wiki link")
                .with_span(Span::new(line_abs_start + at, line_abs_start + at + 4)),
        );
        return None;
    }
    let (target_part, label) = match inner.find('|') {
        Some(j) => (&inner[..j], Some(inner[j + 1..].to_string())),
        None => (inner, None),
    };
    let (namespace, target) = if let Some(colon) = target_part.find(':') {
        let ns = &target_part[..colon];
        let tgt = &target_part[colon + 1..];
        if is_ident(ns) && !tgt.is_empty() {
            (Some(ns.to_string()), tgt.to_string())
        } else {
            (None, target_part.to_string())
        }
    } else {
        (None, target_part.to_string())
    };
    let consumed = 2 + close + 2;
    let id = *next_id;
    *next_id += 1;
    registry.push_wiki_link(WikiLinkEntry {
        id,
        target,
        namespace,
        label,
        span: Span::new(line_abs_start + at, line_abs_start + at + consumed),
    });
    out.push_str(&line[*cursor..at]);
    out.push_str(&inline_token(InlineKind::WikiLink, id));
    *cursor = at + consumed;
    Some(consumed)
}
