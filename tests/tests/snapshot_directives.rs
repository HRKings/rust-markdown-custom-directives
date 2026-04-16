//! Snapshot tests for directive and wiki-link AST structures.

use mdx_ext::ResolutionMode;
use mdx_integration_tests::engine_no_runtime;

fn debug_parse(src: &str) -> String {
    let eng = engine_no_runtime(ResolutionMode::Passthrough);
    let doc = eng.parse(src);
    eng.render_debug(&doc)
}

#[test]
fn snapshot_inline_directive_ast() {
    insta::assert_snapshot!(debug_parse(r#"text {{name key="val"}} more"#));
}

#[test]
fn snapshot_block_directive_ast() {
    insta::assert_snapshot!(debug_parse(":::note\ntitle: Hello\n:::\n"));
}

#[test]
fn snapshot_wiki_link_plain() {
    insta::assert_snapshot!(debug_parse("See [[Home Page|Home]]."));
}

#[test]
fn snapshot_wiki_link_namespaced() {
    insta::assert_snapshot!(debug_parse("Link to [[wiki:Topic]]."));
}

#[test]
fn snapshot_malformed_diagnostics() {
    // Combine several malformed inputs to snapshot their diagnostics.
    let eng = engine_no_runtime(ResolutionMode::Passthrough);
    let doc = eng.parse(":::unterminated\nbody\n\n{{}} and [[]]");
    insta::assert_snapshot!(eng.render_debug(&doc));
}
