//! Golden tests for standard markdown passthrough.
//!
//! These prove the engine behaves correctly when no custom syntax is present,
//! ensuring comrak's output is faithfully projected through our AST.

use mdx_ext::ResolutionMode;
use mdx_integration_tests::{engine_no_runtime, read_fixture};

fn render_html(fixture: &str) -> String {
    let eng = engine_no_runtime(ResolutionMode::Passthrough);
    let doc = eng.parse(&read_fixture(fixture));
    eng.render_html(&doc)
}

#[test]
fn golden_headings() {
    insta::assert_snapshot!(render_html("headings.md"));
}

#[test]
fn golden_emphasis() {
    insta::assert_snapshot!(render_html("emphasis.md"));
}

#[test]
fn golden_code() {
    insta::assert_snapshot!(render_html("code.md"));
}

#[test]
fn golden_lists() {
    insta::assert_snapshot!(render_html("lists.md"));
}

#[test]
fn golden_blockquotes() {
    insta::assert_snapshot!(render_html("blockquotes.md"));
}

#[test]
fn golden_links_images() {
    insta::assert_snapshot!(render_html("links_images.md"));
}

#[test]
fn golden_html_passthrough() {
    insta::assert_snapshot!(render_html("html_passthrough.md"));
}

#[test]
fn golden_frontmatter() {
    insta::assert_snapshot!(render_html("frontmatter.md"));
}
