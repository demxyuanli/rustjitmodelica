use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

fn perf_map() -> &'static RwLock<HashMap<String, u64>> {
    static M: OnceLock<RwLock<HashMap<String, u64>>> = OnceLock::new();
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
        *g.entry(label.to_string()).or_insert(0) += ms;
    }
}

pub fn record_us(label: &'static str, us: u64) {
    if let Ok(mut g) = perf_map().write() {
        *g.entry(label.to_string()).or_insert(0) += us;
    }
}

/// Add to a counter stored in the same map as microsecond totals (distinct key names only).
pub fn record_add(label: impl AsRef<str>, delta: u64) {
    if let Ok(mut g) = perf_map().write() {
        *g.entry(label.as_ref().to_string()).or_insert(0) += delta;
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CacheEvent {
    Hit,
    Miss,
    Write,
    Invalidate,
    DepsMismatch,
}

pub fn record_cache_event(scope: &str, stage: &str, event: CacheEvent) {
    match event {
        CacheEvent::Hit => {
            match scope {
                "L0" => record_add("cache_L0_hits", 1),
                "L1" => record_add("cache_L1_hits", 1),
                _ => record_add("cache_L2_hits", 1),
            }
            record_add(format!("cache_stage_hits:{}:{}", scope, stage), 1);
        }
        CacheEvent::Miss => match scope {
            "L0" => record_add("cache_L0_misses", 1),
            "L1" => record_add("cache_L1_misses", 1),
            _ => record_add("cache_L2_misses", 1),
        },
        CacheEvent::Write => {
            record_add("cache_writes", 1);
            match scope {
                "L0" => record_add("cache_L0_writes", 1),
                "L1" => record_add("cache_L1_writes", 1),
                _ => record_add("cache_L2_writes", 1),
            }
            record_add(format!("cache_stage_writes:{}:{}", scope, stage), 1);
        }
        CacheEvent::Invalidate => {
            record_add("cache_invalidates", 1);
            record_add(format!("cache_stage_invalidations:{}:{}", scope, stage), 1);
        }
        CacheEvent::DepsMismatch => record_add("cache_deps_mismatch", 1),
    }
    if matches!(event, CacheEvent::Miss) {
        record_add(format!("cache_stage_misses:{}:{}", scope, stage), 1);
    }
}

pub fn snapshot() -> HashMap<String, u64> {
    perf_map().read().map(|g| g.clone()).unwrap_or_default()
}

