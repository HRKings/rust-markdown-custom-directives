//! `DirectiveRuntime` implementation backed by a sandboxed `mlua::Lua`.
//!
//! Design notes:
//! * The Lua interpreter is not `Sync`, so the whole runtime lives behind a
//!   `Mutex`. `DirectiveRuntime` is declared `Send` only, matching this.
//! * Scripts register handlers via `mdx.register_directive(name, fn)`. Each
//!   registration records which `ScriptId` owns it so that unload can reverse
//!   only that script's registrations.
//! * Handler functions are stored in the Lua registry (`RegistryKey`) rather
//!   than keyed by name — this keeps them anchored even if the script table
//!   goes out of scope.
//! * `generation()` is bumped on every successful load/unload and feeds
//!   `DirectiveCache::CacheKey` in the engine.

use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use mdx_ext::runtime::HandlerDescriptor;
use mdx_ext::{
    DirectiveInvocation, DirectiveOutput, DirectiveRuntime, RuntimeContext, RuntimeError,
    ScriptId, ScriptSource,
};
use mlua::{Function, Lua, LuaSerdeExt, RegistryKey, Table, Value as LuaValue};

use crate::convert;
use crate::sandbox;

struct HandlerEntry {
    key: RegistryKey,
    script: ScriptId,
}

struct ScriptRecord {
    handler_names: Vec<String>,
}

struct Inner {
    lua: Lua,
    handlers: HashMap<String, HandlerEntry>,
    scripts: HashMap<ScriptId, ScriptRecord>,
    next_script_id: u64,
}

pub struct LuaRuntime {
    inner: Mutex<Inner>,
    generation: AtomicU64,
}

