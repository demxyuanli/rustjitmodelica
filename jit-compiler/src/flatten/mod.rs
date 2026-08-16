//! Flatten pipeline: inheritance, declaration expand, connections.
//! Out of scope here (future epics): full rustc-style query cache over defs, registry serde,
//! OMC instantiateModel parity tooling, parallel flatten.

use crate::ast::{AlgorithmStatement, Equation, Expression, Model, StringInterner};
use crate::loader::ModelLoader;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
mod error;
mod array_size_policy;
pub(crate) mod flatten_cache;
pub use self::flatten_cache::{
    all_disk_cache_roots, flatten_cache_dir, std_cache_root, user_cache_root,
};
mod decl_expand;
pub(crate) mod cache_sqlite;
pub use self::cache_sqlite::{
    export_sqlite_kind_stats_layers, sqlite_config_for_scope, sqlite_read_try_configs,
    sqlite_write_config_for_scope, CacheKindStatRow, CacheStatsLayerExport,
};
pub(crate) mod cache_shm;
pub(crate) mod flat_cache_v1;
pub(crate) mod flat_cache_v2;
pub(crate) mod inheritance_cache_v1;
mod real_fft_sample_points;
mod param_expr_eval;
mod import_resolve;
mod inheritance;
mod record;
mod redeclare;
pub use self::error::FlattenError;
pub use self::array_size_policy::{load_array_sizes_json, load_array_sizes_json_optional, ArraySizePolicy};
pub use self::redeclare::{apply_modification_to_model, apply_redeclare_extends_blocks, ModifyContext};

pub mod connections;
mod expand;
mod expand_body;
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
    ///   Constant propagation passes are additionally reduced in `decl_expand/`.
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
    /// Same strings as salsa `compile_stop` input: "full", "parse", "flatten", "analyze".
    pub compile_stop_label: String,
    /// Runtime guard can force-disable eq_expand parallel path for a compile.
    pub force_disable_eq_parallel: bool,
    /// After `flatten_inheritance` for a loaded class FQN; reused before per-instance modifications.
    inheritance_flat_template_cache: HashMap<String, Arc<Model>>,
    /// Tracks `inner` declarations by type+name → instance path, so `outer`
    /// declarations in child scopes can resolve to the correct inner instance.
    inner_declarations: HashMap<String, String>,
}

impl Flattener {
    fn equation_complexity(eq: &Equation) -> usize {
        fn expr_cost(e: &Expression) -> usize {
            use crate::ast::Expression as E;
            match e {
                E::Variable(_) | E::Number(_) | E::StringLiteral(_) => 1,
                E::BinaryOp(l, _, r) => 1 + expr_cost(l) + expr_cost(r),
                E::Call(_, args) => 4 + args.iter().map(expr_cost).sum::<usize>(),
                E::Der(i) | E::Sample(i) | E::Interval(i) | E::Hold(i) | E::Previous(i) => 2 + expr_cost(i),
                E::SubSample(a, b)
                | E::SuperSample(a, b)
                | E::ShiftSample(a, b)
                | E::BackSample(a, b)
                | E::ArrayAccess(a, b) => 2 + expr_cost(a) + expr_cost(b),
                E::If(c, t, f) => 3 + expr_cost(c) + expr_cost(t) + expr_cost(f),
                E::Range(s, st, e) => 2 + expr_cost(s) + expr_cost(st) + expr_cost(e),
                E::ArrayLiteral(items) => 2 + items.iter().map(expr_cost).sum::<usize>(),
                E::ArrayComprehension { expr, iter_range, .. } => 5 + expr_cost(expr) + expr_cost(iter_range),
                E::Dot(base, _) => 2 + expr_cost(base),
            }
        }
        match eq {
            Equation::Simple(l, r) => 2 + expr_cost(l) + expr_cost(r),
            Equation::Connect(a, b) => 6 + expr_cost(a) + expr_cost(b),
            Equation::For(_, s, e, body) => 8 + expr_cost(s) + expr_cost(e) + body.len() * 4,
            Equation::When(c, body, elses) => {
                10 + expr_cost(c) + body.len() * 3 + elses.iter().map(|(_, b)| b.len() * 3).sum::<usize>()
            }
            Equation::If(c, t, eifs, els) => {
                10 + expr_cost(c)
                    + t.len() * 3
                    + eifs.iter().map(|(_, b)| b.len() * 3).sum::<usize>()
                    + els.as_ref().map(|b| b.len() * 3).unwrap_or(0)
            }
            Equation::MultiAssign(lhss, rhs) => 10 + lhss.len() * 3 + expr_cost(rhs),
            Equation::Reinit(_, v) | Equation::Assert(v, _) | Equation::Terminate(v) | Equation::CallStmt(v) => {
                4 + expr_cost(v)
            }
            Equation::SolvableBlock { equations, residuals, .. } => {
                16 + equations.len() * 4 + residuals.len() * 3
            }
        }
    }

