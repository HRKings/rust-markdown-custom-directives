//! Tests for each DirectiveOutput variant through the full pipeline.

use mdx_ext::{ResolutionMode, RuntimeContext, ScriptSource};
use mdx_integration_tests::{engine, read_script};

#[test]
fn html_output() {
    let mut eng = engine(ResolutionMode::Lenient);
    eng.load_script(ScriptSource::Text(read_script("html_raw.lua")))
        .unwrap();
    let doc = eng.parse(r#"{{html_raw content="hello"}}"#);
    let (resolved, errs) = eng.resolve_keep(doc, &RuntimeContext::default());
    assert_eq!(errs, 0, "{:?}", resolved.diagnostics);
    let html = eng.render_html(&resolved);
    assert!(
        html.contains(r#"<div class="custom">hello</div>"#),
        "raw HTML should appear unescaped: {html}"
    );
}

#[test]
fn component_output() {
    let mut eng = engine(ResolutionMode::Lenient);
    eng.load_script(ScriptSource::Text(read_script("component.lua")))
        .unwrap();
    let doc = eng.parse(r#"{{alert level="warning"}}"#);
    let (resolved, errs) = eng.resolve_keep(doc, &RuntimeContext::default());
    assert_eq!(errs, 0, "{:?}", resolved.diagnostics);
    let html = eng.render_html(&resolved);
    assert!(
        html.contains("mdx-component"),
        "should render as mdx-component: {html}"
    );
    assert!(
        html.contains("data-name=\"Alert\""),
        "should have component name: {html}"
    );
    assert!(
        html.contains("data-prop-level=\"warning\""),
        "should have prop: {html}"
    );
}

#[test]
fn data_output() {
    let mut eng = engine(ResolutionMode::Lenient);
    eng.load_script(ScriptSource::Text(read_script("data.lua")))
        .unwrap();
    let doc = eng.parse("{{data_source}}");
    let (resolved, _) = eng.resolve_keep(doc, &RuntimeContext::default());
    // Data variant emits an Info-severity diagnostic.
    let has_info = resolved
        .diagnostics
        .iter()
        .any(|d| d.code.as_ref() == "MDX203");
    assert!(
        has_info,
        "expected MDX203 info for data output: {:?}",
        resolved.diagnostics
    );
    let html = eng.render_html(&resolved);
    // Serialized JSON should appear in the text.
    assert!(html.contains("42"), "data should be serialized: {html}");
}

#[test]
fn error_in_lenient_mode() {
    let mut eng = engine(ResolutionMode::Lenient);
    eng.load_script(ScriptSource::Text(read_script("error.lua")))
        .unwrap();
    let doc = eng.parse("{{fail}}");
    let (resolved, _) = eng.resolve_keep(doc, &RuntimeContext::default());
    let has_warning = resolved
        .diagnostics
        .iter()
        .any(|d| d.code.as_ref() == "MDX202");
    assert!(
        has_warning,
        "expected MDX202 warning for error output in lenient mode: {:?}",
        resolved.diagnostics
    );
    let html = eng.render_html(&resolved);
    // Lenient fallback text should contain the handler name.
    assert!(
        html.contains("[fail]") || html.contains("[&lt;returned error&gt;]"),
        "lenient fallback should be visible: {html}"
    );
}

#[test]
fn error_in_strict_mode() {
    let mut eng = engine(ResolutionMode::Strict);
    eng.load_script(ScriptSource::Text(read_script("error.lua")))
        .unwrap();
    let doc = eng.parse("{{fail}}");
    let result = eng.resolve(doc, &RuntimeContext::default());
    assert!(
        result.is_err(),
        "strict mode should return Err on handler error"
    );
}

#[test]
fn text_output() {
    let mut eng = engine(ResolutionMode::Lenient);
    eng.load_script(ScriptSource::Text(read_script("shout.lua")))
        .unwrap();
    let doc = eng.parse(r#"{{shout word="hello"}}"#);
    let (resolved, errs) = eng.resolve_keep(doc, &RuntimeContext::default());
    assert_eq!(errs, 0, "{:?}", resolved.diagnostics);
    let html = eng.render_html(&resolved);
    assert!(html.contains("HELLO"), "shout should uppercase: {html}");
}

#[test]
fn markdown_single_level() {
    let mut eng = engine(ResolutionMode::Lenient);
    eng.load_script(ScriptSource::Text(
        r#"
        mdx.register_directive("bold_wrap", function(inv)
            local word = inv.attributes.word or "text"
            return { type = "markdown", value = "**" .. word .. "**" }
        end)
        "#
        .to_string(),
    ))
    .unwrap();
    let doc = eng.parse(r#"{{bold_wrap word="hello"}}"#);
    let (resolved, errs) = eng.resolve_keep(doc, &RuntimeContext::default());
    assert_eq!(errs, 0, "{:?}", resolved.diagnostics);
    let html = eng.render_html(&resolved);
    assert!(
        html.contains("<strong>hello</strong>"),
        "markdown reparse should produce strong: {html}"
    );
}
