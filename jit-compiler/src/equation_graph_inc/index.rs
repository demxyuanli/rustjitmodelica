use std::collections::{HashMap, HashSet};

use crate::analysis::extract_unknowns;
use crate::ast::{Equation, Expression};
use crate::equation_graph::{build_equation_graph_non_incremental, EquationGraph, EquationGraphMode};
use crate::flatten::FlattenedModel;
use crate::string_intern::resolve_id;

use super::keys::{variable_key, NodeKey, VarKey};

#[derive(Debug, Clone)]
pub struct IndexedEquationGraph {
    pub api_graph: EquationGraph,
    pub equation_keys: Vec<NodeKey>,
    pub reverse_index: HashMap<VarKey, Vec<usize>>,
    pub eq_to_vars: Vec<Vec<VarKey>>,
}

impl Default for IndexedEquationGraph {
    fn default() -> Self {
        Self {
            api_graph: EquationGraph {
                nodes: Vec::new(),
                edges: Vec::new(),
                truncated: false,
                total_equations: 0,
                included_equations: 0,
                omitted_equations: 0,
            },
            equation_keys: Vec::new(),
            reverse_index: HashMap::new(),
            eq_to_vars: Vec::new(),
        }
    }
}

fn equation_lhs_solved_var(eq: &Equation) -> Option<String> {
    match eq {
        Equation::Simple(lhs, _) => match lhs {
            Expression::Variable(id) => Some(resolve_id(*id)),
            Expression::Der(inner) => {
                if let Expression::Variable(id) = inner.as_ref() {
                    Some(format!("der_{}", resolve_id(*id)))
                } else {
                    None
                }
            }
            _ => None,
        },
        _ => None,
    }
}

fn normalize_top_level_var_name(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("der_") {
        let head = rest
            .split('.')
            .next()
            .unwrap_or(rest)
            .split('[')
            .next()
            .unwrap_or(rest);
        return format!("der_{}", head);
    }
    name.split('.')
        .next()
        .unwrap_or(name)
        .split('[')
        .next()
        .unwrap_or(name)
        .to_string()
}

pub fn build_indexed(flat_model: &FlattenedModel, mode: EquationGraphMode) -> IndexedEquationGraph {
    let api_graph = build_equation_graph_non_incremental(flat_model, mode);
    if matches!(mode, EquationGraphMode::Structural) {
        return IndexedEquationGraph {
            api_graph,
            ..IndexedEquationGraph::default()
        };
    }

    let mut known = HashSet::new();
    known.insert("time".to_string());
    for decl in &flat_model.declarations {
        if decl.is_parameter {
            known.insert(decl.name.clone());
        }
    }

    let mut equation_keys = Vec::with_capacity(flat_model.equations.len());
    let mut reverse_index: HashMap<VarKey, Vec<usize>> = HashMap::new();
    let mut eq_to_vars: Vec<Vec<VarKey>> = Vec::with_capacity(flat_model.equations.len());

    for (i, eq) in flat_model.equations.iter().enumerate() {
        equation_keys.push(NodeKey::Equation {
            index: i as u32,
            hash: super::keys::equation_hash(eq),
        });

        let mut vars: Vec<String> = extract_unknowns(eq, &known).into_iter().collect();
        if let Some(solved) = equation_lhs_solved_var(eq) {
            vars.push(solved);
        }
        if matches!(mode, EquationGraphMode::TopLevel) {
            vars = vars
                .into_iter()
                .map(|v| normalize_top_level_var_name(&v))
                .collect();
        }
        vars.sort();
        vars.dedup();

        let mut eq_vars = Vec::with_capacity(vars.len());
        for v in vars {
            let k = variable_key(&v);
            eq_vars.push(k);
            reverse_index.entry(k).or_default().push(i);
        }
        eq_to_vars.push(eq_vars);
    }

    IndexedEquationGraph {
        api_graph,
        equation_keys,
        reverse_index,
        eq_to_vars,
    }
}

