//! Flatten pipeline: inheritance, declaration expand, connections.
//! Out of scope here (future epics): full rustc-style query cache over defs, registry serde,
//! OMC instantiateModel parity tooling, parallel flatten.

use crate::ast::{AlgorithmStatement, Equation, Expression, Model, StringInterner};
use crate::loader::ModelLoader;
use std::collections::HashMap;
use std::sync::Arc;
mod error;
mod array_size_policy;
pub(crate) mod flatten_cache;
mod decl_expand;
mod cache_sqlite;
mod cache_shm;
mod flat_cache_v1;
mod real_fft_sample_points;
mod param_expr_eval;
mod import_resolve;
mod inheritance;
mod record;
mod redeclare;
pub use self::error::FlattenError;
pub use self::array_size_policy::{load_array_sizes_json, load_array_sizes_json_optional, ArraySizePolicy};
pub use self::redeclare::{apply_modification_to_model, ModifyContext};

pub mod connections;
mod expand;
pub mod expressions;
pub mod structures;
mod substitute;
pub mod utils;
mod clock_infer;
pub mod flat_snapshot;

use self::connections::resolve_connections;
#[allow(unused_imports)]
pub use self::expressions::{
    eval_const_expr, eval_const_expr_with_params, expr_to_path, index_expression, prefix_expression,
};
pub use self::param_expr_eval::{eval_const_expr_with_array_sizes, eval_const_expr_with_param_exprs};
pub use self::structures::FlattenedModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    Full,
    QuickStructure,
    SuperFast,
}

impl ValidationMode {
    /// Execution contract (used by validate-only callers that stop at `--validate-tier=analyze`):
    /// - `Full`: run the full flatten pipeline.
    /// - `QuickStructure` / `SuperFast`: keep output sufficient for variable/equation analysis
    ///   (incl. initial equations and clocked-variable inference used by classification), but may
    ///   skip work that is only needed for later simulation/JIT phases (e.g. initial algorithms).
    ///   Constant propagation passes are additionally reduced in `decl_expand.rs`.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "quick" | "quickstructure" | "quick_structure" => Self::QuickStructure,
            "superfast" | "super_fast" => Self::SuperFast,
            _ => Self::Full,
        }
    }
}

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
    /// When true, `constrainedby` uses legacy string matching when a loader is available.
    pub coarse_constrainedby_only: bool,
    /// Validation mode controls how aggressively we approximate expensive constant propagation
    /// during validation-only workflows. Default: Full.
    pub validation_mode: ValidationMode,
    /// When array dimension expression is not a compile-time constant, fail instead of scalar fallback.
    pub array_size_policy: ArraySizePolicy,
    /// Flat-base-name -> dimension; merged before legacy/scalar fallback (see `load_array_sizes_json`).
    pub external_array_sizes: HashMap<String, usize>,
    /// Mirrors compiler `warnings_level`: "all" | "none" | "error" (affects array-size warnings in legacy mode).
    pub warnings_level: String,
}

impl Flattener {

    pub fn new() -> Self {
        Flattener {
            loader: ModelLoader::new(),
            name_cache: crate::string_intern::StringInterner::new(),
            coarse_constrainedby_only: false,
            validation_mode: ValidationMode::Full,
            array_size_policy: ArraySizePolicy::default(),
            external_array_sizes: HashMap::new(),
            warnings_level: "all".to_string(),
        }
    }

    /// root_name: model name used to load root (e.g. "TestLib/InitDummy") for DBG-4 source location in errors.
    pub fn flatten(
        &mut self,
        root: &mut Arc<Model>,
        root_name: &str,
    ) -> Result<FlattenedModel, FlattenError> {
        self.flatten_with_mode(root, root_name)
    }

    /// Mode-aware flatten pipeline. For validate-only analyze-tier usage, reduced modes skip work
    /// that does not affect structural analysis results.
    pub fn flatten_with_mode(
        &mut self,
        root: &mut Arc<Model>,
        root_name: &str,
    ) -> Result<FlattenedModel, FlattenError> {
        let root_path = root_name.replace('/', ".");
        self.flatten_inheritance(root, root_path.as_str())?;
        redeclare::validate_modification_prefixes_in_model(root.as_ref())?;
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
            stream_peer_map: HashMap::new(),
            interner: StringInterner::new(),
            inst_records: Vec::new(),
            path_to_inst: HashMap::new(),
        };
        self.expand_declarations(Arc::clone(root), "", &mut flat, Some(root_path.as_str()))?;
        self.expand_equations(model, "", &mut flat);
        self.expand_algorithms(model, "", &mut flat);
        // Initial equations affect variable classification (previous() scanning).
        self.expand_initial_equations(model, "", &mut flat);
        if matches!(self.validation_mode, ValidationMode::Full) {
            self.expand_initial_algorithms(model, "", &mut flat);
        }
        resolve_connections(&mut flat, Some(root_path.as_str()), &self.loader)?;
        // Clocked-variable inference affects discrete classification in analysis.
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

