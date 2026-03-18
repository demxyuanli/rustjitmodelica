use crate::ast::{AlgorithmStatement, Equation, Expression};
use std::collections::HashMap;

use super::expressions::{eval_const_expr, expr_to_path, index_expression, prefix_expression};
use super::utils::{convert_eq_to_alg, get_function_outputs};
use super::ExpandTarget;

impl super::Flattener {
    pub(crate) fn expand_equation_list(
        &mut self,
        equations: &[Equation],
        prefix: &str,
        target: &mut ExpandTarget,
        context_stack: &mut Vec<HashMap<String, Expression>>,
        instances: &HashMap<String, String>,
        when_condition: Option<Expression>,
    ) {
        for eq in equations {
            match eq {
                Equation::CallStmt(_) => {
                    // Parse-only: ignore call statements in equation sections.
                }
                Equation::Simple(lhs, rhs) => {
                    let lhs_sub = self.substitute_stack(lhs, context_stack);
                    let rhs_sub = self.substitute_stack(rhs, context_stack);
                    let lhs_pre = prefix_expression(&lhs_sub, prefix);
                    let rhs_pre = prefix_expression(&rhs_sub, prefix);
                    if let Expression::Variable(name) = &lhs_pre {
                        if let Some(&size) = target.array_sizes.get(name) {
                            for i in 1..=size {
                                let lhs_i = index_expression(&lhs_pre, i);
                                let rhs_i = index_expression(&rhs_pre, i);
                                let lhs_flat = prefix_expression(&lhs_i, "");
                                let rhs_flat = prefix_expression(&rhs_i, "");
                                target.equations.push(Equation::Simple(lhs_flat, rhs_flat));
                            }
                            continue;
                        }
                    }
                    if let Expression::Der(arg) = &lhs_pre {
                        if let Expression::Variable(name) = &**arg {
                            if let Some(&size) = target.array_sizes.get(name) {
                                for i in 1..=size {
                                    let lhs_i =
                                        Expression::Der(Box::new(index_expression(&**arg, i)));
                                    let rhs_i = index_expression(&rhs_pre, i);
                                    let lhs_flat = prefix_expression(&lhs_i, "");
                                    let rhs_flat = prefix_expression(&rhs_i, "");
                                    target.equations.push(Equation::Simple(lhs_flat, rhs_flat));
                                }
                                continue;
                            }
                        }
                    }
                    if let (Expression::Variable(n1), Expression::Variable(n2)) =
                        (&lhs_pre, &rhs_pre)
                    {
                        let ty1 = instances.get(n1).map(|s| s.as_str());
                        let ty2 = instances.get(n2).map(|s| s.as_str());
                        if let (Some(t1), Some(t2)) = (ty1, ty2) {
                            if t1 == t2 {
                                if let Some(comps) = self.get_record_components(t1) {
                                    for c in comps {
                                        let lhs_c = Expression::Variable(format!("{}_{}", n1, c));
                                        let rhs_c = Expression::Variable(format!("{}_{}", n2, c));
                                        target.equations.push(Equation::Simple(lhs_c, rhs_c));
                                    }
                                    continue;
                                }
                            }
                        }
                    }
                    target.equations.push(Equation::Simple(lhs_pre, rhs_pre));
                }
                Equation::MultiAssign(lhss, rhs) => {
                    let lhss_sub: Vec<Expression> = lhss
                        .iter()
                        .map(|e| self.substitute_stack(e, context_stack))
                        .collect();
                    let rhs_sub = self.substitute_stack(rhs, context_stack);
                    let lhss_pre: Vec<Expression> = lhss_sub
                        .iter()
                        .map(|e| prefix_expression(e, prefix))
                        .collect();
                    let rhs_pre = prefix_expression(&rhs_sub, prefix);
                    if let Expression::Call(name, args_pre) = &rhs_pre {
                        // Do not try to load builtin functions as models.
                        let is_builtin_like = |n: &str| {
                            matches!(n, "inStream" | "pow" | "sqrt" | "abs" | "min" | "max")
                                || n.starts_with("Modelica.Math.")
                                || n.starts_with("Medium.")
                                || n.starts_with("Internal.")
                                || n.contains(".Internal.")
                                || n.starts_with("Connections.")
                                || n.starts_with("Frames.")
                                || n.contains(".Frames.")
                                || n == "Utilities.regRoot"
                                || n.ends_with(".Utilities.regRoot")
                                || n == "Utilities.regRoot2"
                                || n.ends_with(".Utilities.regRoot2")
                                || n == "Utilities.regSquare2"
                                || n.ends_with(".Utilities.regSquare2")
                        };
                        if is_builtin_like(name) {
                            eprintln!(
                                "Warning: MultiAssign uses builtin-like function '{}'; not expanded.",
                                name
                            );
                            continue;
                        }
                        if let Ok(func_model) = self.loader.load_model(name) {
                            if let Some((input_names, outputs)) =
                                get_function_outputs(func_model.as_ref())
                            {
                                if input_names.len() == args_pre.len()
                                    && outputs.len() == lhss_pre.len()
                                {
                                    let mut subst = HashMap::new();
                                    for (i, in_name) in input_names.iter().enumerate() {
                                        if i < args_pre.len() {
                                            subst.insert(in_name.clone(), args_pre[i].clone());
                                        }
                                    }
                                    for (i, (_, out_expr)) in outputs.iter().enumerate() {
                                        if i < lhss_pre.len() {
                                            let sub = self.substitute(&out_expr, &subst);
                                            let sub_pre = prefix_expression(&sub, prefix);
                                            target.equations.push(Equation::Simple(
                                                lhss_pre[i].clone(),
                                                sub_pre,
                                            ));
                                        }
                                    }
                                    continue;
                                }
                            }
                        }
                    }
                    eprintln!("Warning: MultiAssign (a,b,...)=f(x) could not expand: RHS must be multi-output function call with matching output count.");
                }
                Equation::Connect(a_expr, b_expr) => {
                    let a_sub = self.substitute_stack(a_expr, context_stack);
                    let b_sub = self.substitute_stack(b_expr, context_stack);
                    let a_pre = prefix_expression(&a_sub, prefix);
                    let b_pre = prefix_expression(&b_sub, prefix);
                    if let (Some(a_path), Some(b_path)) =
                        (expr_to_path(&a_pre), expr_to_path(&b_pre))
                    {
                        if let Some(ref cond) = when_condition {
                            target
                                .conditional_connections
                                .push((cond.clone(), (a_path, b_path)));
                        } else {
                            target.connections.push((a_path, b_path));
                        }
                    } else {
                        eprintln!(
                            "Warning: Could not resolve connection path: {:?} - {:?}",
                            a_pre, b_pre
                        );
                    }
                }
                Equation::For(loop_var, start, end, body) => {
                    let start_sub = self.substitute_stack(start, context_stack);
                    let end_sub = self.substitute_stack(end, context_stack);
                    let start_val = eval_const_expr(&start_sub);
                    let end_val = eval_const_expr(&end_sub);

                    if start_val.is_none() || end_val.is_none() {
                        // Policy: non-const for bounds -> keep Equation::For into backend.
                        // JIT/analysis allocate loop var and referenced vars; do not expand here.
                        // Avoids panic on MSL patterns like for i in 1:size(A,1) loop ...
                        let mut temp_eqs = Vec::new();
                        let mut temp_alg = Vec::new();
                        let mut temp_conn = Vec::new();
                        let mut temp_cond_conn = Vec::new();
                        let mut temp_target = ExpandTarget {
                            equations: &mut temp_eqs,
                            algorithms: &mut temp_alg,
                            connections: &mut temp_conn,
                            conditional_connections: &mut temp_cond_conn,
                            array_sizes: target.array_sizes,
                        };
                        self.expand_equation_list(
                            body,
                            prefix,
                            &mut temp_target,
                            context_stack,
                            instances,
                            when_condition.clone(),
                        );
                        target.conditional_connections.extend(temp_cond_conn);
                        target.connections.extend(temp_conn);
                        target.equations.push(Equation::For(
                            loop_var.clone(),
                            Box::new(start_sub),
                            Box::new(end_sub),
                            temp_eqs,
                        ));
                        target.algorithms.extend(temp_alg);
                        return;
                    }

                    let s_int = start_val.unwrap() as i64;
                    let e_int = end_val.unwrap() as i64;
                    let count = e_int - s_int + 1;
                    // When loop range is large (>100), keep as single Equation::For for JIT to iterate;
                    // avoids huge expansion and stack depth during flatten. See TestLib/BigFor.mo.
                    if count > 100 {
                        let mut temp_eqs = Vec::new();
                        let mut temp_alg = Vec::new();
                        let mut temp_conn = Vec::new();
                        let mut temp_cond_conn = Vec::new();
                        let mut temp_target = ExpandTarget {
                            equations: &mut temp_eqs,
                            algorithms: &mut temp_alg,
                            connections: &mut temp_conn,
                            conditional_connections: &mut temp_cond_conn,
                            array_sizes: target.array_sizes,
                        };
                        self.expand_equation_list(
                            body,
                            prefix,
                            &mut temp_target,
                            context_stack,
                            instances,
                            when_condition.clone(),
                        );
                        target.conditional_connections.extend(temp_cond_conn);
                        target.equations.push(Equation::For(
                            loop_var.clone(),
                            Box::new(start_sub),
                            Box::new(end_sub),
                            temp_eqs,
                        ));
                        return;
                    }
                    for i in s_int..=e_int {
                        context_stack.push(HashMap::from_iter([(
                            loop_var.clone(),
                            Expression::Number(i as f64),
                        )]));
                        self.expand_equation_list(
                            body,
                            prefix,
                            target,
                            context_stack,
                            instances,
                            when_condition.clone(),
                        );
                        context_stack.pop();
                    }
                }
                Equation::When(cond, body, else_whens) => {
                    let cond_sub = self.substitute_stack(cond, context_stack);
                    let cond_pre = prefix_expression(&cond_sub, prefix);
                    let mut temp_eqs = Vec::new();
                    let mut temp_alg = Vec::new();
                    let mut temp_conn = Vec::new();
                    let mut temp_target = ExpandTarget {
                        equations: &mut temp_eqs,
                        algorithms: &mut temp_alg,
                        connections: &mut temp_conn,
                        conditional_connections: target.conditional_connections,
                        array_sizes: target.array_sizes,
                    };
                    self.expand_equation_list(
                        body,
                        prefix,
                        &mut temp_target,
                        context_stack,
                        instances,
                        Some(cond_pre.clone()),
                    );
                    let mut final_body: Vec<AlgorithmStatement> =
                        temp_eqs.into_iter().map(convert_eq_to_alg).collect();
                    final_body.extend(temp_alg);
                    let mut new_else_whens = Vec::new();
                    for (c, s) in else_whens {
                        let c_sub = self.substitute_stack(c, context_stack);
                        let c_pre = prefix_expression(&c_sub, prefix);
                        let mut t_eqs = Vec::new();
                        let mut t_alg = Vec::new();
                        let mut t_conn = Vec::new();
                        let mut t_target = ExpandTarget {
                            equations: &mut t_eqs,
                            algorithms: &mut t_alg,
                            connections: &mut t_conn,
                            conditional_connections: target.conditional_connections,
                            array_sizes: target.array_sizes,
                        };
                        self.expand_equation_list(
                            s,
                            prefix,
                            &mut t_target,
                            context_stack,
                            instances,
                            Some(c_pre),
                        );
                        let mut t_alg_body: Vec<AlgorithmStatement> =
                            t_eqs.into_iter().map(convert_eq_to_alg).collect();
                        t_alg_body.extend(t_alg);
                        new_else_whens.push((prefix_expression(&c_sub, prefix), t_alg_body));
                    }
                    target.algorithms.push(AlgorithmStatement::When(
                        prefix_expression(&cond_sub, prefix),
                        final_body,
                        new_else_whens,
                    ));
                }
                Equation::Reinit(var, val) => {
                    let val_sub = self.substitute_stack(val, context_stack);
                    let var_pre = if prefix.is_empty() {
                        var.clone()
                    } else {
                        format!("{}_{}", prefix, var)
                    };
                    let var_flat = var_pre.replace('.', "_");
                    target.algorithms.push(AlgorithmStatement::Reinit(
                        var_flat,
                        prefix_expression(&val_sub, prefix),
                    ));
                }
                Equation::Assert(cond, msg) => {
                    let cond_sub = self.substitute_stack(cond, context_stack);
                    let msg_sub = self.substitute_stack(msg, context_stack);
                    target.algorithms.push(AlgorithmStatement::Assert(
                        prefix_expression(&cond_sub, prefix),
                        prefix_expression(&msg_sub, prefix),
                    ));
                }
                Equation::Terminate(msg) => {
                    let msg_sub = self.substitute_stack(msg, context_stack);
                    target
                        .algorithms
                        .push(AlgorithmStatement::Terminate(prefix_expression(
                            &msg_sub, prefix,
                        )));
                }
                Equation::If(cond, then_eqs, elseif_list, else_eqs) => {
                    let cond_sub = self.substitute_stack(cond, context_stack);
                    let mut temp_then = Vec::new();
                    let mut temp_alg = Vec::new();
                    let mut temp_conn = Vec::new();
                    let mut then_target = ExpandTarget {
                        equations: &mut temp_then,
                        algorithms: &mut temp_alg,
                        connections: &mut temp_conn,
                        conditional_connections: target.conditional_connections,
                        array_sizes: target.array_sizes,
                    };
                    self.expand_equation_list(
                        then_eqs,
                        prefix,
                        &mut then_target,
                        context_stack,
                        instances,
                        when_condition.clone(),
                    );
                    let then_flat = then_target.equations.drain(..).collect();
                    let mut new_elseif = Vec::new();
                    for (c, eb) in elseif_list {
                        let c_sub = self.substitute_stack(c, context_stack);
                        let mut t_eqs = Vec::new();
                        let mut t_alg = Vec::new();
                        let mut t_conn = Vec::new();
                        let mut t_target = ExpandTarget {
                            equations: &mut t_eqs,
                            algorithms: &mut t_alg,
                            connections: &mut t_conn,
                            conditional_connections: target.conditional_connections,
                            array_sizes: target.array_sizes,
                        };
                        self.expand_equation_list(
                            eb,
                            prefix,
                            &mut t_target,
                            context_stack,
                            instances,
                            when_condition.clone(),
                        );
                        new_elseif.push((prefix_expression(&c_sub, prefix), t_eqs));
                    }
                    let else_flat = else_eqs.as_ref().map(|eqs| {
                        let mut t_eqs = Vec::new();
                        let mut t_alg = Vec::new();
                        let mut t_conn = Vec::new();
                        let mut t_target = ExpandTarget {
                            equations: &mut t_eqs,
                            algorithms: &mut t_alg,
                            connections: &mut t_conn,
                            conditional_connections: target.conditional_connections,
                            array_sizes: target.array_sizes,
                        };
                        self.expand_equation_list(
                            eqs,
                            prefix,
                            &mut t_target,
                            context_stack,
                            instances,
                            when_condition.clone(),
                        );
                        t_eqs
                    });
                    target.equations.push(Equation::If(
                        prefix_expression(&cond_sub, prefix),
                        then_flat,
                        new_elseif,
                        else_flat,
                    ));
                }
                Equation::SolvableBlock { .. } => {
                    panic!("SolvableBlock should not appear during expansion phase")
                }
            }
        }
    }

