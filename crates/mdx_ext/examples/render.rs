//! Minimal CLI: read a markdown file, render HTML to stdout.
//!
//! Runs with `Passthrough` resolution (no runtime), so directives appear in
//! output as fallback HTML comments. For a Lua-powered example, pair this
//! with the `mdx_lua` crate.

use std::env;
use std::fs;
use std::process::ExitCode;

use mdx_ext::{MarkdownEngine, ResolutionMode};

fn main() -> ExitCode {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: render <file.md>");
        return ExitCode::from(2);
    };
    let source = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let engine = MarkdownEngine::builder()
        .with_resolution_mode(ResolutionMode::Passthrough)
        .build()
        .expect("engine");
    let doc = engine.parse(&source);
    print!("{}", engine.render_html(&doc));
    ExitCode::SUCCESS
}
