//! Per-connector connection counts from flatten `connect(a,b)` pairs (for `cardinality` JIT).

use std::collections::HashMap;
use xxhash_rust::xxh64::Xxh64;

pub fn build_connector_connection_degree(connections: &[(String, String)]) -> HashMap<String, usize> {
    let mut m = HashMap::new();
    for (a, b) in connections {
        *m.entry(a.clone()).or_insert(0) += 1;
        *m.entry(b.clone()).or_insert(0) += 1;
    }
    m
}

/// Stable digest for JIT / codegen cache keys when cardinality depends on the connection graph.
pub fn connector_degree_cache_digest(m: &HashMap<String, usize>) -> String {
    let mut pairs: Vec<(&String, &usize)> = m.iter().collect();
    pairs.sort_by_key(|(k, _)| *k);
    let mut h = Xxh64::new(0);
    for (k, c) in pairs {
        h.update(k.as_bytes());
        let c64 = *c as u64;
        h.update(&c64.to_le_bytes());
    }
    format!("cd:{:016x}", h.digest())
}