    fn algorithm_complexity(stmt: &AlgorithmStatement) -> usize {
        fn expr_cost(e: &Expression) -> usize {
            use crate::ast::Expression as E;
            match e {
                E::Variable(_) | E::Number(_) | E::StringLiteral(_) => 1,
                E::BinaryOp(l, _, r) => 1 + expr_cost(l) + expr_cost(r),
                E::Call(_, args) => 4 + args.iter().map(expr_cost).sum::<usize>(),
                E::Der(i) | E::Sample(i) | E::Interval(i) | E::Hold(i) | E::Previous(i) => 2 + expr_cost(i),
                E::SubSample(a, b)
                | E::SuperSample(a, b)
                | E::ShiftSample(a, b)
                | E::BackSample(a, b)
                | E::ArrayAccess(a, b) => 2 + expr_cost(a) + expr_cost(b),
                E::If(c, t, f) => 3 + expr_cost(c) + expr_cost(t) + expr_cost(f),
                E::Range(s, st, e) => 2 + expr_cost(s) + expr_cost(st) + expr_cost(e),
                E::ArrayLiteral(items) => 2 + items.iter().map(expr_cost).sum::<usize>(),
                E::ArrayComprehension { expr, iter_range, .. } => 5 + expr_cost(expr) + expr_cost(iter_range),
                E::Dot(base, _) => 2 + expr_cost(base),
            }
        }
        match stmt {
            AlgorithmStatement::Assignment(l, r) => 2 + expr_cost(l) + expr_cost(r),
            AlgorithmStatement::CallStmt(e)
            | AlgorithmStatement::Reinit(_, e)
            | AlgorithmStatement::Assert(e, _)
            | AlgorithmStatement::Terminate(e) => 4 + expr_cost(e),
            AlgorithmStatement::MultiAssign(lhss, rhs) => 10 + lhss.len() * 3 + expr_cost(rhs),
            AlgorithmStatement::If(c, t, eifs, els) => {
                10 + expr_cost(c)
                    + t.len() * 3
                    + eifs.iter().map(|(_, b)| b.len() * 3).sum::<usize>()
                    + els.as_ref().map(|b| b.len() * 3).unwrap_or(0)
            }
            AlgorithmStatement::While(c, b) => 8 + expr_cost(c) + b.len() * 4,
            AlgorithmStatement::For(_, r, b) => 8 + expr_cost(r) + b.len() * 4,
            AlgorithmStatement::When(c, b, elses) => {
                10 + expr_cost(c) + b.len() * 3 + elses.iter().map(|(_, b)| b.len() * 3).sum::<usize>()
            }
            AlgorithmStatement::NoOp => 1,
            AlgorithmStatement::Break => 1,
            AlgorithmStatement::Return(v) => 1 + v.as_ref().map(expr_cost).unwrap_or(0),
        }
    }

    fn balanced_buckets_by_weight<T: Clone>(
        items: &[T],
        weight_fn: impl Fn(&T) -> usize,
        bucket_count: usize,
    ) -> Vec<Vec<(usize, T)>> {
        let n = bucket_count.max(1);
        let mut buckets: Vec<Vec<(usize, T)>> = (0..n).map(|_| Vec::new()).collect();
        let mut loads: Vec<usize> = vec![0; n];
        let mut indexed: Vec<(usize, usize)> = items
            .iter()
            .enumerate()
            .map(|(idx, it)| (idx, weight_fn(it).max(1)))
            .collect();
        indexed.sort_by(|a, b| b.1.cmp(&a.1));
        for (idx, w) in indexed {
            let mut min_pos = 0usize;
            for i in 1..loads.len() {
                if loads[i] < loads[min_pos] {
                    min_pos = i;
                }
            }
            buckets[min_pos].push((idx, items[idx].clone()));
            loads[min_pos] = loads[min_pos].saturating_add(w);
        }
        buckets
    }

