# mdx — Markdown with Runtime-Loaded Directives

A Rust library for parsing markdown with first-class **block directives**,
**inline directives**, **wiki links**, and **namespaced semantic links**.
Behavior can be extended at runtime through Lua handlers, so you can add
directive logic or semantic-link rendering without recompiling the host app.

Built on top of [comrak](https://crates.io/crates/comrak) (CommonMark + GFM),
so standard markdown keeps working as-is. The extra syntax is layered on top
without forking or replacing the underlying markdown parser.

## Custom Syntax

### Block directives

```markdown
:::statblock
name: Captain Lyra
role: Harbor Master
strength: 12
:::
```

The body between `:::name` and `:::` is decoded in one of two ways:

- YAML mapping bodies become `inv.body` as a Lua table
- Non-mapping bodies stay as raw text in `inv.body`

```markdown
:::myblock
alpha -> beta
beta -> gamma
gamma -> delta
:::
```

In Lua, `inv.body` will be:

- a table for YAML-like key/value bodies
- a string for raw custom text
- `nil` for an empty block

### Inline directives

```markdown
The fortress is {{convert value=3 from="league" to="km"}} away.
```

### Wiki links

```markdown
See [[Home Page]] or [[Home Page|custom label]].
```

### Namespaced links

```markdown
Talk to [[npc:captain-lyra]] at the harbor.
```

Namespaced links are resolved by `register_link_resolver` handlers, allowing domain-specific link types.

## Crates

| Crate                       | Description                                                                              |
| --------------------------- | ---------------------------------------------------------------------------------------- |
| [`mdx_ext`](crates/mdx_ext) | Parser, AST, transforms, renderers, and runtime traits. The core library.                |
| [`mdx_lua`](crates/mdx_lua) | Lua runtime (via [mlua](https://crates.io/crates/mlua)) implementing `DirectiveRuntime`. |

## Quick Start

Use `mdx_ext` when you only need parsing / rendering infrastructure, and use
`mdx_lua` when you want runtime-loaded handlers.

```rust
use std::error::Error;

use mdx_ext::{MarkdownEngine, ResolutionMode, RuntimeContext, ScriptSource};
use mdx_lua::LuaRuntime;

fn main() -> Result<(), Box<dyn Error>> {
    let source = r#"
---
units:
  league_km: 4.8
---

The fortress is {{convert value=3 from="league" to="km"}} away.
"#;

    let mut engine = MarkdownEngine::builder()
        .with_runtime(Box::new(LuaRuntime::new()?))
        .with_resolution_mode(ResolutionMode::Strict)
        .build()?;

    engine.load_script(ScriptSource::File("handlers.lua".into()))?;

    let doc = engine.parse(source);
    let ctx = RuntimeContext {
        document_metadata: doc.frontmatter.as_ref().map(|fm| fm.value.clone()),
        ..RuntimeContext::default()
    };
    let resolved = engine.resolve(doc, &ctx)?;
    let html = engine.render_html(&resolved);

    println!("{html}");
    Ok(())
}
```

## Writing Handlers

Handlers are plain Lua scripts that call `mdx.register_directive` or
`mdx.register_link_resolver`. They receive the directive or link invocation
plus a context table and return a structured output.

### Directive handler

```lua
mdx.register_directive("convert", function(inv, ctx)
    local value = inv.attributes.value
    local factor = ctx.document_metadata.units.league_km or 4.8
    return {
        type = "text",
        value = string.format("%.2f km", value * factor)
    }
end)
```

For block directives, `inv.body` is shape-dependent:

- Lua table for YAML-mapping bodies such as `name: Captain Lyra`
- string for raw custom bodies such as a DSL or free-form text
- `nil` for an empty block

### Link resolver

```lua
mdx.register_link_resolver("npc", function(link, ctx)
    local npc = ctx.document_metadata.npcs[link.target]
    return {
        type = "html",
        value = string.format('<a class="npc-link" href="%s">%s</a>', npc.href, npc.name)
    }
end)
```

### Return types

| `type`        | Effect                                                                   |
| ------------- | ------------------------------------------------------------------------ |
| `"text"`      | Plain text, HTML-escaped on render.                                      |
| `"html"`      | Raw HTML, emitted verbatim.                                              |
| `"markdown"`  | Reparsed by the engine (subject to depth limits).                        |
| `"component"` | Rendered as `<mdx-component data-name="..." data-prop-*="...">`.         |
| `"data"`      | Serialized as JSON text; emits an informational diagnostic.              |
| `"error"`     | Handled per resolution mode (hard error in Strict, fallback in Lenient). |

A plain string return is shorthand for `{ type = "text", value = s }`.

## Resolution Modes

| Mode                  | Behavior                                                                                               |
| --------------------- | ------------------------------------------------------------------------------------------------------ |
| **Strict**            | Unknown handlers and runtime errors produce error-severity diagnostics. `resolve()` returns `Err`.     |
| **Lenient** (default) | Unknown/failed directives become `[name]` fallback text with a warning diagnostic.                     |
| **Passthrough**       | Resolution is skipped entirely. Directive nodes remain in the AST for tooling use (linters, indexers). |

## Examples

Three runnable end-to-end examples live in [`examples/`](examples/). Each one
includes input markdown, a Lua handler, and a small Rust program that loads the
handler dynamically and renders the result.

### Unit conversion (`examples/convert`)

Shows **inline directive resolution**. The `convert` directive turns leagues
into kilometers using a factor defined in frontmatter.

```
cargo run -p mdx_lua --example convert
# If you use a rustc wrapper (sccache, mold, etc.) that chokes on vendored C builds:
cargo --config build.rustc-wrapper='""' run -p mdx_lua --example convert
```

**Input** (`examples/convert/input.md`):

```markdown
---
title: Travel Notes
units:
  league_km: 4.8
---

The fortress is {{convert value=3 from="league" to="km"}} away.
```

**Output**: `The fortress is 14.40 km away.`

### NPC link chips (`examples/npc_link`)

Shows **semantic namespaced link resolution**. `[[npc:captain-lyra]]` is kept
as a namespaced link in the AST, then resolved into a richer HTML chip sourced
from frontmatter.

```
cargo --config build.rustc-wrapper='""' run -p mdx_lua --example npc_link
```

### Stat block (`examples/statblock`)

Shows **block directive resolution**. `:::statblock ... :::` is parsed as a
block directive node, its YAML body is exposed to Lua as structured attributes,
and the handler renders rich HTML.

```
cargo --config build.rustc-wrapper='""' run -p mdx_lua --example statblock
```

**Input** (`examples/statblock/input.md`):

```markdown
:::statblock
name: Captain Lyra
role: Harbor Master
faction: Azure Fleet
strength: 12
agility: 15
willpower: 14
:::
```

There is also a minimal parse-only example in `mdx_ext`:

```
cargo --config build.rustc-wrapper='""' run -p mdx_ext --example render -- path/to/file.md
```

This renders HTML with `Passthrough` mode and no runtime, so directives are
preserved rather than executed. That is useful for parser/tooling workflows,
but it is not the example to use when you want Lua-backed resolution.

## Safety and Limits

The Lua runtime is sandboxed: `io`, `os`, `package`, `require`, `dofile`, `loadfile`, `load`, `debug`, and `collectgarbage` are stripped from the global environment. Scripts cannot access the filesystem or load external code.

Recursive directive expansion is bounded by `ReparseLimits`:

- **`max_reparse_depth`** (default 4): How many layers of `DirectiveOutput::Markdown` nesting are allowed.
- **`max_directives_per_document`** (default 1024): Total directive invocations across the entire document, including reparsed expansions.

Both limits emit diagnostics (`MDX401`, `MDX402`) when exceeded.

## Diagnostics

All parse and resolution errors are structured `Diagnostic` values with severity, code, message, and optional source span. No panics for user-authored content.

| Code     | Meaning                                       |
| -------- | --------------------------------------------- |
| `MDX001` | Invalid YAML frontmatter                      |
| `MDX101` | Malformed block directive                     |
| `MDX102` | Malformed inline directive                    |
| `MDX103` | Malformed wiki link                           |
| `MDX201` | Unknown handler                               |
| `MDX202` | Runtime execution failure                     |
| `MDX203` | Invalid handler return value                  |
| `MDX301` | Block body parsed as raw text (YAML fallback) |
| `MDX401` | Reparse depth limit exceeded                  |
| `MDX402` | Directive budget exceeded                     |
| `MDX501` | Extension hook failure                        |

## Status

Pre-1.0. See [`crates/mdx_ext/src/lib.rs`](crates/mdx_ext/src/lib.rs) for the public API surface.

## License

MIT OR Apache-2.0
