//! Tests for reparse depth limits and directive budget exhaustion.

use mdx_ext::{ReparseLimits, ResolutionMode, RuntimeContext, ScriptSource};
use mdx_integration_tests::{engine, engine_with_limits, read_script};

#[test]
fn self_recursive_depth_limit() {
    let mut eng = engine(ResolutionMode::Lenient);
    eng.load_script(ScriptSource::Text(read_script("recursive_self.lua")))
        .unwrap();
    let doc = eng.parse("{{recursive_self}}");
    let (resolved, _) = eng.resolve_keep(doc, &RuntimeContext::default());
    let has_depth_exceeded = resolved
        .diagnostics
        .iter()
        .any(|d| d.code.as_ref() == "MDX401");
    assert!(
        has_depth_exceeded,
        "expected MDX401 reparse depth exceeded diagnostic, got: {:?}",
        resolved.diagnostics
    );
}

#[test]
fn mutual_recursion_depth_limit() {
    let mut eng = engine(ResolutionMode::Lenient);
    eng.load_script(ScriptSource::Text(read_script("mutual_a_b.lua")))
        .unwrap();
    let doc = eng.parse("{{ping}}");
    let (resolved, _) = eng.resolve_keep(doc, &RuntimeContext::default());
    let has_depth_exceeded = resolved
        .diagnostics
        .iter()
        .any(|d| d.code.as_ref() == "MDX401");
    assert!(
        has_depth_exceeded,
        "expected MDX401 for mutual recursion, got: {:?}",
        resolved.diagnostics
    );
}

#[test]
fn directive_budget_exhaustion() {
    let mut eng = engine(ResolutionMode::Lenient);
    eng.load_script(ScriptSource::Text(read_script("budget_buster.lua")))
        .unwrap();
    let doc = eng.parse("{{spawn}}");
    let (resolved, _) = eng.resolve_keep(doc, &RuntimeContext::default());
    let has_budget_exceeded = resolved
        .diagnostics
        .iter()
        .any(|d| d.code.as_ref() == "MDX402");
    assert!(
        has_budget_exceeded,
        "expected MDX402 directive budget exceeded, got: {:?}",
        resolved.diagnostics
    );
}

#[test]
fn normal_reparse_multilevel() {
    let mut eng = engine(ResolutionMode::Lenient);
    eng.load_script(ScriptSource::Text(
        r#"
        mdx.register_directive("level1", function(inv)
            return { type = "markdown", value = "resolved: {{level2}}" }
        end)
        mdx.register_directive("level2", function(inv)
            return { type = "text", value = "DONE" }
        end)
        "#
        .to_string(),
    ))
    .unwrap();
    let doc = eng.parse("{{level1}}");
    let (resolved, errs) = eng.resolve_keep(doc, &RuntimeContext::default());
    assert_eq!(errs, 0, "no errors expected: {:?}", resolved.diagnostics);
    let html = eng.render_html(&resolved);
    assert!(html.contains("DONE"), "expected DONE in output: {html}");
    // No depth/budget diagnostics.
    let bad_diags: Vec<_> = resolved
        .diagnostics
        .iter()
        .filter(|d| d.code.as_ref() == "MDX401" || d.code.as_ref() == "MDX402")
        .collect();
    assert!(
        bad_diags.is_empty(),
        "unexpected limit diagnostics: {bad_diags:?}"
    );
}

#[test]
fn custom_depth_limit_respected() {
    let limits = ReparseLimits {
        max_reparse_depth: 1,
        max_directives_per_document: 1024,
    };
    let mut eng = engine_with_limits(ResolutionMode::Lenient, limits);
    eng.load_script(ScriptSource::Text(
        r#"
        mdx.register_directive("nest", function(inv)
            return { type = "markdown", value = "inner {{nest}}" }
        end)
        "#
        .to_string(),
    ))
    .unwrap();
    let doc = eng.parse("{{nest}}");
    let (resolved, _) = eng.resolve_keep(doc, &RuntimeContext::default());
    let has_depth_exceeded = resolved
        .diagnostics
        .iter()
        .any(|d| d.code.as_ref() == "MDX401");
    assert!(
        has_depth_exceeded,
        "depth limit of 1 should trigger MDX401: {:?}",
        resolved.diagnostics
    );
}
