use crate::ast::{AlgorithmStatement, Equation, Expression};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::structures::{ClockPartition, FlattenedModel};
use super::Flattener;

impl Flattener {
    pub(super) fn infer_clocked_variables(&self, flat: &mut FlattenedModel) {
        fn expr_contains_clock(e: &Expression) -> bool {
            match e {
                // Any sample(...) interval expression is a clock (inner may be a constant number).
                Expression::Sample(_) => true,
                Expression::Interval(inner)
                | Expression::Hold(inner)
                | Expression::Previous(inner) => expr_contains_clock(inner),
                Expression::SubSample(c, n)
                | Expression::SuperSample(c, n)
                | Expression::ShiftSample(c, n) => expr_contains_clock(c) || expr_contains_clock(n),
                Expression::BinaryOp(l, _, r) => expr_contains_clock(l) || expr_contains_clock(r),
                Expression::Call(_, args) => args.iter().any(expr_contains_clock),
                Expression::ArrayAccess(base, idx) => {
                    expr_contains_clock(base) || expr_contains_clock(idx)
                }
                Expression::Dot(base, _) => expr_contains_clock(base),
                Expression::If(c, t, f) => {
                    expr_contains_clock(c) || expr_contains_clock(t) || expr_contains_clock(f)
                }
                Expression::Range(a, b, c) => {
                    expr_contains_clock(a) || expr_contains_clock(b) || expr_contains_clock(c)
                }
                Expression::ArrayLiteral(items) => items.iter().any(expr_contains_clock),
                _ => false,
            }
        }

        fn clock_partition_key_from_condition(cond: &Expression) -> String {
            let fallback = || {
                let mut h = DefaultHasher::new();
                format!("{:?}", cond).hash(&mut h);
                format!("clock_{:016x}", h.finish())
            };
            match cond {
                Expression::Sample(inner) => match inner.as_ref() {
                    Expression::Number(dt) => format!("sample_{}", dt),
                    _ => fallback(),
                },
                Expression::Call(name, args) if name == "sample" || name.ends_with(".sample") => {
                    let dt = args.first().and_then(|a| {
                        if let Expression::Number(n) = a {
                            Some(*n)
                        } else {
                            None
                        }
                    });
                    let st = args.get(1).and_then(|a| {
                        if let Expression::Number(n) = a {
                            Some(*n)
                        } else {
                            None
                        }
                    });
                    match (dt, st) {
                        (Some(d), Some(s)) => format!("sample_{}_{}", d, s),
                        (Some(d), None) => format!("sample_{}", d),
                        _ => fallback(),
                    }
                }
                Expression::SubSample(c, n) => format!(
                    "subSample_{}_{}",
                    clock_partition_key_from_condition(c),
                    format!("{:?}", n)
                ),
                Expression::SuperSample(c, n) => format!(
                    "superSample_{}_{}",
                    clock_partition_key_from_condition(c),
                    format!("{:?}", n)
                ),
                Expression::ShiftSample(c, n) => format!(
                    "shiftSample_{}_{}",
                    clock_partition_key_from_condition(c),
                    format!("{:?}", n)
                ),
                _ => fallback(),
            }
        }

        fn eff_partition_id(in_clocked: bool, is_clock_cond: bool, cond: &Expression, inherited: Option<&str>) -> Option<String> {
            if !in_clocked {
                return None;
            }
            if is_clock_cond {
                Some(clock_partition_key_from_condition(cond))
            } else {
                Some(
                    inherited
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "default".to_string()),
                )
            }
        }

        fn collect_lhs_vars(expr: &Expression, out: &mut std::collections::HashSet<String>) {
            match expr {
                Expression::Variable(name) => {
                    out.insert(name.clone());
                }
                Expression::Der(inner) => collect_lhs_vars(inner, out),
                Expression::ArrayAccess(base, _) => collect_lhs_vars(base, out),
                Expression::Dot(base, _) => collect_lhs_vars(base, out),
                Expression::ArrayLiteral(items) => {
                    for e in items {
                        collect_lhs_vars(e, out);
                    }
                }
                _ => {}
            }
        }

