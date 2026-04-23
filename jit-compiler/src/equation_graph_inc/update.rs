use std::collections::HashSet;

use crate::equation_graph::EquationGraph;

use super::diff::DirtySet;
use super::index::IndexedEquationGraph;

pub fn update_graph(
    prev: &IndexedEquationGraph,
    next: &IndexedEquationGraph,
    dirty: &DirtySet,
    impacted_eqs: &HashSet<usize>,
) -> EquationGraph {
    if dirty.changed_eqs.is_empty() && dirty.changed_vars.is_empty() {
        return prev.api_graph.clone();
    }
    if impacted_eqs.is_empty() {
        return prev.api_graph.clone();
    }
    next.api_graph.clone()
}

