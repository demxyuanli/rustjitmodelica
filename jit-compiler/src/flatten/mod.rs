use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use crate::ast::{Expression, Equation, Declaration, Model, AlgorithmStatement};
use crate::diag::SourceLocation;
use crate::loader::{ModelLoader, LoadError};

#[derive(Debug)]
pub enum FlattenError {
    Load(LoadError),
    UnknownType(String, String, Option<SourceLocation>),
    IncompatibleConnector(String, String, String, String, Option<SourceLocation>),
}

impl From<LoadError> for FlattenError {
    fn from(e: LoadError) -> Self {
        FlattenError::Load(e)
    }
}

impl fmt::Display for FlattenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlattenError::Load(e) => write!(f, "{}", e),
            FlattenError::UnknownType(ty, inst, loc) => {
                write!(f, "Unknown type '{}' for instance '{}'", ty, inst)?;
                if let Some(ref l) = loc {
                    write!(f, "{}", l.fmt_suffix())?;
                }
                Ok(())
            }
            FlattenError::IncompatibleConnector(a, b, ta, tb, loc) => {
                write!(f, "Error: Incompatible connector types in connect({}, {}): type '{}' vs '{}' (model/connector paths as shown)", a, b, ta, tb)?;
                if let Some(ref l) = loc {
                    write!(f, "{}", l.fmt_suffix())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for FlattenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FlattenError::Load(e) => Some(e),
            _ => None,
        }
    }
}

pub mod structures;
pub mod expressions;
pub mod utils;
pub mod connections;
mod substitute;
mod expand;

pub use self::structures::FlattenedModel;
#[allow(unused_imports)]
pub use self::expressions::{prefix_expression, index_expression, eval_const_expr, expr_to_path};
use self::utils::{is_primitive, resolve_type_alias, apply_modification, merge_models};
use self::connections::resolve_connections;

pub(crate) struct ExpandTarget<'a> {
    pub equations: &'a mut Vec<Equation>,
    pub algorithms: &'a mut Vec<AlgorithmStatement>,
    pub connections: &'a mut Vec<(String, String)>,
    pub conditional_connections: &'a mut Vec<(Expression, (String, String))>,
    pub array_sizes: &'a HashMap<String, usize>,
}

pub struct Flattener {
    pub loader: ModelLoader,
}

impl Flattener {
    pub fn new() -> Self {
        Flattener { loader: ModelLoader::new() }
    }
    
