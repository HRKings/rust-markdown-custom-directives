//! End-to-end exercises of the full pipeline: parse → load Lua → resolve → render.

use std::path::PathBuf;

use mdx_ext::{Document, LinkKind, Node, ResolutionMode, RuntimeContext, ScriptSource};
use mdx_integration_tests::engine;

#[test]
fn plain_markdown_renders() {
    let eng = engine(ResolutionMode::Lenient);
    let doc = eng.parse("# Hi\n\nHello *world*.");
    let (resolved, _) = eng.resolve_keep(doc, &RuntimeContext::default());
    let html = eng.render_html(&resolved);
    assert!(html.contains("<h1>Hi</h1>"), "{html}");
    assert!(html.contains("<em>world</em>"), "{html}");
}

#[test]
fn inline_directive_text_output() {
    let mut eng = engine(ResolutionMode::Lenient);
    eng.load_script(ScriptSource::Text(
        r#"
        mdx.register_directive("shout", function(inv)
            return { type = "text", value = "!" .. (inv.attributes.word or "x") }
        end)
        "#
        .to_string(),
    ))
    .unwrap();
    let doc = eng.parse(r#"A {{shout word="hello"}} b."#);
    let (resolved, _) = eng.resolve_keep(doc, &RuntimeContext::default());
    let html = eng.render_html(&resolved);
    assert!(html.contains("!hello"), "{html}");
}

#[test]
fn block_directive_markdown_reparse() {
    let mut eng = engine(ResolutionMode::Lenient);
    eng.load_script(ScriptSource::Text(
        r#"
        mdx.register_directive("callout", function(inv)
            return { type = "markdown", value = "**" .. inv.body.title .. "**" }
        end)
        "#
        .to_string(),
    ))
    .unwrap();
    let src = ":::callout\ntitle: Note\n:::\n";
    let doc = eng.parse(src);
    let (resolved, errs) = eng.resolve_keep(doc, &RuntimeContext::default());
    assert_eq!(errs, 0, "{:?}", resolved.diagnostics);
    let html = eng.render_html(&resolved);
    assert!(html.contains("<strong>Note</strong>"), "{html}");
}

#[test]
fn wiki_link_is_first_class() {
    let eng = engine(ResolutionMode::Passthrough);
    let doc = eng.parse("See [[Home Page|Home]] and [[wiki:Topic]].");
    let html = eng.render_html(&doc);
    assert!(html.contains("data-link-kind=\"wiki\""), "{html}");
    assert!(html.contains("data-link-kind=\"namespaced\""), "{html}");
}

#[test]
fn strict_mode_fails_on_unknown_handler() {
    let eng = engine(ResolutionMode::Strict);
    let doc = eng.parse("A {{missing}} b.");
    let err = eng.resolve(doc, &RuntimeContext::default()).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("strict"), "{msg}");
}

#[test]
fn passthrough_keeps_directive_nodes() {
    let eng = engine(ResolutionMode::Passthrough);
    let doc = eng.parse(":::note\nbody: yes\n:::\n");
    // No runtime invocation: the directive node should still be present.
    let dbg = eng.render_debug(&doc);
    assert!(dbg.contains("directive"), "{dbg}");
}

#[test]
fn sandbox_blocks_io_and_os() {
    let mut eng = engine(ResolutionMode::Lenient);
    let err = eng
        .load_script(ScriptSource::Text("return io.open('x')".to_string()))
        .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("io") || msg.contains("nil"), "{msg}");
}

#[test]
fn unload_script_removes_handler() {
    let mut eng = engine(ResolutionMode::Strict);
    let id = eng
        .load_script(ScriptSource::Text(
            "mdx.register_directive('x', function() return 'hi' end)".into(),
        ))
        .unwrap();
    assert_eq!(eng.list_handlers().len(), 1);
    eng.unload_script(id).unwrap();
    assert_eq!(eng.list_handlers().len(), 0);
}

#[test]
fn file_loaded_lua_script_converts_using_frontmatter() {
    let mut eng = engine(ResolutionMode::Strict);
    eng.load_script(ScriptSource::File(
        convert_example_dir().join("convert.lua"),
    ))
    .unwrap();

    let source = std::fs::read_to_string(convert_example_dir().join("input.md")).unwrap();
    let doc = eng.parse(&source);
    let ctx = RuntimeContext {
        document_metadata: doc.frontmatter.as_ref().map(|fm| fm.value.clone()),
        ..RuntimeContext::default()
    };

    let resolved = eng.resolve(doc, &ctx).unwrap();
    let text = eng.render_text(&resolved);
    assert!(
        text.contains("14.40 km"),
        "expected converted output in rendered text: {text}"
    );
    assert_eq!(text, "The fortress is 14.40 km away.");
}

#[test]
fn file_loaded_lua_script_resolves_namespaced_npc_link() {
    let mut eng = engine(ResolutionMode::Strict);
    eng.load_script(ScriptSource::File(npc_link_example_dir().join("npc.lua")))
        .unwrap();

    let source = std::fs::read_to_string(npc_link_example_dir().join("input.md")).unwrap();
    let doc = eng.parse(&source);
    assert!(
        has_npc_namespaced_link(&doc),
        "expected parsed document to contain a namespaced npc link"
    );

    let ctx = RuntimeContext {
        document_metadata: doc.frontmatter.as_ref().map(|fm| fm.value.clone()),
        ..RuntimeContext::default()
    };

    let resolved = eng.resolve(doc, &ctx).unwrap();
    let html = eng.render_html(&resolved);
    assert!(html.contains("Captain Lyra"), "{html}");
    assert!(html.contains("/codex/npcs/captain-lyra"), "{html}");
    assert!(html.contains("npc-chip"), "{html}");
}

fn convert_example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join("convert")
}

fn npc_link_example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join("npc_link")
}

fn has_npc_namespaced_link(doc: &Document) -> bool {
    doc.children.iter().any(node_has_npc_namespaced_link)
}

fn node_has_npc_namespaced_link(node: &Node) -> bool {
    match node {
        Node::Paragraph { children, .. }
        | Node::Heading { children, .. }
        | Node::Emphasis { children, .. }
        | Node::Strong { children, .. }
        | Node::BlockQuote { children, .. }
        | Node::ListItem { children, .. }
        | Node::Component { children, .. } => children.iter().any(node_has_npc_namespaced_link),
        Node::List { items, .. } => items.iter().any(node_has_npc_namespaced_link),
        Node::Directive(d) => d.children.iter().any(node_has_npc_namespaced_link),
        Node::Link(link) => match &link.kind {
            LinkKind::NamespacedLink { namespace, target } => {
                namespace == "npc" && target == "captain-lyra"
            }
            _ => false,
        },
        _ => false,
    }
}
