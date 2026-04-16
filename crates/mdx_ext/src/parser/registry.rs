//! Placeholder registry used by the preprocess → comrak roundtrip.
//!
//! The preprocessor replaces custom syntax in the source with deterministic
//! placeholder tokens and stores the original payload in this registry. After
//! `comrak` has parsed the rewritten source, the adapter walks the arena and
//! restores the payloads into first-class AST nodes.

use crate::ast::AttributeMap;
use crate::span::Span;

/// Private Use Area sentinels that survive `comrak`'s inline parsing intact
/// (they're treated as ordinary text characters).
pub const INLINE_OPEN: char = '\u{E000}';
pub const INLINE_CLOSE: char = '\u{E001}';

pub fn inline_token(kind: InlineKind, id: usize) -> String {
    let tag = match kind {
        InlineKind::Directive => 'D',
        InlineKind::WikiLink => 'L',
    };
    format!("{INLINE_OPEN}MDX{tag}{id}{INLINE_CLOSE}")
}

pub fn block_placeholder(id: usize) -> String {
    format!("<!--mdxb:{id}-->")
}

#[derive(Debug, Clone, Copy)]
pub enum InlineKind {
    Directive,
    WikiLink,
}

#[derive(Debug, Clone)]
pub struct BlockEntry {
    pub id: usize,
    pub name: String,
    pub attributes: AttributeMap,
    pub raw_body: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct InlineDirectiveEntry {
    pub id: usize,
    pub name: String,
    pub attributes: AttributeMap,
    pub raw: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct WikiLinkEntry {
    pub id: usize,
    pub target: String,
    pub namespace: Option<String>,
    pub label: Option<String>,
    pub span: Span,
}

#[derive(Debug, Default)]
pub struct PlaceholderRegistry {
    pub blocks: Vec<BlockEntry>,
    pub inline_directives: Vec<InlineDirectiveEntry>,
    pub wiki_links: Vec<WikiLinkEntry>,
}

impl PlaceholderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_block(&mut self, entry: BlockEntry) {
        self.blocks.push(entry);
    }

    pub fn push_inline_directive(&mut self, entry: InlineDirectiveEntry) {
        self.inline_directives.push(entry);
    }

    pub fn push_wiki_link(&mut self, entry: WikiLinkEntry) {
        self.wiki_links.push(entry);
    }

    pub fn block(&self, id: usize) -> Option<&BlockEntry> {
        self.blocks.iter().find(|b| b.id == id)
    }
    pub fn inline_directive(&self, id: usize) -> Option<&InlineDirectiveEntry> {
        self.inline_directives.iter().find(|b| b.id == id)
    }
    pub fn wiki_link(&self, id: usize) -> Option<&WikiLinkEntry> {
        self.wiki_links.iter().find(|b| b.id == id)
    }
}

/// Try to parse an inline placeholder starting at `s[0..]`.
/// Returns `(kind, id, byte_length_of_token_in_s)` on success.
pub fn peek_inline_placeholder(s: &str) -> Option<(InlineKind, usize, usize)> {
    let mut chars = s.char_indices();
    let (_, c0) = chars.next()?;
    if c0 != INLINE_OPEN {
        return None;
    }
    let rest = &s[c0.len_utf8()..];
    let rest_bytes = rest.as_bytes();
    if !rest.starts_with("MDX") || rest_bytes.len() < 4 {
        return None;
    }
    let tag = rest_bytes[3] as char;
    let kind = match tag {
        'D' => InlineKind::Directive,
        'L' => InlineKind::WikiLink,
        _ => return None,
    };
    let digits = &rest[4..];
    let mut id_end = 0usize;
    for b in digits.bytes() {
        if b.is_ascii_digit() {
            id_end += 1;
        } else {
            break;
        }
    }
    if id_end == 0 {
        return None;
    }
    let id: usize = digits[..id_end].parse().ok()?;
    let after_digits = &digits[id_end..];
    if !after_digits.starts_with(INLINE_CLOSE) {
        return None;
    }
    let total = c0.len_utf8() + 3 /* MDX */ + 1 /* tag */ + id_end + INLINE_CLOSE.len_utf8();
    Some((kind, id, total))
}
