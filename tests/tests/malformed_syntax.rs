//! Tests for malformed syntax recovery and edge cases.

use mdx_ext::{Node, ResolutionMode};
use mdx_integration_tests::engine_no_runtime;

#[test]
fn unterminated_block_directive() {
    let eng = engine_no_runtime(ResolutionMode::Passthrough);
    let doc = eng.parse(":::myblock\nsome body text\n");
    let has_malformed = doc.diagnostics.iter().any(|d| d.code.as_ref() == "MDX101");
    assert!(
        has_malformed,
        "expected MDX101 for unterminated block directive: {:?}",
        doc.diagnostics
    );
    // The text should fall through as plain content, not a directive node.
    // Check for "(directive " which is the debug format for a directive node,
    // as opposed to the word "directive" appearing in diagnostic messages.
    let dbg = eng.render_debug(&doc);
    assert!(
        !dbg.contains("(directive ") && !dbg.contains("(inline-directive "),
        "unterminated block should not produce a directive node: {dbg}"
    );
}

#[test]
fn empty_inline_directive() {
    let eng = engine_no_runtime(ResolutionMode::Passthrough);
    let doc = eng.parse("text {{}} more");
    // {{}} has no identifier, so the preprocessor doesn't recognize it as a
    // directive at all — it passes through as plain text with no diagnostic.
    // (split_ident("") returns None, short-circuiting before the name check.)
    let html = eng.render_html(&doc);
    assert!(
        html.contains("{{}}"),
        "empty directive should appear as text: {html}"
    );
    // No directive node should be produced.
    let dbg = eng.render_debug(&doc);
    assert!(
        !dbg.contains("(inline-directive "),
        "empty {{}} should not produce a directive node: {dbg}"
    );
}

#[test]
fn empty_wiki_link() {
    let eng = engine_no_runtime(ResolutionMode::Passthrough);
    let doc = eng.parse("text [[]] more");
    let has_malformed = doc.diagnostics.iter().any(|d| d.code.as_ref() == "MDX103");
    assert!(
        has_malformed,
        "expected MDX103 for empty wiki link: {:?}",
        doc.diagnostics
    );
    let html = eng.render_html(&doc);
    assert!(
        html.contains("[[]]"),
        "empty wiki link should appear as text: {html}"
    );
}

#[test]
fn directive_inside_fenced_code_block() {
    let eng = engine_no_runtime(ResolutionMode::Passthrough);
    let src = mdx_integration_tests::read_fixture("directive_in_code_fence.md");
    let doc = eng.parse(&src);
    // No directive nodes should be produced — everything is inside a code fence.
    let has_directive = doc
        .children
        .iter()
        .any(|n| matches!(n, Node::Directive(_) | Node::InlineDirective(_)));
    assert!(
        !has_directive,
        "directives inside fenced code blocks must not be parsed"
    );
    // The code block should contain the literal directive syntax.
    let dbg = eng.render_debug(&doc);
    assert!(
        dbg.contains("code-block"),
        "should contain a code block node: {dbg}"
    );
}

#[test]
fn nested_block_close_first() {
    let eng = engine_no_runtime(ResolutionMode::Passthrough);
    // The first bare ::: closes "outer". The second ::: is stray text.
    let doc = eng.parse(":::outer\n:::inner\nbody\n:::\nafter\n:::\n");
    let dbg = eng.render_debug(&doc);
    // "outer" should be captured as a directive.
    assert!(
        dbg.contains("outer"),
        "outer directive should be captured: {dbg}"
    );
}
