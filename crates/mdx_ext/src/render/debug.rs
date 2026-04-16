//! Indented s-expression-style debug dump of a `Document`, used by snapshot tests.

use crate::ast::{Document, LinkKind, Node};

/// Render `doc` as a debug tree.
pub fn render(doc: &Document) -> String {
    let mut out = String::new();
    out.push_str("(document");
    if let Some(fm) = &doc.frontmatter {
        out.push_str(&format!("\n  (frontmatter {} {:?})", fm.value, fm.span));
    }
    for child in &doc.children {
        out.push('\n');
        render_node(child, 1, &mut out);
    }
    if !doc.diagnostics.is_empty() {
        out.push_str("\n  (diagnostics");
        for d in &doc.diagnostics {
            out.push_str(&format!(
                "\n    ({:?} {} {:?})",
                d.severity, d.code, d.message
            ));
        }
        out.push(')');
    }
    out.push(')');
    out
}

fn indent(level: usize, out: &mut String) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn render_node(node: &Node, level: usize, out: &mut String) {
    indent(level, out);
    match node {
        Node::Paragraph { children, span } => {
            out.push_str(&format!("(paragraph {:?}", span));
            render_children(children, level, out);
            out.push(')');
        }
        Node::Heading {
            level: h,
            children,
            span,
        } => {
            out.push_str(&format!("(heading {} {:?}", h, span));
            render_children(children, level, out);
            out.push(')');
        }
        Node::Text { value, span } => {
            out.push_str(&format!("(text {:?} {:?})", value, span));
        }
        Node::Emphasis { children, span } => {
            out.push_str(&format!("(em {:?}", span));
            render_children(children, level, out);
            out.push(')');
        }
        Node::Strong { children, span } => {
            out.push_str(&format!("(strong {:?}", span));
            render_children(children, level, out);
            out.push(')');
        }
        Node::Code { value, span } => {
            out.push_str(&format!("(code {:?} {:?})", value, span));
        }
        Node::CodeBlock {
            language,
            value,
            span,
        } => {
            out.push_str(&format!(
                "(code-block {:?} {:?} {:?})",
                language, value, span
            ));
        }
        Node::List {
            ordered,
            start,
            items,
            span,
        } => {
            out.push_str(&format!(
                "(list ordered={} start={:?} {:?}",
                ordered, start, span
            ));
            render_children(items, level, out);
            out.push(')');
        }
        Node::ListItem { children, span } => {
            out.push_str(&format!("(item {:?}", span));
            render_children(children, level, out);
            out.push(')');
        }
        Node::BlockQuote { children, span } => {
            out.push_str(&format!("(blockquote {:?}", span));
            render_children(children, level, out);
            out.push(')');
        }
        Node::Link(link) => {
            let tag = match &link.kind {
                LinkKind::StandardUrl { url, title } => {
                    format!("link url={:?} title={:?}", url, title)
                }
                LinkKind::WikiLink { target } => format!("wiki-link target={:?}", target),
                LinkKind::NamespacedLink { namespace, target } => {
                    format!("ns-link {:?}:{:?}", namespace, target)
                }
            };
            out.push_str(&format!("({} {:?}", tag, link.span));
            render_children(&link.children, level, out);
            out.push(')');
        }
        Node::Image {
            url,
            alt,
            title,
            span,
        } => {
            out.push_str(&format!(
                "(image url={:?} alt={:?} title={:?} {:?})",
                url, alt, title, span
            ));
        }
        Node::Directive(d) => {
            out.push_str(&format!(
                "(directive name={:?} attrs={} body={:?} {:?}",
                d.name,
                serde_json::to_string(&d.attributes).unwrap_or_default(),
                d.body,
                d.span
            ));
            render_children(&d.children, level, out);
            out.push(')');
        }
        Node::InlineDirective(d) => {
            out.push_str(&format!(
                "(inline-directive name={:?} attrs={} raw={:?} {:?})",
                d.name,
                serde_json::to_string(&d.attributes).unwrap_or_default(),
                d.raw,
                d.span
            ));
        }
        Node::Html { value, span } => {
            out.push_str(&format!("(html {:?} {:?})", value, span));
        }
        Node::ThematicBreak { span } => {
            out.push_str(&format!("(thematic-break {:?})", span));
        }
        Node::Component {
            name,
            props,
            children,
            span,
        } => {
            out.push_str(&format!(
                "(component name={:?} props={} {:?}",
                name,
                serde_json::to_string(props).unwrap_or_default(),
                span
            ));
            render_children(children, level, out);
            out.push(')');
        }
        Node::SoftBreak { span } => out.push_str(&format!("(soft-break {:?})", span)),
        Node::HardBreak { span } => out.push_str(&format!("(hard-break {:?})", span)),
    }
}

fn render_children(children: &[Node], level: usize, out: &mut String) {
    for c in children {
        out.push('\n');
        render_node(c, level + 1, out);
    }
}
