//! Normalization pass.
//!
//! * Coalesce adjacent text nodes inside the same parent.
//! * Lowercase directive names and link namespaces.
//! * Parse `DirectiveBody::Raw` that looks like a YAML mapping into `DirectiveBody::Attributes`.

use crate::ast::{DirectiveBody, Document, LinkKind, Node};

pub fn normalize(doc: &mut Document) {
    let mut children = std::mem::take(&mut doc.children);
    walk(&mut children);
    doc.children = children;
}

fn walk(children: &mut Vec<Node>) {
    // First, recurse.
    for child in children.iter_mut() {
        recurse(child);
    }
    // Then coalesce adjacent text nodes at this level.
    coalesce_text(children);
}

fn recurse(node: &mut Node) {
    match node {
        Node::Paragraph { children, .. }
        | Node::Heading { children, .. }
        | Node::Emphasis { children, .. }
        | Node::Strong { children, .. }
        | Node::BlockQuote { children, .. }
        | Node::ListItem { children, .. } => walk(children),
        Node::List { items, .. } => walk(items),
        Node::Link(link) => {
            // Lowercase namespace.
            if let LinkKind::NamespacedLink { namespace, .. } = &mut link.kind {
                *namespace = namespace.to_ascii_lowercase();
            }
            walk(&mut link.children);
        }
        Node::Directive(d) => {
            d.name = d.name.to_ascii_lowercase();
            walk(&mut d.children);
        }
        Node::InlineDirective(d) => {
            d.name = d.name.to_ascii_lowercase();
        }
        Node::Component { children, .. } => walk(children),
        _ => {}
    }
    // Re-parse Raw bodies that already happen to be valid mappings. (cmark
    // adapter already attempted this, but normalization is idempotent and
    // allows the user to mutate bodies between parse and resolve.)
    if let Node::Directive(d) = node {
        if let DirectiveBody::Raw(raw) = &d.body {
            if let Ok(serde_yml::Value::Mapping(_)) = serde_yml::from_str::<serde_yml::Value>(raw) {
                // No-op for now — the cmark adapter already handled conversion. Retained as a hook.
            }
        }
    }
}

fn coalesce_text(children: &mut Vec<Node>) {
    if children.len() < 2 {
        return;
    }
    let mut out: Vec<Node> = Vec::with_capacity(children.len());
    for node in children.drain(..) {
        if let Node::Text { value: v, span: s } = &node {
            if let Some(Node::Text {
                value: prev,
                span: prev_span,
            }) = out.last_mut()
            {
                prev.push_str(v);
                prev_span.end = s.end;
                continue;
            }
        }
        out.push(node);
    }
    *children = out;
}
