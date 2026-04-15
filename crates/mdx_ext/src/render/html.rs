//! HTML renderer over the owned `Document` tree.
//!
//! Text nodes are HTML-escaped; `Node::Html` is emitted raw. Attribute
//! iteration is deterministic because `AttributeMap` is backed by a
//! `BTreeMap`. Unresolved directives render as fallback text and components
//! render as `<mdx-component data-name="…" data-prop-*="…">…</mdx-component>`.

use crate::ast::{AttributeMap, DirectiveBody, LinkKind, Node};
use crate::ast::Document;

/// Render `doc` to an HTML string.
pub fn render(doc: &Document) -> String {
    let mut out = String::new();
    for child in &doc.children {
        render_block(child, &mut out);
    }
    out
}

fn render_block(node: &Node, out: &mut String) {
    match node {
        Node::Paragraph { children, .. } => {
            out.push_str("<p>");
            render_inlines(children, out);
            out.push_str("</p>\n");
        }
        Node::Heading { level, children, .. } => {
            let lvl = (*level).clamp(1, 6);
            out.push_str(&format!("<h{lvl}>"));
            render_inlines(children, out);
            out.push_str(&format!("</h{lvl}>\n"));
        }
        Node::BlockQuote { children, .. } => {
            out.push_str("<blockquote>\n");
            for c in children {
                render_block(c, out);
            }
            out.push_str("</blockquote>\n");
        }
        Node::List { ordered, start, items, .. } => {
            if *ordered {
                match start {
                    Some(s) if *s != 1 => out.push_str(&format!("<ol start=\"{s}\">\n")),
                    _ => out.push_str("<ol>\n"),
                }
            } else {
                out.push_str("<ul>\n");
            }
            for item in items {
                render_block(item, out);
            }
            out.push_str(if *ordered { "</ol>\n" } else { "</ul>\n" });
        }
        Node::ListItem { children, .. } => {
            out.push_str("<li>");
            // If the only child is a paragraph, emit its inlines directly (tight list style).
            if children.len() == 1 {
                if let Node::Paragraph { children: inl, .. } = &children[0] {
                    render_inlines(inl, out);
                    out.push_str("</li>\n");
                    return;
                }
            }
            out.push('\n');
            for c in children {
                render_block(c, out);
            }
            out.push_str("</li>\n");
        }
        Node::CodeBlock { language, value, .. } => {
            match language {
                Some(lang) if !lang.is_empty() => {
                    out.push_str(&format!(
                        "<pre><code class=\"language-{}\">",
                        escape_attr(lang)
                    ));
                }
                _ => out.push_str("<pre><code>"),
            }
            out.push_str(&escape_text(value));
            out.push_str("</code></pre>\n");
        }
        Node::Html { value, .. } => {
            out.push_str(value);
            if !value.ends_with('\n') {
                out.push('\n');
            }
        }
        Node::ThematicBreak { .. } => out.push_str("<hr />\n"),
        Node::Directive(d) => {
            // Unresolved directive — render as fallback comment with fallback text.
            out.push_str(&format!(
                "<!-- unresolved directive :{}: -->\n",
                escape_text(&d.name)
            ));
            if let DirectiveBody::Raw(raw) = &d.body {
                out.push_str("<p>");
                out.push_str(&escape_text(raw));
                out.push_str("</p>\n");
            }
            for c in &d.children {
                render_block(c, out);
            }
        }
        Node::Component { name, props, children, .. } => {
            out.push_str(&format!("<mdx-component data-name=\"{}\"", escape_attr(name)));
            render_component_props(props, out);
            out.push('>');
            for c in children {
                render_block(c, out);
            }
            out.push_str("</mdx-component>\n");
        }
        // Inline-only nodes appearing as blocks get wrapped.
        _ => {
            out.push_str("<p>");
            render_inlines(std::slice::from_ref(node), out);
            out.push_str("</p>\n");
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
        Node::Text { value, .. } => out.push_str(&escape_text(value)),
        Node::Emphasis { children, .. } => {
            out.push_str("<em>");
            render_inlines(children, out);
            out.push_str("</em>");
        }
        Node::Strong { children, .. } => {
            out.push_str("<strong>");
            render_inlines(children, out);
            out.push_str("</strong>");
        }
        Node::Code { value, .. } => {
            out.push_str("<code>");
            out.push_str(&escape_text(value));
            out.push_str("</code>");
        }
        Node::SoftBreak { .. } => out.push('\n'),
        Node::HardBreak { .. } => out.push_str("<br />\n"),
        Node::Link(link) => {
            let (href, title) = match &link.kind {
                LinkKind::StandardUrl { url, title } => (url.clone(), title.clone()),
                LinkKind::WikiLink { target } => (format!("#{}", target), None),
                LinkKind::NamespacedLink { namespace, target } => {
                    (format!("{}:{}", namespace, target), None)
                }
            };
            out.push_str(&format!("<a href=\"{}\"", escape_attr(&href)));
            if let Some(t) = title {
                out.push_str(&format!(" title=\"{}\"", escape_attr(&t)));
            }
            match &link.kind {
                LinkKind::WikiLink { .. } => out.push_str(" data-link-kind=\"wiki\""),
                LinkKind::NamespacedLink { namespace, .. } => {
                    out.push_str(&format!(
                        " data-link-kind=\"namespaced\" data-namespace=\"{}\"",
                        escape_attr(namespace)
                    ));
                }
                _ => {}
            }
            out.push('>');
            render_inlines(&link.children, out);
            out.push_str("</a>");
        }
        Node::Image { url, alt, title, .. } => {
            out.push_str(&format!(
                "<img src=\"{}\" alt=\"{}\"",
                escape_attr(url),
                escape_attr(alt)
            ));
            if let Some(t) = title {
                out.push_str(&format!(" title=\"{}\"", escape_attr(t)));
            }
            out.push_str(" />");
        }
        Node::Html { value, .. } => out.push_str(value),
        Node::InlineDirective(d) => {
            out.push_str(&format!("<!-- unresolved inline directive {{{{{}}}}}-->", escape_text(&d.name)));
        }
        Node::Component { name, props, children, .. } => {
            out.push_str(&format!("<mdx-component data-name=\"{}\"", escape_attr(name)));
            render_component_props(props, out);
            out.push('>');
            render_inlines(children, out);
            out.push_str("</mdx-component>");
        }
        // Block nodes accidentally embedded inline: recurse via render_block fallback.
        _ => render_block(node, out),
    }
}

fn render_component_props(props: &AttributeMap, out: &mut String) {
    for (k, v) in props.iter() {
        let serialized = match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        out.push_str(&format!(
            " data-prop-{}=\"{}\"",
            escape_attr(k),
            escape_attr(&serialized)
        ));
    }
}

fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}