    pub(crate) fn expand_algorithm_list(
        &mut self,
        algorithms: &[AlgorithmStatement],
        prefix: &str,
        target: &mut ExpandTarget,
        context_stack: &mut Vec<HashMap<String, Expression>>,
    ) {
        for stmt in algorithms {
            match stmt {
                AlgorithmStatement::Assignment(lhs, rhs) => {
                    let lhs_sub = self.substitute_stack(lhs, context_stack);
                    let rhs_sub = self.substitute_stack(rhs, context_stack);
                    target.algorithms.push(AlgorithmStatement::Assignment(
                        prefix_expression(&lhs_sub, prefix),
                        prefix_expression(&rhs_sub, prefix),
                    ));
                }
                AlgorithmStatement::MultiAssign(lhss, rhs) => {
                    let lhss_sub: Vec<Expression> = lhss
                        .iter()
                        .map(|e| self.substitute_stack(e, context_stack))
                        .collect();
                    let rhs_sub = self.substitute_stack(rhs, context_stack);
                    let lhss_pre: Vec<Expression> = lhss_sub
                        .iter()
                        .map(|e| prefix_expression(e, prefix))
                        .collect();
                    let rhs_pre = prefix_expression(&rhs_sub, prefix);
                    target
                        .algorithms
                        .push(AlgorithmStatement::MultiAssign(lhss_pre, rhs_pre));
                }
                AlgorithmStatement::If(cond, true_stmts, else_ifs, else_stmts) => {
                    let cond_sub = self.substitute_stack(cond, context_stack);
                    let mut temp_eqs = Vec::new();
                    let mut temp_alg = Vec::new();
                    let mut temp_conn = Vec::new();
                    let mut temp_target = ExpandTarget {
                        equations: &mut temp_eqs,
                        algorithms: &mut temp_alg,
                        connections: &mut temp_conn,
                        conditional_connections: target.conditional_connections,
                        array_sizes: target.array_sizes,
                    };
                    self.expand_algorithm_list(true_stmts, prefix, &mut temp_target, context_stack);
                    let new_true = temp_alg;
                    let mut new_else_ifs = Vec::new();
                    for (c, s) in else_ifs {
                        let c_sub = self.substitute_stack(c, context_stack);
                        let mut t_eqs = Vec::new();
                        let mut t_alg = Vec::new();
                        let mut t_conn = Vec::new();
                        let mut t_target = ExpandTarget {
                            equations: &mut t_eqs,
                            algorithms: &mut t_alg,
                            connections: &mut t_conn,
                            conditional_connections: target.conditional_connections,
                            array_sizes: target.array_sizes,
                        };
                        self.expand_algorithm_list(s, prefix, &mut t_target, context_stack);
                        new_else_ifs.push((prefix_expression(&c_sub, prefix), t_alg));
                    }
                    let mut new_else = None;
                    if let Some(s) = else_stmts {
                        let mut t_eqs = Vec::new();
                        let mut t_alg = Vec::new();
                        let mut t_conn = Vec::new();
                        let mut t_target = ExpandTarget {
                            equations: &mut t_eqs,
                            algorithms: &mut t_alg,
                            connections: &mut t_conn,
                            conditional_connections: target.conditional_connections,
                            array_sizes: target.array_sizes,
                        };
                        self.expand_algorithm_list(s, prefix, &mut t_target, context_stack);
                        new_else = Some(t_alg);
                    }
                    target.algorithms.push(AlgorithmStatement::If(
                        prefix_expression(&cond_sub, prefix),
                        new_true,
                        new_else_ifs,
                        new_else,
                    ));
                }
                AlgorithmStatement::While(cond, body) => {
                    let cond_sub = self.substitute_stack(cond, context_stack);
                    let mut temp_eqs = Vec::new();
                    let mut temp_alg = Vec::new();
                    let mut temp_conn = Vec::new();
                    let mut temp_target = ExpandTarget {
                        equations: &mut temp_eqs,
                        algorithms: &mut temp_alg,
                        connections: &mut temp_conn,
                        conditional_connections: target.conditional_connections,
                        array_sizes: target.array_sizes,
                    };
                    self.expand_algorithm_list(body, prefix, &mut temp_target, context_stack);
                    target.algorithms.push(AlgorithmStatement::While(
                        prefix_expression(&cond_sub, prefix),
                        temp_alg,
                    ));
                }
                AlgorithmStatement::For(var_name, range, body) => {
                    let range_sub = self.substitute_stack(range, context_stack);
                    let mut temp_eqs = Vec::new();
                    let mut temp_alg = Vec::new();
                    let mut temp_conn = Vec::new();
                    let mut temp_target = ExpandTarget {
                        equations: &mut temp_eqs,
                        algorithms: &mut temp_alg,
                        connections: &mut temp_conn,
                        conditional_connections: target.conditional_connections,
                        array_sizes: target.array_sizes,
                    };
                    self.expand_algorithm_list(body, prefix, &mut temp_target, context_stack);
                    target.algorithms.push(AlgorithmStatement::For(
                        var_name.clone(),
                        Box::new(prefix_expression(&range_sub, prefix)),
                        temp_alg,
                    ));
                }
                AlgorithmStatement::When(cond, body, else_whens) => {
                    let cond_sub = self.substitute_stack(cond, context_stack);
                    let mut temp_eqs = Vec::new();
                    let mut temp_alg = Vec::new();
                    let mut temp_conn = Vec::new();
                    let mut temp_target = ExpandTarget {
                        equations: &mut temp_eqs,
                        algorithms: &mut temp_alg,
                        connections: &mut temp_conn,
                        conditional_connections: target.conditional_connections,
                        array_sizes: target.array_sizes,
                    };
                    self.expand_algorithm_list(body, prefix, &mut temp_target, context_stack);
                    let new_body = temp_alg;
                    let mut new_else_whens = Vec::new();
                    for (c, s) in else_whens {
                        let c_sub = self.substitute_stack(c, context_stack);
                        let mut t_eqs = Vec::new();
                        let mut t_alg = Vec::new();
                        let mut t_conn = Vec::new();
                        let mut t_target = ExpandTarget {
                            equations: &mut t_eqs,
                            algorithms: &mut t_alg,
                            connections: &mut t_conn,
                            conditional_connections: target.conditional_connections,
                            array_sizes: target.array_sizes,
                        };
                        self.expand_algorithm_list(s, prefix, &mut t_target, context_stack);
                        new_else_whens.push((prefix_expression(&c_sub, prefix), t_alg));
                    }
                    target.algorithms.push(AlgorithmStatement::When(
                        prefix_expression(&cond_sub, prefix),
                        new_body,
                        new_else_whens,
                    ));
                }
                AlgorithmStatement::Reinit(var, val) => {
                    let val_sub = self.substitute_stack(val, context_stack);
                    let var_pre = if prefix.is_empty() {
                        var.clone()
                    } else {
                        format!("{}_{}", prefix, var)
                    };
                    let var_flat = var_pre.replace('.', "_");
                    target.algorithms.push(AlgorithmStatement::Reinit(
                        var_flat,
                        prefix_expression(&val_sub, prefix),
                    ));
                }
                AlgorithmStatement::Assert(cond, msg) => {
                    let cond_sub = self.substitute_stack(cond, context_stack);
                    let msg_sub = self.substitute_stack(msg, context_stack);
                    target.algorithms.push(AlgorithmStatement::Assert(
                        prefix_expression(&cond_sub, prefix),
                        prefix_expression(&msg_sub, prefix),
                    ));
                }
                AlgorithmStatement::Terminate(msg) => {
                    let msg_sub = self.substitute_stack(msg, context_stack);
                    target
                        .algorithms
                        .push(AlgorithmStatement::Terminate(prefix_expression(
                            &msg_sub, prefix,
                        )));
                }
                AlgorithmStatement::CallStmt(expr) => {
                    let sub = self.substitute_stack(expr, context_stack);
                    target
                        .algorithms
                        .push(AlgorithmStatement::CallStmt(prefix_expression(&sub, prefix)));
                }
                AlgorithmStatement::NoOp => {}
            }
        }
    }
}
