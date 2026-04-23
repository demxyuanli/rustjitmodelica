use crate::flatten::flat_cache_v1::DepHashEntry;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

#[derive(Default, Debug)]
struct DepCollector {
    files: HashMap<String, String>,
    models: HashSet<String>,
}

thread_local! {
    static DEP_STACK: std::cell::RefCell<Vec<DepCollector>> = std::cell::RefCell::new(Vec::new());
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ReverseDepEntry {
    pub file: String,
    pub content_hash: String,
    pub models: Vec<String>,
}

#[derive(Default)]
struct ReverseDepStore {
    file_to_models: HashMap<String, HashSet<String>>,
    file_hashes: HashMap<String, String>,
}

fn global_reverse_dep_store() -> &'static RwLock<ReverseDepStore> {
    static STORE: OnceLock<RwLock<ReverseDepStore>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(ReverseDepStore::default()))
}

pub(crate) fn dep_record_file(path: &str, semantic_hash: &str) {
    DEP_STACK.with(|s| {
        let mut st = s.borrow_mut();
        for c in st.iter_mut() {
            c.files
                .entry(path.to_string())
                .or_insert_with(|| semantic_hash.to_string());
            if let Ok(mut store) = global_reverse_dep_store().write() {
                let entry = store
                    .file_to_models
                    .entry(path.to_string())
                    .or_insert_with(HashSet::new);
                for model in &c.models {
                    entry.insert(model.clone());
                }
                store
                    .file_hashes
                    .insert(path.to_string(), semantic_hash.to_string());
            }
        }
    });
}

pub(crate) fn dep_record_deps(deps: &[DepHashEntry]) {
    for d in deps {
        dep_record_file(d.path.as_str(), d.content_hash.as_str());
    }
}

pub(crate) struct DepScope {
    active: bool,
}

impl DepScope {
    pub(crate) fn begin() -> Self {
        DEP_STACK.with(|s| s.borrow_mut().push(DepCollector::default()));
        Self { active: true }
    }

    pub(crate) fn begin_for_model(model_name: &str) -> Self {
        DEP_STACK.with(|s| {
            let mut st = s.borrow_mut();
            let mut c = DepCollector::default();
            c.models.insert(model_name.to_string());
            st.push(c);
        });
        Self { active: true }
    }

    pub(crate) fn end(mut self) -> Vec<DepHashEntry> {
        self.active = false;
        let mut v = DEP_STACK.with(|s| s.borrow_mut().pop()).unwrap_or_default();
        let mut out: Vec<DepHashEntry> = v
            .files
            .drain()
            .map(|(path, content_hash)| DepHashEntry { path, content_hash })
            .collect();
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }
}

impl Drop for DepScope {
    fn drop(&mut self) {
        if self.active {
            DEP_STACK.with(|s| {
                let _ = s.borrow_mut().pop();
            });
        }
    }
}

/// Model names associated with the given paths via the reverse dependency store (file path string
/// keys match `Path::display()`). Populated during query/flatten work in **this process**; use for
/// incremental re-validation scope in long-lived hosts (IDE, worker). Not persisted across restarts.
pub fn affected_models(changed_files: &[PathBuf]) -> Vec<String> {
    let mut out: HashSet<String> = HashSet::new();
    if let Ok(store) = global_reverse_dep_store().read() {
        for p in changed_files {
            let key = p.display().to_string();
            if let Some(models) = store.file_to_models.get(&key) {
                out.extend(models.iter().cloned());
            }
        }
    }
    let mut v: Vec<String> = out.into_iter().collect();
    v.sort();
    v
}

/// Breadth-first impact expansion scaffold (currently single-layer file->model mapping).
pub fn impact_radius(seed_files: &[PathBuf], max_depth: usize) -> Vec<String> {
    if max_depth == 0 {
        return affected_models(seed_files);
    }
    let mut out = HashSet::new();
    let mut seen_files = HashSet::new();
    let mut q: VecDeque<(PathBuf, usize)> = seed_files
        .iter()
        .cloned()
        .map(|p| (p, 0usize))
        .collect();
    while let Some((p, depth)) = q.pop_front() {
        if !seen_files.insert(p.clone()) {
            continue;
        }
        for m in affected_models(std::slice::from_ref(&p)) {
            out.insert(m);
        }
        if depth < max_depth {
            // Placeholder for future model->file expansion.
        }
    }
    let mut v: Vec<String> = out.into_iter().collect();
    v.sort();
    v
}

pub fn reverse_dep_snapshot() -> Vec<ReverseDepEntry> {
    if let Ok(store) = global_reverse_dep_store().read() {
        let mut out: Vec<ReverseDepEntry> = store
            .file_to_models
            .iter()
            .map(|(file, models)| {
                let mut models_v: Vec<String> = models.iter().cloned().collect();
                models_v.sort();
                ReverseDepEntry {
                    file: file.clone(),
                    content_hash: store.file_hashes.get(file).cloned().unwrap_or_default(),
                    models: models_v,
                }
            })
            .collect();
        out.sort_by(|a, b| a.file.cmp(&b.file));
        return out;
    }
    Vec::new()
}

