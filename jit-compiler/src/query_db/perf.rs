use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

fn perf_map() -> &'static RwLock<HashMap<&'static str, u64>> {
    static M: OnceLock<RwLock<HashMap<&'static str, u64>>> = OnceLock::new();
    M.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn reset() {
    if let Ok(mut g) = perf_map().write() {
        g.clear();
    }
}

#[allow(dead_code)]
pub fn record_ms(label: &'static str, ms: u64) {
    if let Ok(mut g) = perf_map().write() {
        *g.entry(label).or_insert(0) += ms;
    }
}

pub fn record_us(label: &'static str, us: u64) {
    if let Ok(mut g) = perf_map().write() {
        *g.entry(label).or_insert(0) += us;
    }
}

/// Add to a counter stored in the same map as microsecond totals (distinct key names only).
pub fn record_add(label: &'static str, delta: u64) {
    if let Ok(mut g) = perf_map().write() {
        *g.entry(label).or_insert(0) += delta;
    }
}

pub fn snapshot() -> HashMap<&'static str, u64> {
    perf_map().read().map(|g| g.clone()).unwrap_or_default()
}

