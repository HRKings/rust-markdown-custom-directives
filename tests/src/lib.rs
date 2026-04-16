//! Shared helpers for integration tests.

use mdx_ext::{MarkdownEngine, ReparseLimits, ResolutionMode};
use mdx_lua::LuaRuntime;

/// Build an engine with a fresh Lua runtime and the given resolution mode.
pub fn engine(mode: ResolutionMode) -> MarkdownEngine {
    MarkdownEngine::builder()
        .with_runtime(Box::new(LuaRuntime::new().unwrap()))
        .with_resolution_mode(mode)
        .build()
        .unwrap()
}

/// Build an engine with custom reparse limits (for recursion/budget tests).
pub fn engine_with_limits(mode: ResolutionMode, limits: ReparseLimits) -> MarkdownEngine {
    MarkdownEngine::builder()
        .with_runtime(Box::new(LuaRuntime::new().unwrap()))
        .with_resolution_mode(mode)
        .with_reparse_limits(limits)
        .build()
        .unwrap()
}

/// Build an engine with no runtime (passthrough / parse-only tests).
pub fn engine_no_runtime(mode: ResolutionMode) -> MarkdownEngine {
    MarkdownEngine::builder()
        .with_resolution_mode(mode)
        .build()
        .unwrap()
}

/// Path to a fixture file relative to the workspace root.
pub fn fixture_path(name: &str) -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("fixtures");
    p.push(name);
    p
}

/// Read a fixture file to a string.
pub fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(fixture_path(name))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

/// Read a Lua script to a string.
pub fn read_script(name: &str) -> String {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("scripts");
    p.push(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("script {name}: {e}"))
}