    fn flatten_eq_parallel_enabled(&self) -> bool {
        if self.force_disable_eq_parallel {
            return false;
        }
        std::env::var("RUSTMODLICA_FLATTEN_EQ_PARALLEL")
            .ok()
            .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
            .unwrap_or(false)
    }

    fn flatten_eq_parallel_min_items(&self) -> usize {
        std::env::var("RUSTMODLICA_FLATTEN_EQ_PARALLEL_MIN_ITEMS")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(64)
    }

    fn flatten_eq_parallel_micro_batch_size(&self) -> usize {
        std::env::var("RUSTMODLICA_FLATTEN_EQ_MICRO_BATCH")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(8)
    }

    fn flatten_eq_parallel_micro_batch_budget(&self) -> usize {
        std::env::var("RUSTMODLICA_FLATTEN_EQ_MICRO_BATCH_BUDGET")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(512)
    }

    fn adaptive_batches_by_budget<T: Clone>(
        items: &[(usize, T)],
        weight_fn: impl Fn(&T) -> usize,
        max_items_per_batch: usize,
        budget: usize,
    ) -> Vec<Vec<(usize, T)>> {
        let mut batches: Vec<Vec<(usize, T)>> = Vec::new();
        let mut current: Vec<(usize, T)> = Vec::new();
        let mut current_cost: usize = 0;
        let max_items = max_items_per_batch.max(1);
        let budget = budget.max(1);
        for (idx, item) in items {
            let w = weight_fn(item).max(1);
            let would_overflow = !current.is_empty()
                && (current_cost.saturating_add(w) > budget || current.len() >= max_items);
            if would_overflow {
                batches.push(current);
                current = Vec::new();
                current_cost = 0;
            }
            current.push((*idx, item.clone()));
            current_cost = current_cost.saturating_add(w);
        }
        if !current.is_empty() {
            batches.push(current);
        }
        batches
    }

    fn clone_for_parallel_expand(&self) -> Self {
        let mut f = Flattener::new();
        f.coarse_constrainedby_only = self.coarse_constrainedby_only;
        f.validation_mode = self.validation_mode;
        f.array_size_policy = self.array_size_policy;
        f.external_array_sizes = self.external_array_sizes.clone();
        f.warnings_level = self.warnings_level.clone();
        f.compile_stop_label = self.compile_stop_label.clone();
        f.force_disable_eq_parallel = self.force_disable_eq_parallel;
        for path in &self.loader.library_paths {
            f.loader.add_path(path.clone());
        }
        f.loader.set_quiet(self.loader.quiet);
        f
    }