        let mut partitions: std::collections::BTreeMap<String, std::collections::HashSet<String>> =
            std::collections::BTreeMap::new();
        let mut union_all = std::collections::HashSet::new();

        fn add_clocks(
            vars: &mut std::collections::HashSet<String>,
            partmap: &mut std::collections::BTreeMap<String, std::collections::HashSet<String>>,
            union: &mut std::collections::HashSet<String>,
            part_id: &str,
        ) {
            for v in vars.drain() {
                union.insert(v.clone());
                partmap.entry(part_id.to_string()).or_default().insert(v);
            }
        }

        fn walk_algorithms(
            stmts: &[AlgorithmStatement],
            in_clocked: bool,
            inherited_part: Option<&str>,
            partmap: &mut std::collections::BTreeMap<String, std::collections::HashSet<String>>,
            union: &mut std::collections::HashSet<String>,
        ) {
            for stmt in stmts {
                match stmt {
                    AlgorithmStatement::Assignment(lhs, _) => {
                        if in_clocked {
                            let mut vs = std::collections::HashSet::new();
                            collect_lhs_vars(lhs, &mut vs);
                            let pid = inherited_part.unwrap_or("default");
                            add_clocks(&mut vs, partmap, union, pid);
                        }
                    }
                    AlgorithmStatement::MultiAssign(lhss, _) => {
                        if in_clocked {
                            let mut vs = std::collections::HashSet::new();
                            for lhs in lhss {
                                collect_lhs_vars(lhs, &mut vs);
                            }
                            let pid = inherited_part.unwrap_or("default");
                            add_clocks(&mut vs, partmap, union, pid);
                        }
                    }
                    AlgorithmStatement::CallStmt(_) => {}
                    AlgorithmStatement::NoOp => {}
                    AlgorithmStatement::If(_, then_stmts, else_ifs, else_stmts) => {
                        walk_algorithms(then_stmts, in_clocked, inherited_part, partmap, union);
                        for (_, s) in else_ifs {
                            walk_algorithms(s, in_clocked, inherited_part, partmap, union);
                        }
                        if let Some(s) = else_stmts {
                            walk_algorithms(s, in_clocked, inherited_part, partmap, union);
                        }
                    }
                    AlgorithmStatement::While(_, body) => {
                        walk_algorithms(body, in_clocked, inherited_part, partmap, union);
                    }
                    AlgorithmStatement::For(_, _, body) => {
                        walk_algorithms(body, in_clocked, inherited_part, partmap, union);
                    }
                    AlgorithmStatement::When(cond, body, else_whens) => {
                        let is_clock = expr_contains_clock(cond);
                        let child_in_clocked = in_clocked || is_clock;
                        let child_part = eff_partition_id(child_in_clocked, is_clock, cond, inherited_part);
                        let part_ref = child_part.as_deref();
                        walk_algorithms(body, child_in_clocked, part_ref, partmap, union);
                        for (c, s) in else_whens {
                            let ec = expr_contains_clock(c);
                            let cin = in_clocked || ec;
                            let cp = eff_partition_id(cin, ec, c, inherited_part);
                            walk_algorithms(s, cin, cp.as_deref(), partmap, union);
                        }
                    }
                    AlgorithmStatement::Reinit(var, _) => {
                        if in_clocked {
                            let mut vs = std::collections::HashSet::new();
                            vs.insert(var.clone());
                            let pid = inherited_part.unwrap_or("default");
                            add_clocks(&mut vs, partmap, union, pid);
                        }
                    }
                    AlgorithmStatement::Assert(_, _) | AlgorithmStatement::Terminate(_) => {}
                }
            }
        }

