# mdx — markdown engine with runtime-loaded directives

Standalone Rust workspace providing a markdown parser with first-class block
and inline directives, wiki links, and runtime-loaded Lua directive handlers.

## Crates

- `mdx_core` — parser, AST, transforms, renderers, runtime traits
- `mdx_lua`  — optional Lua runtime (via `mlua`) implementing `DirectiveRuntime`

## Status

Pre-1.0. See `crates/mdx_core/src/lib.rs` for the public API surface.
