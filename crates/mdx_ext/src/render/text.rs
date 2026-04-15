//! Plain-text projection of a `Document`, for previews and search indices.

use crate::ast::{Document, LinkKind, Node};

/// Render `doc` as plain text. Paragraphs are separated by blank lines;
/// emphasis/strong are unwrapped; components render as `[component:name]`.
pub fn render(doc: &Document) -> String {
    let mut out = String::new();
    for (i, child) in doc.children.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        render_block(child, &mut out);
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

fn render_block(node: &Node, out: &mut String) {
    match node {
        Node::Paragraph { children, .. } | Node::Heading { children, .. } => {
            render_inlines(children, out);
            out.push('\n');
        }
        Node::BlockQuote { children, .. } | Node::ListItem { children, .. } => {
            for c in children {
                render_block(c, out);
            }
        }
        Node::List { items, .. } => {
            for item in items {
                out.push_str("- ");
                render_block(item, out);
            }
        }
        Node::CodeBlock { value, .. } => {
            out.push_str(value);
            if !value.ends_with('\n') {
                out.push('\n');
            }
        }
        Node::Html { value, .. } => {
            out.push_str(value);
            out.push('\n');
        }
        Node::ThematicBreak { .. } => out.push_str("---\n"),
        Node::Directive(d) => {
            out.push_str(&format!("[:{}:]\n", d.name));
            for c in &d.children {
                render_block(c, out);
            }
        }
        Node::Component { name, children, .. } => {
            out.push_str(&format!("[component:{name}]"));
            for c in children {
                render_block(c, out);
            }
            out.push('\n');
        }
        _ => {
            render_inline(node, out);
            out.push('\n');
        }
    }
}

fn render_inlines(nodes: &[Node], out: &mut String) {
    for n in nodes {
        render_inline(n, out);
    }
}

fn render_inline(node: &Node, out: &mut String) {
    match node {
        Node::Text { value, .. } | Node::Code { value, .. } => out.push_str(value),
        Node::Emphasis { children, .. } | Node::Strong { children, .. } => {
            render_inlines(children, out);
        }
        Node::SoftBreak { .. } => out.push(' '),
        Node::HardBreak { .. } => out.push('\n'),
        Node::Link(link) => {
            render_inlines(&link.children, out);
            match &link.kind {
                LinkKind::StandardUrl { url, .. } => {
                    out.push_str(&format!(" ({url})"));
                }
                LinkKind::WikiLink { target } => {
                    out.push_str(&format!(" ([[{target}]])"));
                }
                LinkKind::NamespacedLink { namespace, target } => {
                    out.push_str(&format!(" ([[{namespace}:{target}]])"));
                }
            }
        }
        Node::Image { alt, .. } => out.push_str(alt),
        Node::InlineDirective(d) => out.push_str(&format!("{{{{{}}}}}", d.name)),
        Node::Component { name, children, .. } => {
            out.push_str(&format!("[component:{name}]"));
            render_inlines(children, out);
        }
        Node::Html { value, .. } => out.push_str(value),
        _ => {}
    }
}
