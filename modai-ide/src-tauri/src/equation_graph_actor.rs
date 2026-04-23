//! Process-level singleton that owns warm `rustmodlica::Compiler` instances
//! keyed by `(project_dir, library_paths_fingerprint)` so subsequent
//! `get_equation_graph_*` calls reuse the parsed Modelica Standard Library
//! instead of re-parsing thousands of `.mo` files each invocation.
//!
//! `Compiler` is `!Send` (carries raw `*const u8` pointers in
//! `external_symbol_ptrs`), so the cache lives behind a single dedicated
//! OS thread and is reached via an `mpsc` channel. Tauri commands run on
//! the blocking pool already, so a synchronous round-trip here costs only
//! the actual compile time once the loader is warm.
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::OnceLock;

use rustmodlica::equation_graph::EquationGraphMode;
use rustmodlica::{Compiler, EquationGraph, NodeKey};

/// Maximum number of warm compilers retained in the cache. Each compiler
/// holds the parsed MSL plus any project-local libraries; capping at a
/// small number keeps memory predictable for sessions that hop between
/// projects without sacrificing the common single-project workflow.
const MAX_CACHED_COMPILERS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CompilerCacheKey {
    project_dir: Option<String>,
    paths_fingerprint: String,
}

fn fingerprint_paths(paths: &[PathBuf]) -> String {
    let mut normalized: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized.join("|")
}

pub struct EquationGraphBuildRequest {
    pub code: String,
    pub model_name: String,
    pub project_dir: Option<String>,
    pub loader_paths: Vec<PathBuf>,
    pub graph_mode: EquationGraphMode,
    pub changed_keys: Option<Vec<NodeKey>>,
}

enum ActorMessage {
    Build {
        request: EquationGraphBuildRequest,
        respond: mpsc::SyncSender<Result<EquationGraph, String>>,
    },
}

fn actor_loop(rx: mpsc::Receiver<ActorMessage>) {
    let mut cache: Vec<(CompilerCacheKey, Compiler)> = Vec::with_capacity(MAX_CACHED_COMPILERS);
    while let Ok(msg) = rx.recv() {
        match msg {
            ActorMessage::Build { request, respond } => {
                let result = handle_build(&mut cache, request);
                let _ = respond.send(result);
            }
        }
    }
}

fn handle_build(
    cache: &mut Vec<(CompilerCacheKey, Compiler)>,
    request: EquationGraphBuildRequest,
) -> Result<EquationGraph, String> {
    let EquationGraphBuildRequest {
        code,
        model_name,
        project_dir,
        loader_paths,
        graph_mode,
        changed_keys,
    } = request;

    let key = CompilerCacheKey {
        project_dir,
        paths_fingerprint: fingerprint_paths(&loader_paths),
    };

    let pos = cache.iter().position(|(k, _)| k == &key);
    let mut entry = match pos {
        Some(idx) => cache.swap_remove(idx),
        None => {
            let mut compiler = Compiler::new();
            for path in &loader_paths {
                compiler.loader.add_path(path.clone());
            }
            (key, compiler)
        }
    };

    entry.1.loader.forget_model(&model_name);

    let result = entry
        .1
        .get_equation_graph_from_source_with_dirty(
            &model_name,
            &code,
            graph_mode,
            changed_keys.as_deref(),
        )
        .map_err(|e| e.to_string());

    cache.push(entry);
    while cache.len() > MAX_CACHED_COMPILERS {
        cache.remove(0);
    }

    result
}

fn sender() -> &'static Sender<ActorMessage> {
    static SENDER: OnceLock<Sender<ActorMessage>> = OnceLock::new();
    SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<ActorMessage>();
        std::thread::Builder::new()
            .name("modai-equation-graph-actor".into())
            .spawn(move || actor_loop(rx))
            .expect("spawn equation-graph actor thread");
        tx
    })
}

/// Build an equation graph using the warm-loader actor. Blocks the calling
/// thread until the actor responds; expected to be invoked from inside
/// `tokio::task::spawn_blocking` so the Tokio runtime is not stalled.
pub fn build_equation_graph_blocking(
    request: EquationGraphBuildRequest,
) -> Result<EquationGraph, String> {
    let (resp_tx, resp_rx) = mpsc::sync_channel::<Result<EquationGraph, String>>(1);
    sender()
        .send(ActorMessage::Build {
            request,
            respond: resp_tx,
        })
        .map_err(|e| format!("equation-graph actor send: {e}"))?;
    resp_rx
        .recv()
        .map_err(|e| format!("equation-graph actor recv: {e}"))?
}
