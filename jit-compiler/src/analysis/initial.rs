use crate::ast::Equation;
use std::collections::HashSet;

use super::blt::eliminate_aliases;
use super::variable_collection::{collect_vars_eq, extract_unknowns};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct InitialSystemInfo {
    pub equation_count: usize,
    pub variable_count: usize,
    pub alias_eliminated_count: usize,
    pub is_underdetermined: bool,
    pub is_overdetermined: bool,
}

pub fn analyze_initial_equations(
    initial_equations: &[Equation],
    known_at_initial: &HashSet<String>,
) -> InitialSystemInfo {
    let (eqs_after_alias, alias_map) = eliminate_aliases(initial_equations.to_vec());
    let mut var_set = HashSet::new();
    for eq in &eqs_after_alias {
        collect_vars_eq(eq, &mut var_set);
    }
    for v in alias_map.keys() {
        var_set.insert(v.clone());
    }
    let unknown_set: HashSet<String> = var_set.difference(known_at_initial).cloned().collect();
    let variable_count = unknown_set.len();
    let equation_count = eqs_after_alias.len();
    let is_underdetermined = equation_count < variable_count;
    let is_overdetermined = equation_count > variable_count;
    InitialSystemInfo {
        equation_count,
        variable_count,
        alias_eliminated_count: alias_map.len(),
        is_underdetermined,
        is_overdetermined,
    }
}

pub fn order_initial_equations_for_application(
    initial_equations: &[Equation],
    known_at_initial: &HashSet<String>,
) -> Vec<usize> {
    let mut indexed: Vec<(usize, usize)> = initial_equations
        .iter()
        .enumerate()
        .map(|(i, eq)| {
            let n = extract_unknowns(eq, known_at_initial).len();
            (i, n)
        })
        .collect();
    indexed.sort_by_key(|&(_, n)| n);
    indexed.into_iter().map(|(i, _)| i).collect()
}
