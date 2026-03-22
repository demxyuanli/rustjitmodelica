use crate::ast::{AlgorithmStatement, Equation, Expression, Model, StringInterner};
use crate::loader::ModelLoader;
use std::collections::HashMap;
use std::sync::Arc;
mod error;
mod decl_expand;
mod import_resolve;
mod inheritance;
mod record;
pub use self::error::FlattenError;

pub mod connections;
mod expand;
pub mod expressions;
pub mod structures;
mod substitute;
pub mod utils;
mod clock_infer;

use self::connections::resolve_connections;
#[allow(unused_imports)]
pub use self::expressions::{
    eval_const_expr, eval_const_expr_with_array_sizes, eval_const_expr_with_params, expr_to_path,
    index_expression, prefix_expression,
};
pub use self::structures::FlattenedModel;

pub(crate) struct ExpandTarget<'a> {
    pub equations: &'a mut Vec<Equation>,
    pub algorithms: &'a mut Vec<AlgorithmStatement>,
    pub connections: &'a mut Vec<(String, String)>,
    pub conditional_connections: &'a mut Vec<(Expression, (String, String))>,
    pub array_sizes: &'a HashMap<String, usize>,
}

pub struct Flattener {
    pub loader: ModelLoader,
    pub name_cache: crate::string_intern::StringInterner,
}

impl Flattener {

    pub fn new() -> Self {
        Flattener {
            loader: ModelLoader::new(),
            name_cache: crate::string_intern::StringInterner::new(),
        }
    }

    /// root_name: model name used to load root (e.g. "TestLib/InitDummy") for DBG-4 source location in errors.
    pub fn flatten(
        &mut self,
        root: &mut Arc<Model>,
        root_name: &str,
    ) -> Result<FlattenedModel, FlattenError> {
        let root_path = root_name.replace('/', ".");
        self.flatten_inheritance(root, root_path.as_str())?;
        let model = root.as_ref();
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
            clock_signal_connections: Vec::new(),
            interner: StringInterner::new(),
        };
        self.expand_declarations(Arc::clone(root), "", &mut flat, Some(root_path.as_str()))?;
        self.expand_equations(model, "", &mut flat);
        self.expand_algorithms(model, "", &mut flat);
        self.expand_initial_equations(model, "", &mut flat);
        self.expand_initial_algorithms(model, "", &mut flat);
        resolve_connections(&mut flat, Some(root_path.as_str()), &self.loader)?;
        self.infer_clocked_variables(&mut flat);
        Ok(flat)
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
        self.expand_equation_list(
            &model.equations,
            prefix,
            &mut target,
            &mut context_stack,
            &instances,
            None,
        );
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
        self.expand_equation_list(
            &model.initial_equations,
            prefix,
            &mut target,
            &mut context_stack,
            &instances,
            None,
        );
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

    fn expand_initial_algorithms(
        &mut self,
        model: &Model,
        prefix: &str,
        flat: &mut FlattenedModel,
    ) {
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
        self.expand_algorithm_list(
            &model.initial_algorithms,
            prefix,
            &mut target,
            &mut context_stack,
        );
    }

}