    /// root_name: model name used to load root (e.g. "TestLib/InitDummy") for DBG-4 source location in errors.
    pub fn flatten(&mut self, root: &mut Arc<Model>, root_name: &str) -> Result<FlattenedModel, FlattenError> {
        self.flatten_inheritance(root)?;
        let model = Arc::make_mut(root);
        let mut flat = FlattenedModel {
            declarations: Vec::new(),
            equations: Vec::new(),
            algorithms: Vec::new(),
            initial_equations: Vec::new(),
            initial_algorithms: Vec::new(),
            connections: Vec::new(),
            conditional_connections: Vec::new(),
            instances: HashMap::new(),
            array_sizes: HashMap::new(),
            clocked_var_names: std::collections::HashSet::new(),
            clock_partitions: Vec::new(),
        };
        self.expand_declarations(model, "", &mut flat, Some(root_name))?;
        self.expand_equations(model, "", &mut flat);
        self.expand_algorithms(model, "", &mut flat);
        self.expand_initial_equations(model, "", &mut flat);
        self.expand_initial_algorithms(model, "", &mut flat);
        resolve_connections(&mut flat, Some(root_name), &self.loader)?;
        self.infer_clocked_variables(&mut flat);
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

    fn expand_declarations(&mut self, model: &Model, prefix: &str, flat: &mut FlattenedModel, current_model_name: Option<&str>) -> Result<(), FlattenError> {
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
                
                let loc = current_model_name
                    .and_then(|n| self.loader.get_path_for_model(n))
                    .map(|p| SourceLocation { file: p.display().to_string(), line: 0, column: 0 });
                let resolved_type = resolve_type_alias(&model.type_aliases, &decl.type_name);
                if is_primitive(&resolved_type) {
                    flat.declarations.push(Declaration {
                        type_name: resolved_type.clone(),
                        name: full_path.clone(),
                        replaceable: decl.replaceable,
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
                        is_rest: decl.is_rest,
                        annotation: None,
                    });
                } else {
                    let mut sub_model = self.loader.load_model(&resolved_type)
                        .map_err(|_| FlattenError::UnknownType(resolved_type.clone(), full_path.clone(), loc))?;
                    self.flatten_inheritance(&mut sub_model)?;
                    for modification in &decl.modifications {
                        apply_modification(Arc::make_mut(&mut sub_model), modification);
                    }
                    flat.instances.insert(full_path.clone(), resolved_type.clone());
                    self.expand_declarations(sub_model.as_ref(), &full_path, flat, Some(resolved_type.as_str()))?;
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
        let instances = flat.instances.clone();
        let mut target = ExpandTarget {
            equations: &mut flat.equations,
            algorithms: &mut flat.algorithms,
            connections: &mut flat.connections,
            conditional_connections: &mut flat.conditional_connections,
            array_sizes: &flat.array_sizes,
        };
        self.expand_equation_list(&model.equations, prefix, &mut target, &mut context_stack, &instances, None);
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
        let instances = flat.instances.clone();
        let mut target = ExpandTarget {
            equations: &mut flat.initial_equations,
            algorithms: &mut flat.initial_algorithms,
            connections: &mut flat.connections,
            conditional_connections: &mut flat.conditional_connections,
            array_sizes: &flat.array_sizes,
        };
        self.expand_equation_list(&model.initial_equations, prefix, &mut target, &mut context_stack, &instances, None);
    }

    fn get_record_components(&mut self, type_name: &str) -> Option<Vec<String>> {
        let m = self.loader.load_model(type_name).ok()?;
        if m.is_record {
            Some(m.declarations.iter().map(|d| d.name.clone()).collect())
        } else {
            None
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
            conditional_connections: &mut flat.conditional_connections,
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
            conditional_connections: &mut flat.conditional_connections,
            array_sizes: &flat.array_sizes,
        };
        self.expand_algorithm_list(&model.initial_algorithms, prefix, &mut target, &mut context_stack);
    }

    fn infer_clocked_variables(&self, flat: &mut FlattenedModel) {
        fn expr_contains_clock(e: &Expression) -> bool {
            match e {
                Expression::Sample(inner)
                | Expression::Interval(inner)
                | Expression::Hold(inner)
                | Expression::Previous(inner) => expr_contains_clock(inner),
                Expression::SubSample(c, n)
                | Expression::SuperSample(c, n)
                | Expression::ShiftSample(c, n) => expr_contains_clock(c) || expr_contains_clock(n),
                Expression::BinaryOp(l, _, r) => expr_contains_clock(l) || expr_contains_clock(r),
                Expression::Call(_, args) => args.iter().any(expr_contains_clock),
                Expression::ArrayAccess(base, idx) => expr_contains_clock(base) || expr_contains_clock(idx),
                Expression::Dot(base, _) => expr_contains_clock(base),
                Expression::If(c, t, f) => expr_contains_clock(c) || expr_contains_clock(t) || expr_contains_clock(f),
                Expression::Range(a, b, c) => expr_contains_clock(a) || expr_contains_clock(b) || expr_contains_clock(c),
                Expression::ArrayLiteral(items) => items.iter().any(expr_contains_clock),
                _ => false,
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

        fn walk_algorithms(stmts: &[AlgorithmStatement], clocked: bool, out: &mut std::collections::HashSet<String>) {
            for stmt in stmts {
                match stmt {
                    AlgorithmStatement::Assignment(lhs, _) => {
                        if clocked {
                            collect_lhs_vars(lhs, out);
                        }
                    }
                    AlgorithmStatement::If(_, then_stmts, else_ifs, else_stmts) => {
                        walk_algorithms(then_stmts, clocked, out);
                        for (_, s) in else_ifs {
                            walk_algorithms(s, clocked, out);
                        }
                        if let Some(s) = else_stmts {
                            walk_algorithms(s, clocked, out);
                        }
                    }
                    AlgorithmStatement::While(_, body) => {
                        walk_algorithms(body, clocked, out);
                    }
                    AlgorithmStatement::For(_, _, body) => {
                        walk_algorithms(body, clocked, out);
                    }
                    AlgorithmStatement::When(cond, body, else_whens) => {
                        let is_clock = expr_contains_clock(cond);
                        let new_clocked = clocked || is_clock;
                        walk_algorithms(body, new_clocked, out);
                        for (c, s) in else_whens {
                            let else_clocked = clocked || expr_contains_clock(c);
                            walk_algorithms(s, else_clocked, out);
                        }
                    }
                    AlgorithmStatement::Reinit(var, _) => {
                        if clocked {
                            out.insert(var.clone());
                        }
                    }
                    AlgorithmStatement::Assert(_, _) | AlgorithmStatement::Terminate(_) => {}
                }
            }
        }

        let mut clocked = std::collections::HashSet::new();
        walk_algorithms(&flat.algorithms, false, &mut clocked);
        walk_algorithms(&flat.initial_algorithms, false, &mut clocked);
        flat.clocked_var_names = clocked.clone();
        if !clocked.is_empty() {
            flat.clock_partitions.push(self::structures::ClockPartition {
                id: "default".to_string(),
                var_names: clocked,
            });
        }
    }

}
