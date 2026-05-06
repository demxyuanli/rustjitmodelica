use crate::ast::Expression;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{OnceLock, RwLock};

use super::super::expressions::eval_const_expr;

#[derive(Debug)]
pub struct SubstituteCache {
    pub(super) cache: HashMap<*const Expression, Expression>,
    pub(super) max_size: usize,
    pub(super) order: Vec<*const Expression>,
}

impl SubstituteCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(max_size.min(4096)),
            max_size,
            order: Vec::with_capacity(max_size.min(4096)),
        }
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.order.clear();
    }
}

pub(super) fn expand_range_indices(
    start: &Expression,
    step: &Expression,
    end: &Expression,
) -> Option<Vec<i64>> {
    let start_val = eval_const_expr(start)? as i64;
    let step_val = eval_const_expr(step)? as i64;
    let end_val = eval_const_expr(end)? as i64;
    if step_val == 0 {
        return None;
    }
    let mut values = Vec::new();
    let mut curr = start_val;
    let max_len = 100_000;
    while (step_val > 0 && curr <= end_val) || (step_val < 0 && curr >= end_val) {
        values.push(curr);
        if values.len() >= max_len {
            break;
        }
        curr += step_val;
    }
    Some(values)
}

#[allow(dead_code)]
pub(super) fn global_subst_cache() -> &'static RwLock<HashMap<u128, Expression>> {
    static C: OnceLock<RwLock<HashMap<u128, Expression>>> = OnceLock::new();
    C.get_or_init(|| RwLock::new(HashMap::new()))
}

#[allow(dead_code)]
pub(super) fn hash_expr_bincode(expr: &Expression) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Ok(bytes) = bincode::serialize(expr) {
        bytes.hash(&mut h);
    } else {
        std::mem::discriminant(expr).hash(&mut h);
    }
    h.finish()
}

#[allow(dead_code)]
pub(super) fn hash_context(context: &HashMap<String, Expression>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut keys: Vec<&String> = context.keys().collect();
    keys.sort();
    for k in keys {
        k.hash(&mut h);
        if let Some(v) = context.get(k) {
            hash_expr_bincode(v).hash(&mut h);
        }
    }
    h.finish()
}
