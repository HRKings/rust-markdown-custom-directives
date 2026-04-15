//! Sandbox helpers: strip dangerous standard-library entry points from the
//! Lua globals table so that loaded scripts cannot touch the filesystem,
//! network, or host process.

use mlua::{Lua, Result as LuaResult};

/// Names stripped from the global environment unconditionally.
const STRIPPED_GLOBALS: &[&str] = &[
    "io", "os", "package", "require", "dofile", "loadfile", "load", "loadstring", "debug",
    "collectgarbage",
];

/// Apply the sandbox: remove unsafe globals.
pub fn install(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();
    for name in STRIPPED_GLOBALS {
        globals.set(*name, mlua::Value::Nil)?;
    }
    Ok(())
}
