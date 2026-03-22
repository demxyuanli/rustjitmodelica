// Equation/variable dependency graph for analysis and debugging.
// Builds nodes (equations, variables) and edges (depends, solves) from a flattened model.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::analysis::extract_unknowns;
use crate::ast::{Equation, Expression};
use crate::flatten::FlattenedModel;
use crate::string_intern::resolve_id;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquationGraphNode {
    pub id: String,
    pub label: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquationGraphEdge {
    pub source: String,
    pub target: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquationGraph {
    pub nodes: Vec<EquationGraphNode>,
    pub edges: Vec<EquationGraphEdge>,
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

fn equation_short_label(eq: &Equation, index: usize) -> String {
    match eq {
        Equation::Simple(lhs, rhs) => {
            let l = expr_short(lhs);
            let r = expr_short(rhs);
            if l.len() + r.len() < 50 {
                format!("{} = {}", l, r)
            } else {
                format!("eq[{}]", index)
            }
        }
        _ => format!("eq[{}]", index),
    }
}

fn expr_short(e: &Expression) -> String {
    match e {
        Expression::Variable(id) => resolve_id(*id),
        Expression::Der(inner) => format!("der({})", expr_short(inner)),
        Expression::Number(x) => format!("{}", x),
        Expression::BinaryOp(l, op, r) => {
            let op_str = match op {
                crate::ast::Operator::Add => "+",
                crate::ast::Operator::Sub => "-",
                crate::ast::Operator::Mul => "*",
                crate::ast::Operator::Div => "/",
                _ => "?",
            };
            format!("{} {} {}", expr_short(l), op_str, expr_short(r))
        }
        _ => "...".to_string(),
    }
}

/// Build equation/variable dependency graph from a flattened model.
/// Known vars (parameters, time) are excluded from the variable set; equations reference unknowns.
pub fn build_equation_graph(flat_model: &FlattenedModel) -> EquationGraph {
    let mut known = HashSet::new();
    known.insert("time".to_string());
    for decl in &flat_model.declarations {
        if decl.is_parameter {
            known.insert(decl.name.clone());
        }
    }

    let mut node_ids = HashSet::new();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    let mut all_vars: HashSet<String> = HashSet::new();
    for (_i, eq) in flat_model.equations.iter().enumerate() {
        let unknowns = extract_unknowns(eq, &known);
        for u in &unknowns {
            all_vars.insert(u.clone());
        }
        if let Some(solved) = equation_lhs_solved_var(eq) {
            all_vars.insert(solved);
        }
    }

    for (i, eq) in flat_model.equations.iter().enumerate() {
        let eq_id = format!("eq_{}", i);
        if node_ids.insert(eq_id.clone()) {
            nodes.push(EquationGraphNode {
                id: eq_id.clone(),
                label: equation_short_label(eq, i),
                kind: "equation".to_string(),
            });
        }
        let unknowns = extract_unknowns(eq, &known);
        let solved = equation_lhs_solved_var(eq);
        for u in &unknowns {
            let var_id = format!("v_{}", u.replace('.', "_").replace(' ', "_"));
            if node_ids.insert(var_id.clone()) {
                nodes.push(EquationGraphNode {
                    id: var_id.clone(),
                    label: u.clone(),
                    kind: "variable".to_string(),
                });
            }
            if Some(u.as_str()) != solved.as_deref() {
                edges.push(EquationGraphEdge {
                    source: eq_id.clone(),
                    target: var_id,
                    kind: "depends".to_string(),
                });
            }
        }
        if let Some(solved) = solved {
            if all_vars.contains(&solved) {
                let var_id = format!("v_{}", solved.replace('.', "_"));
                if node_ids.insert(var_id.clone()) {
                    nodes.push(EquationGraphNode {
                        id: var_id.clone(),
                        label: solved.clone(),
                        kind: "variable".to_string(),
                    });
                }
                edges.push(EquationGraphEdge {
                    source: eq_id,
                    target: var_id,
                    kind: "solves".to_string(),
                });
            }
        }
    }

    EquationGraph { nodes, edges }
}
