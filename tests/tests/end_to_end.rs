//! End-to-end exercises of the full pipeline: parse → load Lua → resolve → render.

use mdx_ext::{MarkdownEngine, ResolutionMode, RuntimeContext, ScriptSource};
use mdx_lua::LuaRuntime;

fn engine(mode: ResolutionMode) -> MarkdownEngine {
    MarkdownEngine::builder()
        .with_runtime(Box::new(LuaRuntime::new().unwrap()))
        .with_resolution_mode(mode)
        .build()
        .unwrap()
}

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
