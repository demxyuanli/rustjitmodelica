use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::equation_graph::{EquationGraph, EquationGraphMode};
use crate::flatten::FlattenedModel;

mod closure;
mod diff;
mod index;
mod keys;
mod update;

pub use diff::DirtySet;
pub use keys::NodeKey;

#[derive(Debug, Clone)]
struct SessionEntry {
    snapshot: diff::FlatSnapshot,
    indexed: index::IndexedEquationGraph,
}

fn session_store() -> &'static Mutex<HashMap<EquationGraphMode, SessionEntry>> {
    static STORE: OnceLock<Mutex<HashMap<EquationGraphMode, SessionEntry>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn merge_caller_dirty(
    dirty: &mut DirtySet,
    next_snapshot: &diff::FlatSnapshot,
    caller_dirty: Option<&[NodeKey]>,
) {
    if let Some(keys) = caller_dirty {
        let has_equation_keys = keys.iter().any(|k| matches!(k, NodeKey::Equation { .. }));
        if has_equation_keys {
            // Caller-supplied equation dirty set has priority over auto-diff for equation indices.
            dirty.changed_eqs.clear();
        }
        for k in keys {
            match k {
                NodeKey::Equation { index, hash } => {
                    let idx = *index as usize;
                    let backend_hash = next_snapshot.equation_hashes.get(idx).copied().unwrap_or(0);
                    // Caller hash matches backend canonical hash: formatting-only/no-op for this equation.
                    if *hash != 0 && backend_hash == *hash {
                        crate::query_db::perf_record_add("eqgraph_caller_hash_match_skip_count", 1);
                        continue;
                    }
                    // Unknown backend hash (out of range) or mismatched hash: keep conservative dirty mark.
                    if backend_hash == 0 {
                        crate::query_db::perf_record_add(
                            "eqgraph_caller_hash_out_of_range_dirty_count",
                            1,
                        );
                        dirty.changed_eqs.insert(idx);
                    } else {
                        crate::query_db::perf_record_add("eqgraph_caller_hash_mismatch_dirty_count", 1);
                        dirty.changed_eqs.insert(idx);
                    }
                }
                NodeKey::Variable(v) | NodeKey::TopLevelComponent(v) => {
                    crate::query_db::perf_record_add("eqgraph_caller_var_dirty_count", 1);
                    dirty.changed_vars.insert(*v);
                }
            }
        }
    }
}

