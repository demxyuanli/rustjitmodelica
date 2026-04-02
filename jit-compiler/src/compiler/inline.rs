use crate::flatten::FlattenedModel;
use crate::loader::ModelLoader;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

mod builtin;
mod function_body;
mod record_access;
mod rewrite;
mod subst;
mod traverse;

pub(crate) use builtin::is_builtin_function;
pub(crate) use function_body::get_function_body;
use rewrite::ResolveMemoEntry;

const MAX_INLINE_RECURSION_DEPTH: u32 = 64;

fn global_resolve_memo() -> &'static RwLock<HashMap<String, ResolveMemoEntry>> {
    static MEMO: OnceLock<RwLock<HashMap<String, ResolveMemoEntry>>> = OnceLock::new();
    MEMO.get_or_init(|| RwLock::new(HashMap::new()))
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