        fn walk_equations(
            eqs: &[Equation],
            in_clocked: bool,
            inherited_part: Option<&str>,
            partmap: &mut std::collections::BTreeMap<String, std::collections::HashSet<String>>,
            union: &mut std::collections::HashSet<String>,
        ) {
            for eq in eqs {
                match eq {
                    Equation::Simple(lhs, rhs) => {
                        let is_clock = expr_contains_clock(lhs) || expr_contains_clock(rhs);
                        if in_clocked || is_clock {
                            let mut vs = std::collections::HashSet::new();
                            collect_lhs_vars(lhs, &mut vs);
                            let pid = if in_clocked {
                                inherited_part.unwrap_or("default").to_string()
                            } else if expr_contains_clock(lhs) {
                                clock_partition_key_from_condition(lhs)
                            } else {
                                clock_partition_key_from_condition(rhs)
                            };
                            add_clocks(&mut vs, partmap, union, &pid);
                        }
                    }
                    Equation::MultiAssign(lhss, rhs) => {
                        let is_clock = expr_contains_clock(rhs);
                        if in_clocked || is_clock {
                            let mut vs = std::collections::HashSet::new();
                            for lhs in lhss {
                                collect_lhs_vars(lhs, &mut vs);
                            }
                            let pid = inherited_part.unwrap_or("default").to_string();
                            add_clocks(&mut vs, partmap, union, &pid);
                        }
                    }
                    Equation::For(_, _, _, body) => {
                        walk_equations(body, in_clocked, inherited_part, partmap, union)
                    }
                    Equation::Connect(_, _) | Equation::CallStmt(_) => {}
                    Equation::When(cond, body, else_whens) => {
                        let is_clock = expr_contains_clock(cond);
                        let child_in_clocked = in_clocked || is_clock;
                        let child_part = eff_partition_id(child_in_clocked, is_clock, cond, inherited_part);
                        let part_ref = child_part.as_deref();
                        walk_equations(body, child_in_clocked, part_ref, partmap, union);
                        for (c, b) in else_whens {
                            let ec = expr_contains_clock(c);
                            let cin = in_clocked || ec;
                            let cp = eff_partition_id(cin, ec, c, inherited_part);
                            walk_equations(b, cin, cp.as_deref(), partmap, union);
                        }
                    }
                    Equation::If(cond, then_eqs, else_ifs, else_eqs) => {
                        let branch_clocked = in_clocked || expr_contains_clock(cond);
                        let p = if branch_clocked {
                            Some(inherited_part.unwrap_or("default"))
                        } else {
                            None
                        };
                        walk_equations(then_eqs, branch_clocked, p, partmap, union);
                        for (c, b) in else_ifs {
                            let elseif_clocked = in_clocked || expr_contains_clock(c);
                            let p2 = if elseif_clocked {
                                Some(inherited_part.unwrap_or("default"))
                            } else {
                                None
                            };
                            walk_equations(b, elseif_clocked, p2, partmap, union);
                        }
                        if let Some(b) = else_eqs {
                            walk_equations(b, in_clocked, inherited_part, partmap, union);
                        }
                    }
                    Equation::Reinit(var, expr) => {
                        if in_clocked || expr_contains_clock(expr) {
                            let mut vs = std::collections::HashSet::new();
                            vs.insert(var.clone());
                            let pid = inherited_part.unwrap_or("default").to_string();
                            add_clocks(&mut vs, partmap, union, &pid);
                        }
                    }
                    Equation::Assert(_, _) | Equation::Terminate(_) => {}
                    Equation::SolvableBlock { equations, .. } => {
                        walk_equations(equations, in_clocked, inherited_part, partmap, union)
                    }
                }
            }
        }

        walk_algorithms(
            &flat.algorithms,
            false,
            None,
            &mut partitions,
            &mut union_all,
        );
        walk_algorithms(
            &flat.initial_algorithms,
            false,
            None,
            &mut partitions,
            &mut union_all,
        );
        walk_equations(
            &flat.equations,
            false,
            None,
            &mut partitions,
            &mut union_all,
        );
        walk_equations(
            &flat.initial_equations,
            false,
            None,
            &mut partitions,
            &mut union_all,
        );

        flat.clocked_var_names = union_all;
        flat.clock_partitions.clear();
        for (id, var_names) in partitions {
            if !var_names.is_empty() {
                flat.clock_partitions.push(ClockPartition { id, var_names });
            }
        }
    }
}
