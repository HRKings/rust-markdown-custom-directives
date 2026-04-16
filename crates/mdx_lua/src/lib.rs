//! Lua runtime for `mdx_ext` directive and semantic-link handlers.
//!
//! Loads scripts dynamically at runtime; handlers register themselves via
//! `register_directive(name, fn)` and `register_link_resolver(namespace, fn)`
//! exposed on the global `mdx` table.
#![forbid(unsafe_code)]

pub mod convert;
pub mod runtime;
pub mod sandbox;

pub use runtime::LuaRuntime;
