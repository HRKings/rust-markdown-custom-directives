//! `DirectiveRuntime` implementation backed by a sandboxed `mlua::Lua`.
//!
//! Design notes:
//! * The Lua interpreter is not `Sync`, so the whole runtime lives behind a
//!   `Mutex`. `DirectiveRuntime` is declared `Send` only, matching this.
//! * Scripts register handlers via `mdx.register_directive(name, fn)` and
//!   namespaced link resolvers via `mdx.register_link_resolver(namespace, fn)`.
//!   Each registration records which `ScriptId` owns it so that unload can
//!   reverse only that script's registrations.
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
    DirectiveInvocation, DirectiveOutput, DirectiveRuntime, LinkInvocation, RuntimeContext,
    RuntimeError, ScriptId, ScriptSource,
};
use mlua::{Function, Lua, LuaSerdeExt, RegistryKey, Table, Value as LuaValue};

use crate::convert;
use crate::sandbox;

struct HandlerEntry {
    key: RegistryKey,
    script: ScriptId,
}

struct ScriptRecord {
    directive_handler_names: Vec<String>,
    link_handler_names: Vec<String>,
}

struct Inner {
    lua: Lua,
    directive_handlers: HashMap<String, HandlerEntry>,
    link_handlers: HashMap<String, HandlerEntry>,
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
                directive_handlers: HashMap::new(),
                link_handlers: HashMap::new(),
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
    let register_directive = lua
        .create_function(
            |lua, (name, func): (String, Function)| -> mlua::Result<()> {
                let key = lua.create_registry_value(func)?;
                PENDING_REGISTRATIONS.with(|cell| {
                    cell.borrow_mut()
                        .push(PendingRegistration::Directive { name, key });
                });
                Ok(())
            },
        )
        .map_err(lua_err)?;
    let register_link_resolver = lua
        .create_function(
            |lua, (namespace, func): (String, Function)| -> mlua::Result<()> {
                let key = lua.create_registry_value(func)?;
                PENDING_REGISTRATIONS.with(|cell| {
                    cell.borrow_mut()
                        .push(PendingRegistration::Link { namespace, key });
                });
                Ok(())
            },
        )
        .map_err(lua_err)?;
    table
        .set("register_directive", register_directive)
        .map_err(lua_err)?;
    table
        .set("register_link_resolver", register_link_resolver)
        .map_err(lua_err)?;
    lua.globals().set("mdx", table).map_err(lua_err)?;
    Ok(())
}

enum PendingRegistration {
    Directive { name: String, key: RegistryKey },
    Link { namespace: String, key: RegistryKey },
}

thread_local! {
    static PENDING_REGISTRATIONS: std::cell::RefCell<Vec<PendingRegistration>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

impl DirectiveRuntime for LuaRuntime {
    fn load_script(&mut self, source: ScriptSource) -> Result<ScriptId, RuntimeError> {
        let (name, chunk): (String, String) = match source {
            ScriptSource::File(path) => {
                let content = fs::read_to_string(&path)
                    .map_err(|e| RuntimeError::Load(format!("read {}: {e}", path.display())))?;
                (path.display().to_string(), content)
            }
            ScriptSource::Text(t) => ("<inline>".to_string(), t),
            ScriptSource::NamedText { name, content } => (name, content),
        };
        let mut inner = self.lock();
        let id = ScriptId(inner.next_script_id);
        inner.next_script_id += 1;
        PENDING_REGISTRATIONS.with(|c| c.borrow_mut().clear());
        let exec_result = inner.lua.load(&chunk).set_name(&name).exec();
        exec_result.map_err(|e| RuntimeError::Load(e.to_string()))?;
        // Drain the pending registrations.
        let pending: Vec<PendingRegistration> =
            PENDING_REGISTRATIONS.with(|c| std::mem::take(&mut *c.borrow_mut()));
        let mut directive_handler_names = Vec::new();
        let mut link_handler_names = Vec::new();
        for reg in pending {
            match reg {
                PendingRegistration::Directive { name, key } => {
                    if let Some(prev) = inner.directive_handlers.remove(&name) {
                        let _ = inner.lua.remove_registry_value(prev.key);
                    }
                    inner
                        .directive_handlers
                        .insert(name.clone(), HandlerEntry { key, script: id });
                    directive_handler_names.push(name);
                }
                PendingRegistration::Link { namespace, key } => {
                    if let Some(prev) = inner.link_handlers.remove(&namespace) {
                        let _ = inner.lua.remove_registry_value(prev.key);
                    }
                    inner
                        .link_handlers
                        .insert(namespace.clone(), HandlerEntry { key, script: id });
                    link_handler_names.push(namespace);
                }
            }
        }
        let _ = name;
        inner.scripts.insert(
            id,
            ScriptRecord {
                directive_handler_names,
                link_handler_names,
            },
        );
        self.generation.fetch_add(1, Ordering::Relaxed);
        Ok(id)
    }

    fn unload_script(&mut self, id: ScriptId) -> Result<(), RuntimeError> {
        let mut inner = self.lock();
        let rec = inner
            .scripts
            .remove(&id)
            .ok_or_else(|| RuntimeError::Load(format!("unknown script id {id:?}")))?;
        for hname in rec.directive_handler_names {
            if let Some(entry) = inner.directive_handlers.remove(&hname) {
                if entry.script == id {
                    let _ = inner.lua.remove_registry_value(entry.key);
                }
            }
        }
        for namespace in rec.link_handler_names {
            if let Some(entry) = inner.link_handlers.remove(&namespace) {
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
            .directive_handlers
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
            .directive_handlers
            .get(handler)
            .ok_or_else(|| RuntimeError::UnknownHandler(handler.to_string()))?;
        let func: Function = inner.lua.registry_value(&entry.key).map_err(|e| {
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

    fn execute_link(
        &self,
        namespace: &str,
        invocation: LinkInvocation,
        ctx: &RuntimeContext,
    ) -> Result<DirectiveOutput, RuntimeError> {
        let inner = self.lock();
        let entry = inner
            .link_handlers
            .get(namespace)
            .ok_or_else(|| RuntimeError::UnknownLinkResolver(namespace.to_string()))?;
        let func: Function = inner.lua.registry_value(&entry.key).map_err(|e| {
            RuntimeError::Execution(format!(
                "link resolver '{namespace}': failed to retrieve function: {e}"
            ))
        })?;
        let link_table = convert::link_invocation_to_lua(&inner.lua, &invocation)?;
        let ctx_table = build_context_table(&inner.lua, ctx)?;
        let result: LuaValue = func
            .call((link_table, ctx_table))
            .map_err(|e| RuntimeError::Execution(format!("link resolver '{namespace}': {e}")))?;
        convert::lua_to_output(&inner.lua, result)
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
}

fn build_context_table(lua: &Lua, ctx: &RuntimeContext) -> Result<Table, RuntimeError> {
    let t = lua
        .create_table()
        .map_err(|e| RuntimeError::Other(e.to_string()))?;
    if let Some(meta) = &ctx.document_metadata {
        let v = lua
            .to_value(meta)
            .map_err(|e| RuntimeError::Other(e.to_string()))?;
        t.set("document_metadata", v)
            .map_err(|e| RuntimeError::Other(e.to_string()))?;
        let v = lua
            .to_value(meta)
            .map_err(|e| RuntimeError::Other(e.to_string()))?;
        t.set("frontmatter", v)
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
