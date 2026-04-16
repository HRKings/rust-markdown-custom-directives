//! Conversions between `serde_json::Value`, `mlua::Value`, and
//! `mdx_ext::DirectiveOutput`.

use mdx_ext::{AttributeMap, DirectiveInvocation, DirectiveOutput, RuntimeError};
use mlua::{Lua, LuaSerdeExt, Table, Value as LuaValue};
use serde_json::Value as JsonValue;

/// Convert a JSON value to a Lua value via mlua's serde bridge.
pub fn json_to_lua(lua: &Lua, value: &JsonValue) -> Result<LuaValue, RuntimeError> {
    lua.to_value(value).map_err(lua_err)
}

/// Convert a Lua value to a JSON value via mlua's serde bridge.
pub fn lua_to_json(lua: &Lua, value: LuaValue) -> Result<JsonValue, RuntimeError> {
    lua.from_value(value).map_err(lua_err)
}

/// Build the Lua table passed to a directive handler.
pub fn invocation_to_lua(lua: &Lua, inv: &DirectiveInvocation) -> Result<Table, RuntimeError> {
    let t = lua.create_table().map_err(lua_err)?;
    t.set("name", inv.name.as_str()).map_err(lua_err)?;
    t.set(
        "kind",
        match inv.kind {
            mdx_ext::ast::DirectiveKind::Block => "block",
            mdx_ext::ast::DirectiveKind::Inline => "inline",
        },
    )
    .map_err(lua_err)?;
    // Attributes — converted through serde for fidelity.
    let attrs_json = JsonValue::Object(
        inv.attributes
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    );
    t.set("attributes", json_to_lua(lua, &attrs_json)?)
        .map_err(lua_err)?;
    // Body.
    let body_val: JsonValue = match &inv.body {
        mdx_ext::DirectiveBody::None => JsonValue::Null,
        mdx_ext::DirectiveBody::Raw(s) => JsonValue::String(s.clone()),
        mdx_ext::DirectiveBody::Attributes(a) => {
            JsonValue::Object(a.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        }
    };
    t.set("body", json_to_lua(lua, &body_val)?)
        .map_err(lua_err)?;
    t.set("children_text", inv.children_text.as_str())
        .map_err(lua_err)?;
    // Span.
    let span = lua.create_table().map_err(lua_err)?;
    span.set("start", inv.span.start).map_err(lua_err)?;
    span.set("end_", inv.span.end).map_err(lua_err)?;
    t.set("span", span).map_err(lua_err)?;
    Ok(t)
}

/// Map the Lua return value of a handler into a `DirectiveOutput`.
///
/// Conventions:
/// * A plain `string` → `DirectiveOutput::Text`.
/// * A `table` with a `type` field selects the variant:
///     * `text` / `html` / `markdown` → `{ type, value }`
///     * `component` → `{ type, name, props? }`
///     * `data` → `{ type, value }`
///     * `error` → `{ type, message }`
/// * A `table` without a `type` field but with a `text`/`html`/`markdown`/`component`
///   key is accepted as a convenience.
/// * `nil` → empty `Text("")`.
pub fn lua_to_output(lua: &Lua, value: LuaValue) -> Result<DirectiveOutput, RuntimeError> {
    match value {
        LuaValue::Nil => Ok(DirectiveOutput::Text(String::new())),
        LuaValue::String(s) => Ok(DirectiveOutput::Text(
            s.to_str().map_err(lua_err)?.to_string(),
        )),
        LuaValue::Table(t) => table_to_output(lua, t),
        other => Err(RuntimeError::InvalidReturn(format!(
            "unsupported handler return type: {}",
            other.type_name()
        ))),
    }
}

fn table_to_output(lua: &Lua, t: Table) -> Result<DirectiveOutput, RuntimeError> {
    // Prefer explicit `type` discriminator.
    let ty: Option<String> = t.get("type").ok();
    let ty = ty.map(|s| s.to_ascii_lowercase());
    let select = ty.as_deref();
    match select {
        Some("text") => Ok(DirectiveOutput::Text(string_field(&t, "value")?)),
        Some("html") => Ok(DirectiveOutput::Html(string_field(&t, "value")?)),
        Some("markdown") => Ok(DirectiveOutput::Markdown(string_field(&t, "value")?)),
        Some("data") => {
            let raw: LuaValue = t.get("value").map_err(lua_err)?;
            Ok(DirectiveOutput::Data(lua_to_json(lua, raw)?))
        }
        Some("component") => build_component(lua, &t),
        Some("error") => Ok(DirectiveOutput::Error {
            message: string_field(&t, "message").unwrap_or_default(),
        }),
        Some(other) => Err(RuntimeError::InvalidReturn(format!(
            "unknown directive output type: {other}"
        ))),
        None => {
            // Convenience forms: single-key tables.
            if let Ok(s) = string_field(&t, "text") {
                return Ok(DirectiveOutput::Text(s));
            }
            if let Ok(s) = string_field(&t, "html") {
                return Ok(DirectiveOutput::Html(s));
            }
            if let Ok(s) = string_field(&t, "markdown") {
                return Ok(DirectiveOutput::Markdown(s));
            }
            if let Ok(name) = string_field(&t, "component") {
                let props_val: LuaValue = t.get("props").unwrap_or(LuaValue::Nil);
                let props = value_to_attrs(lua, props_val)?;
                return Ok(DirectiveOutput::Component { name, props });
            }
            Err(RuntimeError::InvalidReturn(
                "table has no recognised directive-output discriminator".into(),
            ))
        }
    }
}

fn build_component(lua: &Lua, t: &Table) -> Result<DirectiveOutput, RuntimeError> {
    let name = string_field(t, "name")?;
    let props_val: LuaValue = t.get("props").unwrap_or(LuaValue::Nil);
    let props = value_to_attrs(lua, props_val)?;
    Ok(DirectiveOutput::Component { name, props })
}

fn string_field(t: &Table, key: &str) -> Result<String, RuntimeError> {
    let v: LuaValue = t.get(key).map_err(lua_err)?;
    match v {
        LuaValue::String(s) => Ok(s.to_str().map_err(lua_err)?.to_string()),
        LuaValue::Nil => Err(RuntimeError::InvalidReturn(format!("missing field: {key}"))),
        other => Err(RuntimeError::InvalidReturn(format!(
            "field {key} must be a string, got {}",
            other.type_name()
        ))),
    }
}

fn value_to_attrs(lua: &Lua, value: LuaValue) -> Result<AttributeMap, RuntimeError> {
    if matches!(value, LuaValue::Nil) {
        return Ok(AttributeMap::new());
    }
    let json = lua_to_json(lua, value)?;
    match json {
        JsonValue::Object(m) => {
            let mut attrs = AttributeMap::new();
            for (k, v) in m {
                attrs.insert(k, v);
            }
            Ok(attrs)
        }
        _ => Err(RuntimeError::InvalidReturn(
            "props must be a table/object".into(),
        )),
    }
}

fn lua_err(e: mlua::Error) -> RuntimeError {
    RuntimeError::Execution(e.to_string())
}
