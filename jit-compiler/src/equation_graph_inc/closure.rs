use std::collections::{HashSet, VecDeque};

use super::index::IndexedEquationGraph;
use super::keys::VarKey;

pub fn closure_of(
    indexed: &IndexedEquationGraph,
    seed_vars: &HashSet<VarKey>,
    max_depth: Option<usize>,
) -> HashSet<usize> {
    let mut impacted_eqs = HashSet::new();
    let mut seen_vars = HashSet::new();
    let mut queue: VecDeque<(VarKey, usize)> = seed_vars.iter().map(|v| (*v, 0usize)).collect();

    while let Some((var, depth)) = queue.pop_front() {
        if !seen_vars.insert(var) {
            continue;
        }
        if let Some(eq_idxs) = indexed.reverse_index.get(&var) {
            for eq_idx in eq_idxs {
                if !impacted_eqs.insert(*eq_idx) {
                    continue;
                }
                if max_depth.is_some_and(|d| depth >= d) {
                    continue;
                }
                if let Some(vars) = indexed.eq_to_vars.get(*eq_idx) {
                    for v in vars {
                        if !seen_vars.contains(v) {
                            queue.push_back((*v, depth + 1));
                        }
                    }
                }
            }
        }
    }
    impacted_eqs
}

