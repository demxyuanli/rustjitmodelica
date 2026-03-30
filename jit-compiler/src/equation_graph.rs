// Equation/variable dependency graph for analysis and debugging.
// Builds nodes (equations, variables) and edges (depends, solves) from a flattened model.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::analysis::extract_unknowns;
use crate::ast::{expr_to_connector_path, Equation, Expression, Model};
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
    pub truncated: bool,
    pub total_equations: usize,
    pub included_equations: usize,
    pub omitted_equations: usize,
}

const DEFAULT_MAX_GRAPH_NODES: usize = 480;
const DEFAULT_MAX_GRAPH_EDGES: usize = 2200;
const TOP_LEVEL_MAX_GRAPH_NODES: usize = 220;
const TOP_LEVEL_MAX_GRAPH_EDGES: usize = 1200;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EquationGraphMode {
    Full,
    Compact,
    TopLevel,
    /// Parse-level instances and connects only; no flatten (fast, approximate).
    Structural,
}

impl Default for EquationGraphMode {
    fn default() -> Self {
        EquationGraphMode::Compact
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

fn build_equation_variable_graph(
    flat_model: &FlattenedModel,
    max_nodes: Option<usize>,
    max_edges: Option<usize>,
) -> EquationGraph {
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
    let total_equations = flat_model.equations.len();
    let mut included_equations = 0usize;
    let mut truncated = false;

    for (i, eq) in flat_model.equations.iter().enumerate() {
        if max_nodes.is_some_and(|n| nodes.len() >= n) || max_edges.is_some_and(|e| edges.len() >= e) {
            truncated = true;
            break;
        }
        let eq_id = format!("eq_{}", i);
        if node_ids.insert(eq_id.clone()) {
            nodes.push(EquationGraphNode {
                id: eq_id.clone(),
                label: equation_short_label(eq, i),
                kind: "equation".to_string(),
            });
        }
        included_equations += 1;
        let unknowns = extract_unknowns(eq, &known);
        let solved = equation_lhs_solved_var(eq);
        for u in &unknowns {
            if max_nodes.is_some_and(|n| nodes.len() >= n) || max_edges.is_some_and(|e| edges.len() >= e) {
                truncated = true;
                break;
            }
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
        if truncated {
            break;
        }
        if let Some(solved) = solved {
            if max_nodes.is_some_and(|n| nodes.len() >= n) || max_edges.is_some_and(|e| edges.len() >= e) {
                truncated = true;
                break;
            }
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

    EquationGraph {
        nodes,
        edges,
        truncated,
        total_equations,
        included_equations,
        omitted_equations: total_equations.saturating_sub(included_equations),
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

fn build_top_level_component_graph(flat_model: &FlattenedModel) -> EquationGraph {
    let mut known = HashSet::new();
    known.insert("time".to_string());
    for decl in &flat_model.declarations {
        if decl.is_parameter {
            known.insert(decl.name.clone());
        }
    }

    let total_equations = flat_model.equations.len();
    let mut included_equations = 0usize;
    let mut truncated = false;

    let mut node_ids = HashSet::new();
    let mut edge_ids = HashSet::new();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for eq in &flat_model.equations {
        if nodes.len() >= TOP_LEVEL_MAX_GRAPH_NODES || edges.len() >= TOP_LEVEL_MAX_GRAPH_EDGES {
            truncated = true;
            break;
        }

        let mut vars: Vec<String> = extract_unknowns(eq, &known)
            .into_iter()
            .map(|v| normalize_top_level_var_name(&v))
            .collect();
        if let Some(solved) = equation_lhs_solved_var(eq) {
            vars.push(normalize_top_level_var_name(&solved));
        }
        vars.sort();
        vars.dedup();
        if vars.is_empty() {
            continue;
        }
        included_equations += 1;

        for v in &vars {
            let node_id = format!("c_{}", v.replace(' ', "_"));
            if node_ids.insert(node_id.clone()) {
                nodes.push(EquationGraphNode {
                    id: node_id,
                    label: v.clone(),
                    kind: "component".to_string(),
                });
            }
        }

        for left in 0..vars.len() {
            for right in (left + 1)..vars.len() {
                if edges.len() >= TOP_LEVEL_MAX_GRAPH_EDGES {
                    truncated = true;
                    break;
                }
                let a = vars[left].as_str();
                let b = vars[right].as_str();
                if a == b {
                    continue;
                }
                let (s, t) = if a < b { (a, b) } else { (b, a) };
                let edge_key = format!("{}->{}", s, t);
                if edge_ids.insert(edge_key) {
                    edges.push(EquationGraphEdge {
                        source: format!("c_{}", s.replace(' ', "_")),
                        target: format!("c_{}", t.replace(' ', "_")),
                        kind: "depends".to_string(),
                    });
                }
            }
            if truncated {
                break;
            }
        }
        if truncated {
            break;
        }
    }

    EquationGraph {
        nodes,
        edges,
        truncated,
        total_equations,
        included_equations,
        omitted_equations: total_equations.saturating_sub(included_equations),
    }
}

fn sanitize_graph_id(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn is_builtin_scalar_type(type_name: &str) -> bool {
    matches!(
        type_name.trim(),
        "Real" | "Integer" | "Boolean" | "String"
    )
}

/// Instance and `connect` graph from the parsed model only (no library flatten).
pub fn build_structural_graph(model: &Model) -> EquationGraph {
    let mut node_ids = HashSet::new();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for d in &model.declarations {
        if is_builtin_scalar_type(&d.type_name) {
            continue;
        }
        let id = format!("i_{}", sanitize_graph_id(&d.name));
        if node_ids.insert(id.clone()) {
            nodes.push(EquationGraphNode {
                id,
                label: format!("{} : {}", d.name, d.type_name),
                kind: "instance".to_string(),
            });
        }
    }

    let total_equations = model.equations.len();
    let mut included_equations = 0usize;
    for eq in &model.equations {
        if let Equation::Connect(a, b) = eq {
            let Some(sa) = expr_to_connector_path(a) else {
                continue;
            };
            let Some(sb) = expr_to_connector_path(b) else {
                continue;
            };
            let na = format!("c_{}", sanitize_graph_id(&sa));
            let nb = format!("c_{}", sanitize_graph_id(&sb));
            for (path, nid) in [(sa.as_str(), na.clone()), (sb.as_str(), nb.clone())] {
                if node_ids.insert(nid.clone()) {
                    nodes.push(EquationGraphNode {
                        id: nid,
                        label: path.to_string(),
                        kind: "connector".to_string(),
                    });
                }
            }
            edges.push(EquationGraphEdge {
                source: na,
                target: nb,
                kind: "connect".to_string(),
            });
            included_equations += 1;
        }
    }

    EquationGraph {
        nodes,
        edges,
        truncated: false,
        total_equations,
        included_equations,
        omitted_equations: total_equations.saturating_sub(included_equations),
    }
}

/// Build equation/variable dependency graph from a flattened model.
/// Known vars (parameters, time) are excluded from the variable set; equations reference unknowns.
pub fn build_equation_graph(flat_model: &FlattenedModel, mode: EquationGraphMode) -> EquationGraph {
    match mode {
        EquationGraphMode::Full => build_equation_variable_graph(flat_model, None, None),
        EquationGraphMode::Compact => build_equation_variable_graph(
            flat_model,
            Some(DEFAULT_MAX_GRAPH_NODES),
            Some(DEFAULT_MAX_GRAPH_EDGES),
        ),
        EquationGraphMode::TopLevel => build_top_level_component_graph(flat_model),
        EquationGraphMode::Structural => EquationGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            truncated: true,
            total_equations: flat_model.equations.len(),
            included_equations: 0,
            omitted_equations: flat_model.equations.len(),
        },
    }
}