pub fn build_or_update_equation_graph(
    flat_model: &FlattenedModel,
    mode: EquationGraphMode,
    caller_dirty: Option<&[NodeKey]>,
) -> EquationGraph {
    let next_indexed = index::build_indexed(flat_model, mode);
    let next_snapshot = diff::snapshot_flat(flat_model);

    let mut store = match session_store().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    let out = if let Some(prev) = store.get(&mode) {
        let mut dirty = diff::diff_flat_models(&prev.snapshot, &next_snapshot);
        merge_caller_dirty(&mut dirty, &next_snapshot, caller_dirty);
        let mut impacted_eqs = closure::closure_of(&prev.indexed, &dirty.changed_vars, None);
        impacted_eqs.extend(dirty.changed_eqs.iter().copied());
        update::update_graph(&prev.indexed, &next_indexed, &dirty, &impacted_eqs)
    } else {
        next_indexed.api_graph.clone()
    };

    store.insert(
        mode,
        SessionEntry {
            snapshot: next_snapshot,
            indexed: next_indexed,
        },
    );
    out
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::time::Instant;

    use crate::Compiler;
    use crate::flatten::FlattenedModel;
    use crate::query_db::{perf_reset, perf_snapshot};
    use crate::string_intern::StringInterner;

    use super::{closure::closure_of, diff::diff_flat_models, diff::snapshot_flat, EquationGraphMode};
    use super::{index::IndexedEquationGraph, keys::variable_key};

    fn fake_flat() -> FlattenedModel {
        FlattenedModel {
            declarations: Vec::new(),
            equations: Vec::new(),
            algorithms: Vec::new(),
            initial_equations: Vec::new(),
            initial_algorithms: Vec::new(),
            connections: Vec::new(),
            conditional_connections: Vec::new(),
            instances: HashMap::new(),
            array_sizes: HashMap::new(),
            clocked_var_names: HashSet::new(),
            clock_partitions: Vec::new(),
            clock_signal_connections: Vec::new(),
            stream_peer_map: HashMap::new(),
            stream_connection_set: HashMap::new(),
            stream_flow_map: HashMap::new(),
            expandable_instances: HashSet::new(),
            interner: StringInterner::new(),
            inst_records: Vec::new(),
            path_to_inst: HashMap::new(),
        }
    }

    #[test]
    fn diff_flat_models_empty_when_same_snapshot() {
        let f = fake_flat();
        let s = snapshot_flat(&f);
        let d = diff_flat_models(&s, &s);
        assert!(d.changed_eqs.is_empty());
        assert!(d.changed_vars.is_empty());
    }

    #[test]
    fn closure_bfs_two_hops() {
        let mut indexed = IndexedEquationGraph::default();
        let va = variable_key("a");
        let vb = variable_key("b");
        let vc = variable_key("c");
        indexed.reverse_index.insert(va, vec![0]);
        indexed.reverse_index.insert(vb, vec![0, 1]);
        indexed.reverse_index.insert(vc, vec![1]);
        indexed.eq_to_vars = vec![vec![va, vb], vec![vb, vc]];
        let mut seed = HashSet::new();
        seed.insert(va);

        let one_hop = closure_of(&indexed, &seed, Some(0));
        assert!(one_hop.contains(&0));
        assert!(!one_hop.contains(&1));

        let two_hop = closure_of(&indexed, &seed, Some(2));
        assert!(two_hop.contains(&0));
        assert!(two_hop.contains(&1));
    }

    #[test]
    fn fallback_entry_works_on_empty_flat() {
        let f = fake_flat();
        let g = super::build_or_update_equation_graph(&f, EquationGraphMode::Compact, None);
        assert!(g.nodes.is_empty());
        assert!(g.edges.is_empty());
    }

    #[test]
    fn merge_caller_dirty_skips_equation_when_hash_matches_canonical() {
        perf_reset();
        let mut dirty = super::DirtySet::default();
        dirty.changed_eqs.insert(0);
        let snapshot = super::diff::FlatSnapshot {
            equation_hashes: vec![1234],
            declaration_names: HashSet::new(),
        };
        let keys = vec![super::NodeKey::Equation { index: 0, hash: 1234 }];
        super::merge_caller_dirty(&mut dirty, &snapshot, Some(&keys));
        assert!(dirty.changed_eqs.is_empty());
        let perf = perf_snapshot();
        assert_eq!(
            perf.get("eqgraph_caller_hash_match_skip_count").copied().unwrap_or(0),
            1
        );
    }

    #[test]
    fn merge_caller_dirty_marks_equation_when_hash_differs() {
        perf_reset();
        let mut dirty = super::DirtySet::default();
        let snapshot = super::diff::FlatSnapshot {
            equation_hashes: vec![1234],
            declaration_names: HashSet::new(),
        };
        let keys = vec![super::NodeKey::Equation { index: 0, hash: 5678 }];
        super::merge_caller_dirty(&mut dirty, &snapshot, Some(&keys));
        assert!(dirty.changed_eqs.contains(&0));
        let perf = perf_snapshot();
        assert_eq!(
            perf.get("eqgraph_caller_hash_mismatch_dirty_count")
                .copied()
                .unwrap_or(0),
            1
        );
    }

    struct EnvGuard {
        key: &'static str,
        old: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let old = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, old }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(v) = &self.old {
                std::env::set_var(self.key, v);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn run_two_versions(incremental: bool) -> (crate::EquationGraph, u128) {
        let _g = EnvGuard::set(
            "RUSTMODLICA_EQGRAPH_INCREMENTAL",
            if incremental { "1" } else { "0" },
        );
        let mut compiler = Compiler::new();
        let model_name = "EqGraphAB";
        let code_v1 = r#"
model EqGraphAB
  Real x;
  Real y;
equation
  x = time;
  y = x + 1;
end EqGraphAB;
"#;
        let code_v2 = r#"
model EqGraphAB
  Real x;
  Real y;
equation
  x = time + 2;
  y = x + 1;
end EqGraphAB;
"#;
        let t0 = Instant::now();
        let _ = compiler
            .get_equation_graph_from_source_with_dirty(
                model_name,
                code_v1,
                EquationGraphMode::Compact,
                None,
            )
            .expect("v1 graph");
        let g2 = compiler
            .get_equation_graph_from_source_with_dirty(
                model_name,
                code_v2,
                EquationGraphMode::Compact,
                None,
            )
            .expect("v2 graph");
        (g2, t0.elapsed().as_millis())
    }

    fn normalize_graph(mut g: crate::EquationGraph) -> crate::EquationGraph {
        g.nodes
            .sort_by(|a, b| a.id.cmp(&b.id).then(a.kind.cmp(&b.kind)).then(a.label.cmp(&b.label)));
        g.edges.sort_by(|a, b| {
            a.source
                .cmp(&b.source)
                .then(a.target.cmp(&b.target))
                .then(a.kind.cmp(&b.kind))
        });
        g
    }

    #[test]
    fn ab_incremental_consistency_and_timing() {
        let (g_base, base_ms) = run_two_versions(false);
        let (g_inc, inc_ms) = run_two_versions(true);
        let base_json = serde_json::to_string(&normalize_graph(g_base)).expect("serialize base graph");
        let inc_json = serde_json::to_string(&normalize_graph(g_inc)).expect("serialize inc graph");
        assert_eq!(base_json, inc_json);
        eprintln!(
            "eqgraph_ab same-model-edit: baseline_ms={} incremental_ms={}",
            base_ms, inc_ms
        );
    }
}

