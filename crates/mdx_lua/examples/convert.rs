use std::path::PathBuf;
use std::process::ExitCode;

use mdx_ext::{MarkdownEngine, ResolutionMode, RuntimeContext, ScriptSource};
use mdx_lua::LuaRuntime;

fn main() -> ExitCode {
    let base = example_dir();
    let script_path = base.join("convert.lua");
    let input_path = base.join("input.md");

    let source = match std::fs::read_to_string(&input_path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("read {}: {err}", input_path.display());
            return ExitCode::FAILURE;
        }
    };

    let mut engine = match MarkdownEngine::builder()
        .with_runtime(Box::new(LuaRuntime::new().expect("lua runtime")))
        .with_resolution_mode(ResolutionMode::Strict)
        .build()
    {
        Ok(engine) => engine,
        Err(err) => {
            eprintln!("build engine: {err}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = engine.load_script(ScriptSource::File(script_path.clone())) {
        eprintln!("load {}: {err}", script_path.display());
        return ExitCode::FAILURE;
    }

    let doc = engine.parse(&source);
    let ctx = RuntimeContext {
        document_metadata: doc.frontmatter.as_ref().map(|fm| fm.value.clone()),
        ..RuntimeContext::default()
    };

    let resolved = match engine.resolve(doc, &ctx) {
        Ok(doc) => doc,
        Err(err) => {
            eprintln!("resolve document: {err}");
            return ExitCode::FAILURE;
        }
    };

    println!("{}", engine.render_text(&resolved));
    ExitCode::SUCCESS
}

fn example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("convert")
}
