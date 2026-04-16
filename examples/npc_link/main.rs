use std::path::PathBuf;
use std::process::ExitCode;

use mdx_ext::{Document, LinkKind, MarkdownEngine, Node, ResolutionMode, RuntimeContext, ScriptSource};
use mdx_lua::LuaRuntime;

fn main() -> ExitCode {
    let base = example_dir();
    let input_path = base.join("input.md");
    let script_path = base.join("npc.lua");

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
    if !has_npc_namespaced_link(&doc) {
        eprintln!("expected parsed document to contain [[npc:...]] as a namespaced link");
        return ExitCode::FAILURE;
    }

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

    println!("{}", engine.render_html(&resolved));
    ExitCode::SUCCESS
}

fn example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("npc_link")
}

fn has_npc_namespaced_link(doc: &Document) -> bool {
    doc.children.iter().any(node_has_npc_namespaced_link)
}

fn node_has_npc_namespaced_link(node: &Node) -> bool {
    match node {
        Node::Paragraph { children, .. }
        | Node::Heading { children, .. }
        | Node::Emphasis { children, .. }
        | Node::Strong { children, .. }
        | Node::BlockQuote { children, .. }
        | Node::ListItem { children, .. }
        | Node::Component { children, .. } => children.iter().any(node_has_npc_namespaced_link),
        Node::List { items, .. } => items.iter().any(node_has_npc_namespaced_link),
        Node::Directive(d) => d.children.iter().any(node_has_npc_namespaced_link),
        Node::Link(link) => match &link.kind {
            LinkKind::NamespacedLink { namespace, target } => {
                namespace == "npc" && target == "captain-lyra"
            }
            _ => false,
        },
        _ => false,
    }
}
