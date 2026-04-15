//! Renderers over the owned `Document` tree.
//!
//! Three renderers ship in v1:
//! * [`html::render`] — HTML output (escapes text, emits standard tags,
//!   renders components as `<mdx-component data-name="…">`).
//! * [`text::render`] — plain text projection for previews / search indices.
//! * [`debug::render`] — indented s-expression-style dump with spans.

pub mod debug;
pub mod html;
pub mod text;
