use crate::flatten::FlattenedModel;
use crate::ast::Model;
use crate::loader::ModelLoader;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

mod builtin;
pub(crate) mod enum_check;
mod function_body;
mod record_access;
mod rewrite;
mod subst;
mod traverse;

pub(crate) use builtin::is_builtin_function;
pub(crate) use function_body::get_function_body;
use rewrite::ResolveMemoEntry;

const MAX_INLINE_RECURSION_DEPTH: u32 = 64;
const MAX_GLOBAL_INLINE_MODEL_CACHE: usize = 8192;

fn global_resolve_memo() -> &'static RwLock<HashMap<String, ResolveMemoEntry>> {
    static MEMO: OnceLock<RwLock<HashMap<String, ResolveMemoEntry>>> = OnceLock::new();
    MEMO.get_or_init(|| RwLock::new(HashMap::new()))
}

fn global_inline_model_cache() -> &'static RwLock<HashMap<String, Arc<Model>>> {
    static CACHE: OnceLock<RwLock<HashMap<String, Arc<Model>>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(super) fn seed_inline_model_cache(cache: &mut HashMap<String, Arc<Model>>, loader: &ModelLoader) {
    if let Ok(g) = global_inline_model_cache().read() {
        for (k, v) in g.iter() {
            cache.entry(k.clone()).or_insert_with(|| Arc::clone(v));
        }
    }
    for (k, v) in loader.snapshot_warm_models() {
        cache.entry(k).or_insert(v);
    }
}

pub(super) fn merge_inline_model_cache(cache: HashMap<String, Arc<Model>>) {
    if cache.is_empty() {
        return;
    }
    if let Ok(mut g) = global_inline_model_cache().write() {
        for (k, v) in cache {
            g.entry(k).or_insert(v);
        }
        while g.len() > MAX_GLOBAL_INLINE_MODEL_CACHE {
            if let Some(k) = g.keys().next().cloned() {
                g.remove(&k);
            } else {
                break;
            }
        }
    }
}

pub fn inline_function_calls(
    flat: &mut FlattenedModel,
    loader: &mut ModelLoader,
    stage_trace: bool,
) {
    let max_depth = std::env::var("RUSTMODLICA_INLINE_MAX_DEPTH")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(MAX_INLINE_RECURSION_DEPTH);
    let mut resolve_memo = global_resolve_memo()
        .read()
        .map(|g| g.clone())
        .unwrap_or_default();
    traverse::inline_function_calls_in_model(flat, loader, &mut resolve_memo, max_depth, stage_trace);
    if let Ok(mut g) = global_resolve_memo().write() {
        const MAX_MEMO_ENTRIES: usize = 4096;
        if resolve_memo.len() > MAX_MEMO_ENTRIES {
            resolve_memo.retain(|_, v| matches!(v, ResolveMemoEntry::Resolved(_)));
            while resolve_memo.len() > MAX_MEMO_ENTRIES {
                if let Some(k) = resolve_memo.keys().next().cloned() {
                    resolve_memo.remove(&k);
                } else {
                    break;
                }
            }
        }
        *g = resolve_memo;
    }
}
