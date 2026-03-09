// File system watcher: monitors project directory for changes and triggers
// incremental index updates via debounced events.

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use crate::index_manager::CodeIndex;

const DEBOUNCE_MS: u64 = 500;

struct WatcherState {
    _watcher: RecommendedWatcher,
}

static WATCHER: Mutex<Option<WatcherState>> = Mutex::new(None);

pub fn start_watching(app_handle: AppHandle, project_dir: String) -> Result<(), String> {
    stop_watching()?;

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher =
        RecommendedWatcher::new(tx, notify::Config::default()).map_err(|e| e.to_string())?;

    let dir = Path::new(&project_dir);
    if !dir.is_dir() {
        return Err("Project directory does not exist".to_string());
    }

    watcher
        .watch(dir, RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;

    let project_dir_clone = project_dir.clone();
    std::thread::spawn(move || {
        debounce_loop(rx, app_handle, &project_dir_clone);
    });

    let mut guard = WATCHER.lock().map_err(|e| e.to_string())?;
    *guard = Some(WatcherState { _watcher: watcher });

    Ok(())
}

pub fn stop_watching() -> Result<(), String> {
    let mut guard = WATCHER.lock().map_err(|e| e.to_string())?;
    *guard = None;
    Ok(())
}

fn debounce_loop(
    rx: mpsc::Receiver<notify::Result<Event>>,
    app_handle: AppHandle,
    project_dir: &str,
) {
    let mut pending: Vec<PathBuf> = Vec::new();
    let mut last_event = Instant::now();
    let debounce = Duration::from_millis(DEBOUNCE_MS);

    loop {
        match rx.recv_timeout(debounce) {
            Ok(Ok(event)) => {
                if is_relevant_event(&event) {
                    for path in &event.paths {
                        if is_indexable(path) && !pending.contains(path) {
                            pending.push(path.clone());
                        }
                    }
                    last_event = Instant::now();
                }
            }
            Ok(Err(_)) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !pending.is_empty() && last_event.elapsed() >= debounce {
                    process_pending(&app_handle, project_dir, &pending);
                    pending.clear();
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn process_pending(app_handle: &AppHandle, project_dir: &str, paths: &[PathBuf]) {
    let index = CodeIndex::new(project_dir);
    let base = Path::new(project_dir);

    let mut updated_files: Vec<String> = Vec::new();
    for path in paths {
        if let Ok(rel) = path.strip_prefix(base) {
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if path.exists() {
                let _ = index.update_file(&rel_str);
            } else {
                let _ = index.remove_file(&rel_str);
            }
            updated_files.push(rel_str);
        }
    }

    if !updated_files.is_empty() {
        let _ = app_handle.emit("index-updated", &updated_files);
    }
}

fn is_relevant_event(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}

fn is_indexable(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    if name.starts_with('.') {
        return false;
    }

    let skip_dirs = ["node_modules", "target", "build", ".git", ".modai-ide-data"];
    for component in path.components() {
        let comp = component.as_os_str().to_string_lossy();
        if skip_dirs.contains(&comp.as_ref()) {
            return false;
        }
    }

    let exts = [
        "mo", "rs", "ts", "tsx", "js", "jsx", "py", "c", "h", "cpp", "hpp", "toml", "json",
        "css", "html",
    ];
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| exts.contains(&e))
        .unwrap_or(false)
}