impl LuaRuntime {
    pub fn new() -> Result<Self, RuntimeError> {
        let lua = Lua::new();
        sandbox::install(&lua).map_err(lua_err)?;
        install_mdx_table(&lua)?;
        Ok(Self {
            inner: Mutex::new(Inner {
                lua,
                handlers: HashMap::new(),
                scripts: HashMap::new(),
                next_script_id: 1,
            }),
            generation: AtomicU64::new(0),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn lua_err(e: mlua::Error) -> RuntimeError {
    RuntimeError::Load(e.to_string())
}

fn install_mdx_table(lua: &Lua) -> Result<(), RuntimeError> {
    let table = lua.create_table().map_err(lua_err)?;
    let register = lua
        .create_function(
            |lua, (name, func): (String, Function)| -> mlua::Result<()> {
                // Stash the function in the registry and append to the pending list
                // held on the Lua side in a hidden table.
                let key = lua.create_registry_value(func)?;
                let staging: Table = lua.named_registry_value("__mdx_pending").or_else(|_| {
                    let t = lua.create_table()?;
                    lua.set_named_registry_value("__mdx_pending", t.clone())?;
                    Ok::<_, mlua::Error>(t)
                })?;
                let next = staging.raw_len() + 1;
                // We can't store a RegistryKey in Lua, so we serialize a small marker
                // and re-resolve on the Rust side via a parallel Rust Vec. Use a
                // separate table keyed by insertion order containing the name only,
                // then drain the RegistryKey via a thread-local channel.
                staging.set(next, name.clone())?;
                // Publish the RegistryKey through a second registry slot, keyed by
                // sequence number.
                let keys_table_name = "__mdx_pending_keys";
                let keys_table: Table = lua.named_registry_value(keys_table_name).or_else(|_| {
                    let t = lua.create_table()?;
                    lua.set_named_registry_value(keys_table_name, t.clone())?;
                    Ok::<_, mlua::Error>(t)
                })?;
                // Store the raw registry key index as a userdata-free integer by
                // round-tripping through a boxed integer. `create_registry_value`
                // gives us a RegistryKey that owns the slot; we need to hand it to
                // Rust after load, so we cache its numeric index via `into_i32`-less
                // means: store the key inside a Rust-only thread_local.
                PENDING_KEYS.with(|cell| {
                    let mut v = cell.borrow_mut();
                    v.push((name, key));
                });
                let _ = keys_table; // silence unused warning in some configurations
                Ok(())
            },
        )
        .map_err(lua_err)?;
    table.set("register_directive", register).map_err(lua_err)?;
    lua.globals().set("mdx", table).map_err(lua_err)?;
    Ok(())
}

thread_local! {
    static PENDING_KEYS: std::cell::RefCell<Vec<(String, RegistryKey)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

impl DirectiveRuntime for LuaRuntime {
    fn load_script(&mut self, source: ScriptSource) -> Result<ScriptId, RuntimeError> {
        let (name, chunk): (String, String) = match source {
            ScriptSource::File(path) => {
                let content = fs::read_to_string(&path).map_err(|e| {
                    RuntimeError::Load(format!("read {}: {e}", path.display()))
                })?;
                (path.display().to_string(), content)
            }
            ScriptSource::Text(t) => ("<inline>".to_string(), t),
            ScriptSource::NamedText { name, content } => (name, content),
        };
        let mut inner = self.lock();
        let id = ScriptId(inner.next_script_id);
        inner.next_script_id += 1;
        PENDING_KEYS.with(|c| c.borrow_mut().clear());
        let exec_result = inner.lua.load(&chunk).set_name(&name).exec();
        exec_result.map_err(|e| RuntimeError::Load(e.to_string()))?;
        // Drain the pending registrations.
        let pending: Vec<(String, RegistryKey)> =
            PENDING_KEYS.with(|c| std::mem::take(&mut *c.borrow_mut()));
        let mut handler_names = Vec::new();
        for (hname, key) in pending {
            // If a handler of the same name is already registered, replace it.
            if let Some(prev) = inner.handlers.remove(&hname) {
                let _ = inner.lua.remove_registry_value(prev.key);
            }
            inner
                .handlers
                .insert(hname.clone(), HandlerEntry { key, script: id });
            handler_names.push(hname);
        }
        let _ = name;
        inner.scripts.insert(id, ScriptRecord { handler_names });
        self.generation.fetch_add(1, Ordering::Relaxed);
        Ok(id)
    }

    fn unload_script(&mut self, id: ScriptId) -> Result<(), RuntimeError> {
        let mut inner = self.lock();
        let rec = inner
            .scripts
            .remove(&id)
            .ok_or_else(|| RuntimeError::Load(format!("unknown script id {id:?}")))?;
        for hname in rec.handler_names {
            if let Some(entry) = inner.handlers.remove(&hname) {
                if entry.script == id {
                    let _ = inner.lua.remove_registry_value(entry.key);
                }
            }
        }
        self.generation.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn list_handlers(&self) -> Vec<HandlerDescriptor> {
        let inner = self.lock();
        inner
            .handlers
            .iter()
            .map(|(name, entry)| HandlerDescriptor {
                name: name.clone(),
                script: entry.script,
                kind: None,
            })
            .collect()
    }

    fn execute(
        &self,
        handler: &str,
        invocation: DirectiveInvocation,
        ctx: &RuntimeContext,
    ) -> Result<DirectiveOutput, RuntimeError> {
        let inner = self.lock();
        let entry = inner
            .handlers
            .get(handler)
            .ok_or_else(|| RuntimeError::UnknownHandler(handler.to_string()))?;
        let func: Function = inner
            .lua
            .registry_value(&entry.key)
            .map_err(|e| {
                RuntimeError::Execution(format!(
                    "handler '{handler}': failed to retrieve function: {e}"
                ))
            })?;
        let inv_table = convert::invocation_to_lua(&inner.lua, &invocation)?;
        let ctx_table = build_context_table(&inner.lua, ctx)?;
        let result: LuaValue = func
            .call((inv_table, ctx_table))
            .map_err(|e| RuntimeError::Execution(format!("handler '{handler}': {e}")))?;
        convert::lua_to_output(&inner.lua, result)
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
}

fn build_context_table(lua: &Lua, ctx: &RuntimeContext) -> Result<Table, RuntimeError> {
    let t = lua.create_table().map_err(|e| RuntimeError::Other(e.to_string()))?;
    if let Some(meta) = &ctx.document_metadata {
        let v = lua
            .to_value(meta)
            .map_err(|e| RuntimeError::Other(e.to_string()))?;
        t.set("document_metadata", v)
            .map_err(|e| RuntimeError::Other(e.to_string()))?;
    }
    let vars = lua
        .create_table()
        .map_err(|e| RuntimeError::Other(e.to_string()))?;
    for (k, v) in &ctx.variables {
        let lv = lua
            .to_value(v)
            .map_err(|e| RuntimeError::Other(e.to_string()))?;
        vars.set(k.as_str(), lv)
            .map_err(|e| RuntimeError::Other(e.to_string()))?;
    }
    t.set("variables", vars)
        .map_err(|e| RuntimeError::Other(e.to_string()))?;
    Ok(t)
}