    pub fn new() -> Self {
        Flattener {
            loader: ModelLoader::new(),
            name_cache: crate::string_intern::StringInterner::new(),
            coarse_constrainedby_only: false,
            validation_mode: ValidationMode::Full,
            array_size_policy: ArraySizePolicy::default(),
            external_array_sizes: HashMap::new(),
            warnings_level: "all".to_string(),
            compile_stop_label: "full".to_string(),
            force_disable_eq_parallel: false,
            inheritance_flat_template_cache: HashMap::new(),
            inner_declarations: HashMap::new(),
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
        let mode_start = std::time::Instant::now();

        // Set up enumerations for enum literal validation during flatten.
        {
            let mut enums = root.enumerations.clone();
            // Also collect from inner classes
            for inner in &root.inner_classes {
                for (k, v) in &inner.enumerations {
                    enums.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
            crate::flatten::expressions::set_flatten_enumerations(enums);
        }

        // Emit mode diagnostic for performance analysis
        if std::env::var("RUSTMODLICA_PERF_TRACE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            eprintln!(
                "[flatten] START mode={:?} model={}",
                self.validation_mode,
                root_path
            );
        }

        self.flatten_inheritance(root, root_path.as_str())?;
        redeclare::validate_modification_prefixes_in_model(root.as_ref())?;

        let result = match self.validation_mode {
            ValidationMode::SuperFast => {
                // SuperFast: Only collect top-level declarations without recursive sub-model loading.
                // This is the fastest path for structural validation.
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
                    stream_connection_set: HashMap::new(),
                    stream_flow_map: HashMap::new(),
                    expandable_instances: HashSet::new(),
                    interner: StringInterner::new(),
                    inst_records: Vec::new(),
                    path_to_inst: HashMap::new(),
                };
                // Only expand top-level declarations (no recursive sub-model loading)
                self.expand_declarations_super_fast(Arc::clone(root), "", &mut flat, Some(root_path.as_str()))?;
                // Still need equations for structural analysis
                self.expand_equations(model, "", &mut flat);
                // Skip algorithms, initial_equations, initial_algorithms for SuperFast
                resolve_connections(&mut flat, Some(root_path.as_str()), &self.loader)?;
                self.infer_clocked_variables(&mut flat);
                Ok(flat)
            }
            ValidationMode::QuickStructure => {
                // QuickStructure: Simplified declaration expand with reduced iteration counts.
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
                    stream_connection_set: HashMap::new(),
                    stream_flow_map: HashMap::new(),
                    expandable_instances: HashSet::new(),
                    interner: StringInterner::new(),
                    inst_records: Vec::new(),
                    path_to_inst: HashMap::new(),
                };
                self.expand_declarations(Arc::clone(root), "", &mut flat, Some(root_path.as_str()))?;
                self.expand_equations(model, "", &mut flat);
                // Skip algorithms for QuickStructure (not needed for structural analysis)
                self.expand_initial_equations(model, "", &mut flat);
                // Skip initial_algorithms for QuickStructure
                resolve_connections(&mut flat, Some(root_path.as_str()), &self.loader)?;
                self.infer_clocked_variables(&mut flat);
                Ok(flat)
            }
            ValidationMode::Full => {
                // Full: Complete flatten pipeline
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
                    stream_connection_set: HashMap::new(),
                    stream_flow_map: HashMap::new(),
                    expandable_instances: HashSet::new(),
                    interner: StringInterner::new(),
                    inst_records: Vec::new(),
                    path_to_inst: HashMap::new(),
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
        };

        // Emit completion diagnostic
        if std::env::var("RUSTMODLICA_PERF_TRACE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            if let Ok(ref flat) = result {
                eprintln!(
                    "[flatten] END mode={:?} decls={} eqs={} instances={} us={}",
                    self.validation_mode,
                    flat.declarations.len(),
                    flat.equations.len(),
                    flat.instances.len(),
                    mode_start.elapsed().as_micros()
                );
            }
        }

        result
    }

