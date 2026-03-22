use crate::analysis::normalize_der;
use crate::ast::{Equation, Expression};
use crate::compiler::equation_convert;
use crate::jit::{ArrayInfo, ArrayType};

use super::types::VariableLayout;

pub(crate) fn normalize_equations(equations: &[Equation]) -> Vec<Equation> {
    equations.iter().map(normalize_equation).collect()
}

pub(crate) fn normalize_equation(eq: &Equation) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => Equation::Simple(normalize_der(lhs), normalize_der(rhs)),
        Equation::For(var, start, end, body) => Equation::For(
            var.clone(),
            start.clone(),
            end.clone(),
            body.iter().map(normalize_simple_equation).collect(),
        ),
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => Equation::If(
            normalize_der(cond),
            then_eqs.iter().map(normalize_simple_equation).collect(),
            elseif_list
                .iter()
                .map(|(elseif_cond, body)| {
                    (
                        normalize_der(elseif_cond),
                        body.iter().map(normalize_simple_equation).collect(),
                    )
                })
                .collect(),
            else_eqs
                .as_ref()
                .map(|eqs| eqs.iter().map(normalize_simple_equation).collect()),
        ),
        _ => eq.clone(),
    }
}

pub(crate) fn normalize_simple_equation(eq: &Equation) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => Equation::Simple(normalize_der(lhs), normalize_der(rhs)),
        _ => eq.clone(),
    }
}

pub(crate) fn ensure_derivative_outputs(layout: &mut VariableLayout) {
    for var in &layout.state_vars {
        let der_var = format!("der_{}", var);
        if !layout.output_var_index.contains_key(&der_var) {
            let pos = layout.output_vars.len();
            layout.output_var_index.insert(der_var.clone(), pos);
            layout.output_vars.push(der_var);
        }
    }

    let mut derived_array_entries = Vec::new();
    for (name, info) in &layout.array_info {
        if !matches!(info.array_type, ArrayType::State) {
            continue;
        }
        let der_name = format!("der_{}", name);
        if layout.array_info.contains_key(&der_name) {
            continue;
        }
        let first_der = format!("der_{}_1", name);
        if let Some(&start_index) = layout.output_var_index.get(&first_der) {
            derived_array_entries.push((
                der_name,
                ArrayInfo {
                    array_type: ArrayType::Output,
                    start_index,
                    size: info.size,
                },
            ));
        }
    }
    for (name, info) in derived_array_entries {
        layout.array_info.insert(name, info);
    }
}

pub(crate) fn build_diff_equations(layout: &VariableLayout) -> Vec<Equation> {
    layout
        .state_vars
        .iter()
        .map(|var| {
            let rhs_expr = if let Some((base, idx)) = equation_convert::parse_array_index(var) {
                if layout.array_info.contains_key(&base)
                    && layout.array_info.contains_key(&format!("der_{}", base))
                {
                    Expression::ArrayAccess(
                        Box::new(Expression::var(&format!("der_{}", base))),
                        Box::new(Expression::Number(idx as f64)),
                    )
                } else {
                    Expression::var(&format!("der_{}", var))
                }
            } else {
                Expression::var(&format!("der_{}", var))
            };

            Equation::Simple(
                Expression::Der(Box::new(Expression::var(var))),
                rhs_expr,
            )
        })
        .collect()
}
