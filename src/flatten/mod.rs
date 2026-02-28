use std::collections::HashMap;
use std::sync::Arc;
use crate::ast::{Expression, Equation, Declaration, Model, AlgorithmStatement};
use crate::loader::{ModelLoader, LoadError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlattenError {
    #[error("{0}")]
    Load(#[from] LoadError),
    #[error("Unknown type '{0}' for instance '{1}'")]
    UnknownType(String, String),
    #[error("Error: Incompatible connector types in connect({0}, {1}): type '{2}' vs '{3}' (model/connector paths as shown)")]
    IncompatibleConnector(String, String, String, String),
}

pub mod structures;
pub mod expressions;
pub mod utils;
pub mod connections;

pub use self::structures::FlattenedModel;
pub use self::expressions::{prefix_expression, index_expression, eval_const_expr, expr_to_path};
use self::utils::{is_primitive, apply_modification, merge_models, convert_eq_to_alg};
use self::connections::resolve_connections;

pub(crate) struct ExpandTarget<'a> {
    pub equations: &'a mut Vec<Equation>,
    pub algorithms: &'a mut Vec<AlgorithmStatement>,
    pub connections: &'a mut Vec<(String, String)>,
    pub array_sizes: &'a HashMap<String, usize>,
}

pub struct Flattener {
    pub loader: ModelLoader,
}

impl Flattener {
    pub fn new() -> Self {
        Flattener { loader: ModelLoader::new() }
    }
    
    pub fn flatten(&mut self, root: &mut Arc<Model>) -> Result<FlattenedModel, FlattenError> {
        self.flatten_inheritance(root)?;
        let model = Arc::make_mut(root);
        let mut flat = FlattenedModel {
            declarations: Vec::new(),
            equations: Vec::new(),
            algorithms: Vec::new(),
            initial_equations: Vec::new(),
            initial_algorithms: Vec::new(),
            connections: Vec::new(),
            instances: HashMap::new(),
            array_sizes: HashMap::new(),
        };
        self.expand_declarations(model, "", &mut flat)?;
        self.expand_equations(model, "", &mut flat);
        self.expand_algorithms(model, "", &mut flat);
        self.expand_initial_equations(model, "", &mut flat);
        self.expand_initial_algorithms(model, "", &mut flat);
        resolve_connections(&mut flat)?;
        Ok(flat)
    }

    fn flatten_inheritance(&mut self, arc: &mut Arc<Model>) -> Result<(), FlattenError> {
        let model = Arc::make_mut(arc);
        let extends = std::mem::take(&mut model.extends);
        for clause in extends {
            let base_name = clause.model_name.clone();
            let mut base_model = self.loader.load_model(&base_name)?;
            self.flatten_inheritance(&mut base_model)?;
            for modification in &clause.modifications {
                apply_modification(Arc::make_mut(&mut base_model), modification);
            }
            merge_models(model, base_model.as_ref());
        }
        Ok(())
    }

    fn expand_declarations(&mut self, model: &Model, prefix: &str, flat: &mut FlattenedModel) -> Result<(), FlattenError> {
        // Build context from parameters in this model
        let mut context: HashMap<String, Expression> = HashMap::new();
        for decl in &model.declarations {
            if decl.is_parameter {
                if let Some(val) = &decl.start_value {
                    context.insert(decl.name.clone(), val.clone());
                }
            }
        }

        for decl in &model.declarations {
            // Evaluate array size
            let array_len = if let Some(size_expr) = &decl.array_size {
                let sub_expr = self.substitute(size_expr, &context);
                if let Some(val) = eval_const_expr(&sub_expr) {
                    Some(val as usize)
                } else {
                    eprintln!("Warning: Could not evaluate array size for '{}'", decl.name);
                    None
                }
            } else {
                None
            };
            
            let count = array_len.unwrap_or(1);
            let is_array = array_len.is_some();
            
            let base_name = if prefix.is_empty() { decl.name.clone() } else { format!("{}_{}", prefix, decl.name) };
            
            if is_array {
                flat.array_sizes.insert(base_name.clone(), count);
            }

            for i in 1..=count {
                let name_suffix = if is_array { format!("_{}", i) } else { "".to_string() };
                let local_name = format!("{}{}", decl.name, name_suffix);
                let full_path = if prefix.is_empty() { local_name.clone() } else { format!("{}_{}", prefix, local_name) };
                
                if is_primitive(&decl.type_name) {
                    flat.declarations.push(Declaration {
                        type_name: decl.type_name.clone(),
                        name: full_path.clone(),
                        is_parameter: decl.is_parameter,
                        is_flow: decl.is_flow,
                        is_discrete: decl.is_discrete,
                        is_input: decl.is_input,
                        is_output: decl.is_output,
                        start_value: if let Some(val) = &decl.start_value {
                            let sub = self.substitute(val, &context);
                            if is_array {
                                Some(index_expression(&sub, i))
                            } else {
                                Some(sub)
                            }
                        } else {
                            None
                        },
                        array_size: None,
                        modifications: Vec::new(),
                        annotation: None,
                    });
                } else {
                    let mut sub_model = self.loader.load_model(&decl.type_name)
                        .map_err(|_| FlattenError::UnknownType(decl.type_name.clone(), full_path.clone()))?;
                    self.flatten_inheritance(&mut sub_model)?;
                    for modification in &decl.modifications {
                        apply_modification(Arc::make_mut(&mut sub_model), modification);
                    }
                    flat.instances.insert(full_path.clone(), decl.type_name.clone());
                    self.expand_declarations(sub_model.as_ref(), &full_path, flat)?;
                    self.expand_equations(sub_model.as_ref(), &full_path, flat);
                }
            }
        }
        Ok(())
    }

    fn expand_equations(&mut self, model: &Model, prefix: &str, flat: &mut FlattenedModel) {
        let mut context: HashMap<String, Expression> = HashMap::new();
        for decl in &model.declarations {
            if decl.is_parameter {
                if let Some(val) = &decl.start_value {
                    context.insert(decl.name.clone(), val.clone());
                }
            }
        }
        let mut context_stack = vec![context];
        let mut target = ExpandTarget {
            equations: &mut flat.equations,
            algorithms: &mut flat.algorithms,
            connections: &mut flat.connections,
            array_sizes: &flat.array_sizes,
        };
        self.expand_equation_list(&model.equations, prefix, &mut target, &mut context_stack);
    }

    fn expand_initial_equations(&mut self, model: &Model, prefix: &str, flat: &mut FlattenedModel) {
        let mut context: HashMap<String, Expression> = HashMap::new();
        for decl in &model.declarations {
            if decl.is_parameter {
                if let Some(val) = &decl.start_value {
                    context.insert(decl.name.clone(), val.clone());
                }
            }
        }
        let mut context_stack = vec![context];
        let mut target = ExpandTarget {
            equations: &mut flat.initial_equations,
            algorithms: &mut flat.initial_algorithms,
            connections: &mut flat.connections,
            array_sizes: &flat.array_sizes,
        };
        self.expand_equation_list(&model.initial_equations, prefix, &mut target, &mut context_stack);
    }

    fn expand_equation_list(
        &mut self,
        equations: &[Equation],
        prefix: &str,
        target: &mut ExpandTarget,
        context_stack: &mut Vec<HashMap<String, Expression>>,
    ) {
        for eq in equations {
            match eq {
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
                                    let lhs_i = Expression::Der(Box::new(index_expression(&**arg, i)));
                                    let rhs_i = index_expression(&rhs_pre, i);
                                    let lhs_flat = prefix_expression(&lhs_i, "");
                                    let rhs_flat = prefix_expression(&rhs_i, "");
                                    target.equations.push(Equation::Simple(lhs_flat, rhs_flat));
                                }
                                continue;
                            }
                        }
                    }
                    target.equations.push(Equation::Simple(lhs_pre, rhs_pre));
                }
                Equation::Connect(a_expr, b_expr) => {
                    let a_sub = self.substitute_stack(a_expr, context_stack);
                    let b_sub = self.substitute_stack(b_expr, context_stack);
                    let a_pre = prefix_expression(&a_sub, prefix);
                    let b_pre = prefix_expression(&b_sub, prefix);
                    if let (Some(a_path), Some(b_path)) = (expr_to_path(&a_pre), expr_to_path(&b_pre)) {
                        target.connections.push((a_path, b_path));
                    } else {
                        eprintln!("Warning: Could not resolve connection path: {:?} - {:?}", a_pre, b_pre);
                    }
                }
                Equation::For(loop_var, start, end, body) => {
                    let start_sub = self.substitute_stack(start, context_stack);
                    let end_sub = self.substitute_stack(end, context_stack);
                    let start_val = eval_const_expr(&start_sub).expect("For-loop start must be constant");
                    let end_val = eval_const_expr(&end_sub).expect("For-loop end must be constant");
                    let s_int = start_val as i64;
                    let e_int = end_val as i64;
                    let count = e_int - s_int + 1;
                    // When loop range is large (>100), keep as single Equation::For for JIT to iterate;
                    // avoids huge expansion and stack depth during flatten. See TestLib/BigFor.mo.
                    if count > 100 {
                        let mut temp_eqs = Vec::new();
                        let mut temp_alg = Vec::new();
                        let mut temp_conn = Vec::new();
                        let mut temp_target = ExpandTarget {
                            equations: &mut temp_eqs,
                            algorithms: &mut temp_alg,
                            connections: &mut temp_conn,
                            array_sizes: target.array_sizes,
                        };
                        self.expand_equation_list(body, prefix, &mut temp_target, context_stack);
                        target.equations.push(Equation::For(
                            loop_var.clone(),
                            Box::new(start_sub),
                            Box::new(end_sub),
                            temp_eqs
                        ));
                        return;
                    }
                    for i in s_int..=e_int {
                        context_stack.push(HashMap::from_iter([(loop_var.clone(), Expression::Number(i as f64))]));
                        self.expand_equation_list(body, prefix, target, context_stack);
                        context_stack.pop();
                    }
                }
                Equation::When(cond, body, else_whens) => {
                    let cond_sub = self.substitute_stack(cond, context_stack);
                    let mut temp_eqs = Vec::new();
                    let mut temp_alg = Vec::new();
                    let mut temp_conn = Vec::new();
                    let mut temp_target = ExpandTarget {
                        equations: &mut temp_eqs,
                        algorithms: &mut temp_alg,
                        connections: &mut temp_conn,
                        array_sizes: target.array_sizes,
                    };
                    self.expand_equation_list(body, prefix, &mut temp_target, context_stack);
                    let mut final_body: Vec<AlgorithmStatement> = temp_eqs.into_iter().map(convert_eq_to_alg).collect();
                    final_body.extend(temp_alg);
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
                            array_sizes: target.array_sizes,
                        };
                        self.expand_equation_list(s, prefix, &mut t_target, context_stack);
                        let mut t_alg_body: Vec<AlgorithmStatement> = t_eqs.into_iter().map(convert_eq_to_alg).collect();
                        t_alg_body.extend(t_alg);
                        new_else_whens.push((prefix_expression(&c_sub, prefix), t_alg_body));
                    }
                    target.algorithms.push(AlgorithmStatement::When(
                        prefix_expression(&cond_sub, prefix),
                        final_body,
                        new_else_whens
                    ));
                }
                Equation::Reinit(var, val) => {
                    let val_sub = self.substitute_stack(val, context_stack);
                    let var_pre = if prefix.is_empty() { var.clone() } else { format!("{}_{}", prefix, var) };
                    let var_flat = var_pre.replace('.', "_");
                    target.algorithms.push(AlgorithmStatement::Reinit(
                        var_flat,
                        prefix_expression(&val_sub, prefix)
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
                    target.algorithms.push(AlgorithmStatement::Terminate(
                        prefix_expression(&msg_sub, prefix),
                    ));
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
                        array_sizes: target.array_sizes,
                    };
                    self.expand_equation_list(then_eqs, prefix, &mut then_target, context_stack);
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
                            array_sizes: target.array_sizes,
                        };
                        self.expand_equation_list(eb, prefix, &mut t_target, context_stack);
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
                            array_sizes: target.array_sizes,
                        };
                        self.expand_equation_list(eqs, prefix, &mut t_target, context_stack);
                        t_eqs
                    });
                    target.equations.push(Equation::If(
                        prefix_expression(&cond_sub, prefix),
                        then_flat,
                        new_elseif,
                        else_flat,
                    ));
                }
                Equation::SolvableBlock { .. } => panic!("SolvableBlock should not appear during expansion phase"),
            }
        }
    }

    fn expand_algorithms(&mut self, model: &Model, prefix: &str, flat: &mut FlattenedModel) {
        let mut context: HashMap<String, Expression> = HashMap::new();
        for decl in &model.declarations {
            if decl.is_parameter {
                if let Some(val) = &decl.start_value {
                    context.insert(decl.name.clone(), val.clone());
                }
            }
        }
        let mut context_stack = vec![context];
        let mut target = ExpandTarget {
            equations: &mut flat.equations,
            algorithms: &mut flat.algorithms,
            connections: &mut flat.connections,
            array_sizes: &flat.array_sizes,
        };
        self.expand_algorithm_list(&model.algorithms, prefix, &mut target, &mut context_stack);
    }

    fn expand_initial_algorithms(&mut self, model: &Model, prefix: &str, flat: &mut FlattenedModel) {
        let mut context: HashMap<String, Expression> = HashMap::new();
        for decl in &model.declarations {
            if decl.is_parameter {
                if let Some(val) = &decl.start_value {
                    context.insert(decl.name.clone(), val.clone());
                }
            }
        }
        let mut context_stack = vec![context];
        let mut target = ExpandTarget {
            equations: &mut flat.initial_equations,
            algorithms: &mut flat.initial_algorithms,
            connections: &mut flat.connections,
            array_sizes: &flat.array_sizes,
        };
        self.expand_algorithm_list(&model.initial_algorithms, prefix, &mut target, &mut context_stack);
    }

    fn expand_algorithm_list(
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
                        prefix_expression(&rhs_sub, prefix)
                    ));
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
                            array_sizes: target.array_sizes,
                        };
                        self.expand_algorithm_list(s, prefix, &mut t_target, context_stack);
                        new_else = Some(t_alg);
                    }
                    target.algorithms.push(AlgorithmStatement::If(
                        prefix_expression(&cond_sub, prefix),
                        new_true,
                        new_else_ifs,
                        new_else
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
                        array_sizes: target.array_sizes,
                    };
                    self.expand_algorithm_list(body, prefix, &mut temp_target, context_stack);
                    target.algorithms.push(AlgorithmStatement::While(
                        prefix_expression(&cond_sub, prefix),
                        temp_alg
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
                        array_sizes: target.array_sizes,
                    };
                    self.expand_algorithm_list(body, prefix, &mut temp_target, context_stack);
                    target.algorithms.push(AlgorithmStatement::For(
                        var_name.clone(),
                        Box::new(prefix_expression(&range_sub, prefix)),
                        temp_alg
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
                            array_sizes: target.array_sizes,
                        };
                        self.expand_algorithm_list(s, prefix, &mut t_target, context_stack);
                        new_else_whens.push((prefix_expression(&c_sub, prefix), t_alg));
                    }
                    target.algorithms.push(AlgorithmStatement::When(
                        prefix_expression(&cond_sub, prefix),
                        new_body,
                        new_else_whens
                    ));
                }
                AlgorithmStatement::Reinit(var, val) => {
                    let val_sub = self.substitute_stack(val, context_stack);
                    let var_pre = if prefix.is_empty() { var.clone() } else { format!("{}_{}", prefix, var) };
                    let var_flat = var_pre.replace('.', "_");
                    target.algorithms.push(AlgorithmStatement::Reinit(
                        var_flat,
                        prefix_expression(&val_sub, prefix)
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
                    target.algorithms.push(AlgorithmStatement::Terminate(
                        prefix_expression(&msg_sub, prefix),
                    ));
                }
            }
        }
    }

    fn lookup_context_stack(context_stack: &[HashMap<String, Expression>], name: &str) -> Option<Expression> {
        for map in context_stack.iter().rev() {
            if let Some(val) = map.get(name) {
                return Some(val.clone());
            }
        }
        None
    }

    fn substitute_stack(&mut self, expr: &Expression, context_stack: &[HashMap<String, Expression>]) -> Expression {
        match expr {
            Expression::Variable(name) => {
                if let Some(val) = Self::lookup_context_stack(context_stack, name) {
                    val
                } else {
                    expr.clone()
                }
            }
            Expression::Number(_) => expr.clone(),
            Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
                Box::new(self.substitute_stack(lhs, context_stack)),
                op.clone(),
                Box::new(self.substitute_stack(rhs, context_stack))
            ),
            Expression::Call(func, args) => Expression::Call(
                func.clone(),
                args.iter().map(|arg| self.substitute_stack(arg, context_stack)).collect()
            ),
            Expression::Der(arg) => Expression::Der(Box::new(self.substitute_stack(arg, context_stack))),
            Expression::ArrayAccess(arr, idx) => {
                 let new_arr = self.substitute_stack(arr, context_stack);
                 let new_idx = self.substitute_stack(idx, context_stack);
                 if let (Expression::Variable(name), Expression::Number(n)) = (&new_arr, &new_idx) {
                     let n_int = *n as i64;
                     Expression::Variable(format!("{}_{}", name, n_int))
                 } else if let (Expression::ArrayLiteral(elements), Expression::Number(n)) = (&new_arr, &new_idx) {
                     let idx = *n as usize;
                     if idx > 0 && idx <= elements.len() {
                         elements[idx - 1].clone()
                     } else {
                         eprintln!("Index out of bounds in substitution: {} (len {})", idx, elements.len());
                         Expression::Number(0.0)
                     }
                 } else {
                     Expression::ArrayAccess(Box::new(new_arr), Box::new(new_idx))
                 }
            }
            Expression::Dot(base, member) => {
                 let new_base = self.substitute_stack(base, context_stack);
                 if let Some(base_path) = expr_to_path(&new_base) {
                     let full_path = format!("{}.{}", base_path, member);
                     if let Some(val) = self.resolve_global_constant(&full_path) {
                         return val;
                     }
                 }
                 Expression::Dot(Box::new(new_base), member.clone())
            }
            Expression::If(cond, t_expr, f_expr) => Expression::If(
                Box::new(self.substitute_stack(cond, context_stack)),
                Box::new(self.substitute_stack(t_expr, context_stack)),
                Box::new(self.substitute_stack(f_expr, context_stack))
            ),
            Expression::Range(start, step, end) => Expression::Range(
                Box::new(self.substitute_stack(start, context_stack)),
                Box::new(self.substitute_stack(step, context_stack)),
                Box::new(self.substitute_stack(end, context_stack))
            ),
            Expression::ArrayLiteral(exprs) => {
                Expression::ArrayLiteral(exprs.iter().map(|e| self.substitute_stack(e, context_stack)).collect())
            }
        }
    }

    fn substitute(&mut self, expr: &Expression, context: &HashMap<String, Expression>) -> Expression {
        match expr {
            Expression::Variable(name) => {
                if let Some(val) = context.get(name) {
                    val.clone()
                } else {
                    expr.clone()
                }
            }
            Expression::Number(_) => expr.clone(),
            Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
                Box::new(self.substitute(lhs, context)),
                op.clone(),
                Box::new(self.substitute(rhs, context))
            ),
            Expression::Call(func, args) => Expression::Call(
                func.clone(),
                args.iter().map(|arg| self.substitute(arg, context)).collect()
            ),
            Expression::Der(arg) => {
                Expression::Der(Box::new(self.substitute(arg, context)))
            }
            Expression::ArrayAccess(arr, idx) => {
                 let new_arr = self.substitute(arr, context);
                 let new_idx = self.substitute(idx, context);
                 
                 if let (Expression::Variable(name), Expression::Number(n)) = (&new_arr, &new_idx) {
                     let n_int = *n as i64;
                     Expression::Variable(format!("{}_{}", name, n_int))
                 } else if let (Expression::ArrayLiteral(elements), Expression::Number(n)) = (&new_arr, &new_idx) {
                     let idx = *n as usize;
                     if idx > 0 && idx <= elements.len() {
                         elements[idx - 1].clone()
                     } else {
                         eprintln!("Index out of bounds in substitution: {} (len {})", idx, elements.len());
                         Expression::Number(0.0)
                     }
                 } else {
                     Expression::ArrayAccess(Box::new(new_arr), Box::new(new_idx))
                 }
            }
            Expression::Dot(base, member) => {
                 let new_base = self.substitute(base, context);
                 
                 // Try to resolve global constant
                 if let Some(base_path) = expr_to_path(&new_base) {
                     let full_path = format!("{}.{}", base_path, member);
                     if let Some(val) = self.resolve_global_constant(&full_path) {
                         return val;
                     }
                 }
                 
                 Expression::Dot(Box::new(new_base), member.clone())
            }
            Expression::If(cond, t_expr, f_expr) => Expression::If(
                Box::new(self.substitute(cond, context)),
                Box::new(self.substitute(t_expr, context)),
                Box::new(self.substitute(f_expr, context))
            ),
            Expression::Range(start, step, end) => Expression::Range(
                Box::new(self.substitute(start, context)),
                Box::new(self.substitute(step, context)),
                Box::new(self.substitute(end, context))
            ),
            Expression::ArrayLiteral(exprs) => {
                Expression::ArrayLiteral(exprs.iter().map(|e| self.substitute(e, context)).collect())
            }
        }
    }
    
    fn resolve_global_constant(&mut self, path: &str) -> Option<Expression> {
        if let Some((model_name, var_name)) = path.rsplit_once('.') {
            // Try loading model_name (silently)
            if let Ok(model) = self.loader.load_model_silent(model_name, true) {
                for decl in &model.declarations {
                    if decl.name == var_name {
                        // Found declaration. Check if it has a value.
                        if let Some(val) = &decl.start_value {
                            return Some(val.clone());
                        }
                    }
                }
            }
        }
        None
    }
}