    /// Flatten assuming inheritance was already applied to `root`.
    /// Intended for query-based workflows that compute inheritance in a separate stage.
    pub fn flatten_with_mode_preinherited(
        &mut self,
        root: &Arc<Model>,
        root_name: &str,
    ) -> Result<FlattenedModel, FlattenError> {
        let root_path = root_name.replace('/', ".");
        redeclare::validate_modification_prefixes_in_model(root.as_ref())?;

        // Set up enumerations for enum literal validation during flatten.
        {
            let mut enums = root.enumerations.clone();
            for inner in &root.inner_classes {
                for (k, v) in &inner.enumerations {
                    enums.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
            crate::flatten::expressions::set_flatten_enumerations(enums);
        }

        match self.validation_mode {
            ValidationMode::SuperFast => {
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
                    stream_connection_set: HashMap::new(),
                    stream_flow_map: HashMap::new(),
                    expandable_instances: HashSet::new(),
                    interner: StringInterner::new(),
                    inst_records: Vec::new(),
                    path_to_inst: HashMap::new(),
                };
                self.expand_declarations_super_fast(Arc::clone(root), "", &mut flat, Some(root_path.as_str()))?;
                self.expand_equations(model, "", &mut flat);
                resolve_connections(&mut flat, Some(root_path.as_str()), &self.loader)?;
                self.infer_clocked_variables(&mut flat);
                Ok(flat)
            }
            ValidationMode::QuickStructure => {
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
                    stream_connection_set: HashMap::new(),
                    stream_flow_map: HashMap::new(),
                    expandable_instances: HashSet::new(),
                    interner: StringInterner::new(),
                    inst_records: Vec::new(),
                    path_to_inst: HashMap::new(),
                };
                self.expand_declarations(Arc::clone(root), "", &mut flat, Some(root_path.as_str()))?;
                self.expand_equations(model, "", &mut flat);
                self.expand_initial_equations(model, "", &mut flat);
                resolve_connections(&mut flat, Some(root_path.as_str()), &self.loader)?;
                self.infer_clocked_variables(&mut flat);
                Ok(flat)
            }
            ValidationMode::Full => {
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
                    stream_connection_set: HashMap::new(),
                    stream_flow_map: HashMap::new(),
                    expandable_instances: HashSet::new(),
                    interner: StringInterner::new(),
                    inst_records: Vec::new(),
                    path_to_inst: HashMap::new(),
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
        }
    }

    /// SuperFast declaration expansion: only top-level declarations, no recursive sub-model loading.
    pub(crate) fn expand_declarations_super_fast(
        &mut self,
        model: Arc<Model>,
        prefix: &str,
        flat: &mut FlattenedModel,
        current_model_name: Option<&str>,
    ) -> Result<(), FlattenError> {
        self.expand_declarations_with_mode(
            model,
            prefix,
            flat,
            current_model_name,
            decl_expand::ExpandDeclMode::SuperFast,
        )
    }

    pub(crate) fn decl_expand_preinherited(
        &mut self,
        root: Arc<Model>,
        root_name: &str,
    ) -> Result<FlattenedModel, FlattenError> {
        let root_path = root_name.replace('/', ".");
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
            stream_connection_set: HashMap::new(),
            stream_flow_map: HashMap::new(),
            expandable_instances: HashSet::new(),
            interner: StringInterner::new(),
            inst_records: Vec::new(),
            path_to_inst: HashMap::new(),
        };
        // DeclAndSubEq: expand nested instance equations here so the salsa split
        // (decl_expanded + eq_expanded) matches legacy Full flatten. DeclOnly left
        // nested eqs empty and eq_expand_root_preinherited only expanded the root,
        // collapsing MultiBody models (e.g. EngineV6) to a handful of root connects.
        self.expand_declarations_with_mode(
            root,
            "",
            &mut flat,
            Some(root_path.as_str()),
            crate::flatten::decl_expand::ExpandDeclMode::DeclAndSubEq,
        )?;
        Ok(flat)
    }

    pub(crate) fn eq_expand_root_preinherited(
        &mut self,
        root: &Model,
        flat: &mut FlattenedModel,
    ) {
        // Nested instance equations/connections are expected to already be present
        // in `flat` (from DeclAndSubEq). This only adds the root model's sections.
        let t = std::time::Instant::now();
        self.expand_equations(root, "", flat);
        crate::query_db::perf_record_us(
            "eq_expand_equations_us",
            t.elapsed().as_micros() as u64,
        );
        let t = std::time::Instant::now();
        self.expand_algorithms(root, "", flat);
        crate::query_db::perf_record_us(
            "eq_expand_algorithms_us",
            t.elapsed().as_micros() as u64,
        );
        let t = std::time::Instant::now();
        self.expand_initial_equations(root, "", flat);
        crate::query_db::perf_record_us(
            "eq_expand_initial_equations_us",
            t.elapsed().as_micros() as u64,
        );
        if matches!(self.validation_mode, ValidationMode::Full) {
            let t = std::time::Instant::now();
            self.expand_initial_algorithms(root, "", flat);
            crate::query_db::perf_record_us(
                "eq_expand_initial_algorithms_us",
                t.elapsed().as_micros() as u64,
            );
        }
    }

    pub(crate) fn infer_clocked_variables_preinherited(&self, flat: &mut FlattenedModel) {
        self.infer_clocked_variables(flat);
    }

    // expand_* methods: expand_body.rs
}

