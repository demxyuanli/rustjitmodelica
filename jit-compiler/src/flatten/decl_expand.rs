use super::{eval_const_expr_with_param_exprs, index_expression, FlattenError, Flattener};
use crate::ast::{Declaration, Expression, Model};
use crate::diag::SourceLocation;
use crate::loader::LoadError;
use crate::flatten::utils::{is_primitive, resolve_inner_class_alias, resolve_type_alias};
use crate::flatten::{apply_modification_to_model, ModifyContext};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use super::substitute::SubstituteCache;

fn perf_trace_enabled() -> bool {
    std::env::var("RUSTMODLICA_PERF_TRACE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

#[derive(Debug, Default)]
struct ParamPassOptimizer {
    param_deps: HashMap<String, HashSet<String>>,
    stable_params: HashSet<String>,
    last_change_pass: HashMap<String, usize>,
    dependents_index: HashMap<String, Vec<String>>,
}

impl ParamPassOptimizer {
    fn rebuild_dependency_graph(&mut self, model: &Model) {
        self.param_deps.clear();
        self.dependents_index.clear();
        for decl in &model.declarations {
            if !decl.is_parameter {
                continue;
            }
            let mut deps: HashSet<String> = HashSet::new();
            if let Some(val) = decl.start_value.as_ref() {
                Self::collect_var_refs(val, &mut deps);
            }
            deps.remove(&decl.name);
            self.param_deps.insert(decl.name.clone(), deps);
        }
        // Reverse index: param -> dependents
        for (p, deps) in &self.param_deps {
            for d in deps {
                self.dependents_index
                    .entry(d.clone())
                    .or_default()
                    .push(p.clone());
            }
        }
    }

    fn collect_var_refs(expr: &Expression, out: &mut HashSet<String>) {
        use crate::ast::Expression as E;
        match expr {
            E::Variable(id) => {
                out.insert(crate::string_intern::resolve_id(*id));
            }
            E::BinaryOp(l, _, r) => {
                Self::collect_var_refs(l, out);
                Self::collect_var_refs(r, out);
            }
            E::Call(_, args) => {
                for a in args {
                    Self::collect_var_refs(a, out);
                }
            }
            E::Der(inner) => Self::collect_var_refs(inner, out),
            E::If(c, t, f) => {
                Self::collect_var_refs(c, out);
                Self::collect_var_refs(t, out);
                Self::collect_var_refs(f, out);
            }
            E::ArrayAccess(arr, idx) => {
                Self::collect_var_refs(arr, out);
                Self::collect_var_refs(idx, out);
            }
            E::ArrayLiteral(items) => {
                for it in items {
                    Self::collect_var_refs(it, out);
                }
            }
            _ => {}
        }
    }

    fn optimize_param_passes(
        &mut self,
        flattener: &mut Flattener,
        model: &Model,
        context: &mut HashMap<String, Expression>,
        local_array_sizes: &HashMap<String, usize>,
    ) -> usize {
        let (max_fast_passes, stability_passes) = match flattener.validation_mode {
            super::ValidationMode::SuperFast => return 0,
            super::ValidationMode::QuickStructure => (5usize, 1usize),
            super::ValidationMode::Full => (32usize, 2usize),
        };

        self.stable_params.clear();
        self.last_change_pass.clear();
        self.rebuild_dependency_graph(model);

        let mut stalled = 0usize;
        let mut pass = 0usize;
        while pass < max_fast_passes {
            let mut sub_cache = SubstituteCache::new(4096);
            let mut changed_params: Vec<String> = Vec::new();
            for decl in &model.declarations {
                if !decl.is_parameter {
                    continue;
                }
                if self.stable_params.contains(&decl.name) {
                    continue;
                }
                let Some(val) = decl.start_value.as_ref() else {
                    self.stable_params.insert(decl.name.clone());
                    continue;
                };
                if self
                    .param_deps
                    .get(&decl.name)
                    .map(|s| s.is_empty())
                    .unwrap_or(false)
                {
                    self.stable_params.insert(decl.name.clone());
                    continue;
                }
                let sub = flattener.substitute_cached_cow(val, context, &mut sub_cache);
                if let Some(n) = eval_const_expr_with_param_exprs(sub.as_ref(), context, local_array_sizes) {
                    let update = match context.get(&decl.name) {
                        None => true,
                        Some(Expression::Number(p)) => (n - p).abs() > 1e-12,
                        Some(_) => true,
                    };
                    if update {
                        context.insert(decl.name.clone(), Expression::Number(n));
                        changed_params.push(decl.name.clone());
                        self.last_change_pass.insert(decl.name.clone(), pass);
                    }
                }
            }

            if changed_params.is_empty() {
                stalled += 1;
                if stalled >= stability_passes {
                    break;
                }
            } else {
                stalled = 0;
                for changed in &changed_params {
                    if let Some(deps) = self.dependents_index.get(changed) {
                        for dep in deps {
                            self.stable_params.remove(dep);
                        }
                    }
                }
            }

            for p in self.param_deps.keys() {
                if self.stable_params.contains(p) {
                    continue;
                }
                let last = *self.last_change_pass.get(p).unwrap_or(&0);
                if pass.saturating_sub(last) > 5 {
                    self.stable_params.insert(p.clone());
                }
            }
            pass += 1;
        }
        pass
    }
}

#[derive(Debug, Default)]
struct ArrayDimensionOptimizer {
    computed_dims: HashMap<String, usize>,
    uncalculable: HashSet<String>,
}

impl ArrayDimensionOptimizer {
    fn compute_expr_complexity(expr: &Expression) -> u32 {
        use crate::ast::Expression as E;
        match expr {
            E::Number(_) => 0,
            E::Variable(_) => 1,
            E::StringLiteral(_) => 0,
            E::BinaryOp(l, _, r) => 1 + Self::compute_expr_complexity(l) + Self::compute_expr_complexity(r),
            E::Call(_, args) => 3 + args.iter().map(Self::compute_expr_complexity).sum::<u32>(),
            E::If(c, t, f) => {
                2 + Self::compute_expr_complexity(c)
                    + Self::compute_expr_complexity(t)
                    + Self::compute_expr_complexity(f)
            }
            E::Der(inner) => 1 + Self::compute_expr_complexity(inner),
            E::ArrayAccess(a, i) => 2 + Self::compute_expr_complexity(a) + Self::compute_expr_complexity(i),
            E::ArrayLiteral(items) => 2 + items.iter().map(Self::compute_expr_complexity).sum::<u32>(),
            _ => 10,
        }
    }

    fn optimize_array_dims(
        &mut self,
        flattener: &mut Flattener,
        model: &Model,
        context: &HashMap<String, Expression>,
        local_array_sizes: &mut HashMap<String, usize>,
    ) -> usize {
        const COMPLEXITY_THRESHOLD: u32 = 5;
        let max_fast_passes = match flattener.validation_mode {
            super::ValidationMode::SuperFast => return 0,
            super::ValidationMode::QuickStructure => 3usize,
            super::ValidationMode::Full => 16usize,
        };
        let perf = perf_trace_enabled();
        self.computed_dims.clear();
        self.uncalculable.clear();

        let mut pass = 0usize;
        while pass < max_fast_passes {
            let mut sub_cache = SubstituteCache::new(4096);
            let mut dim_changed = false;
            for decl in &model.declarations {
                if local_array_sizes.contains_key(&decl.name) {
                    continue;
                }
                if self.uncalculable.contains(&decl.name) {
                    continue;
                }
                let Some(size_expr) = decl.array_size.as_ref() else {
                    continue;
                };
                if let Some(&n) = self.computed_dims.get(&decl.name) {
                    local_array_sizes.insert(decl.name.clone(), n);
                    continue;
                }
                if let Expression::Number(n) = size_expr {
                    let sz = *n as usize;
                    if sz > 0 {
                        local_array_sizes.insert(decl.name.clone(), sz);
                        self.computed_dims.insert(decl.name.clone(), sz);
                        dim_changed = true;
                    }
                    continue;
                }
                let complexity = Self::compute_expr_complexity(size_expr);
                if complexity > COMPLEXITY_THRESHOLD {
                    self.uncalculable.insert(decl.name.clone());
                    if perf {
                        eprintln!(
                            "[perf] array_dim_skip_complex name={} complexity={}",
                            decl.name, complexity
                        );
                    }
                    continue;
                }
                if let Some(ref cond_expr) = decl.condition {
                    let cond_sub = flattener.substitute_cached_cow(cond_expr, context, &mut sub_cache);
                    if let Some(v) =
                        eval_const_expr_with_param_exprs(cond_sub.as_ref(), context, local_array_sizes)
                    {
                        if v == 0.0 {
                            continue;
                        }
                    }
                }
                let sub_expr = flattener.substitute_cached_cow(size_expr, context, &mut sub_cache);
                if let Some(val) =
                    eval_const_expr_with_param_exprs(sub_expr.as_ref(), context, local_array_sizes)
                {
                    let n = val as usize;
                    if n > 0 {
                        local_array_sizes.insert(decl.name.clone(), n);
                        self.computed_dims.insert(decl.name.clone(), n);
                        dim_changed = true;
                    }
                }
            }
            if !dim_changed {
                break;
            }
            pass += 1;
        }
        pass
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExpandDeclMode {
    DeclOnly,
    DeclAndSubEq,
}

impl Flattener {
    pub(super) fn expand_declarations(
        &mut self,
        model: Arc<Model>,
        prefix: &str,
        flat: &mut crate::flatten::FlattenedModel,
        current_model_name: Option<&str>,
    ) -> Result<(), FlattenError> {
        self.expand_declarations_with_mode(
            model,
            prefix,
            flat,
            current_model_name,
            ExpandDeclMode::DeclAndSubEq,
        )
    }

    pub(super) fn expand_declarations_with_mode(
        &mut self,
        model: Arc<Model>,
        prefix: &str,
        flat: &mut crate::flatten::FlattenedModel,
        current_model_name: Option<&str>,
        mode: ExpandDeclMode,
    ) -> Result<(), FlattenError> {
        #[derive(Clone)]
        enum Task {
            Process {
                model: Arc<Model>,
                prefix: String,
                current_model_name: Option<String>,
                msl_import_context: String,
            },
            ExpandEquations {
                model: Arc<Model>,
                prefix: String,
            },
        }

        let msl_ctx = current_model_name.unwrap_or("").to_string();
        let mut stack: Vec<Task> = vec![Task::Process {
            model,
            prefix: prefix.to_string(),
            current_model_name: current_model_name.map(|s| s.to_string()),
            msl_import_context: msl_ctx,
        }];
        let mut array_size_warned_names: HashSet<String> = HashSet::new();

        while let Some(task) = stack.pop() {
            match task {
                Task::ExpandEquations { model, prefix } => {
                    self.expand_equations(model.as_ref(), &prefix, flat);
                }
                Task::Process {
                    model,
                    prefix,
                    current_model_name,
                    msl_import_context,
                } => {
                    let current_qualified = current_model_name.as_deref().unwrap_or("");
                    let mut context: HashMap<String, Expression> = HashMap::new();
                    let mut local_array_sizes: HashMap<String, usize> = HashMap::new();
                    for decl in &model.declarations {
                        if decl.is_parameter {
                            if let Some(val) = &decl.start_value {
                                context.insert(decl.name.clone(), val.clone());
                            }
                        }
                    }

                    // Parameter propagation is a hot path in validate mode; use a fast optimizer
                    // first, then fall back to the legacy fixed-point loop if not converged.
                    let mut param_opt = ParamPassOptimizer::default();
                    let fast_passes =
                        param_opt.optimize_param_passes(self, &model, &mut context, &local_array_sizes);
                    let perf = perf_trace_enabled();
                    if perf && fast_passes >= 16 {
                        eprintln!(
                            "[perf] param_passes_fast={} total_params={} stable_params={}",
                            fast_passes,
                            param_opt.param_deps.len(),
                            param_opt.stable_params.len()
                        );
                    }
                    const MAX_PARAM_PASSES_TOTAL: usize = 128;
                    if self.validation_mode == super::ValidationMode::Full && fast_passes >= 32 {
                        for _ in fast_passes..MAX_PARAM_PASSES_TOTAL {
                            let mut changed = false;
                            for decl in &model.declarations {
                                if decl.is_parameter {
                                    if let Some(val) = &decl.start_value {
                                        let sub = self.substitute(val, &context);
                                        if let Some(n) = eval_const_expr_with_param_exprs(
                                            &sub,
                                            &context,
                                            &local_array_sizes,
                                        ) {
                                            let update = match context.get(&decl.name) {
                                                None => true,
                                                Some(Expression::Number(p)) => (n - p).abs() > 1e-12,
                                                Some(_) => true,
                                            };
                                            if update {
                                                context.insert(decl.name.clone(), Expression::Number(n));
                                                changed = true;
                                            }
                                        }
                                    }
                                }
                            }
                            if !changed {
                                break;
                            }
                        }
                    }

                    // Array dimension inference is another validate hot path; use a fast optimizer
                    // then fall back to the legacy fixed-point loop if needed.
                    let mut arr_opt = ArrayDimensionOptimizer::default();
                    let dim_fast_passes =
                        arr_opt.optimize_array_dims(self, &model, &context, &mut local_array_sizes);
                    if perf_trace_enabled() && dim_fast_passes >= 8 {
                        eprintln!(
                            "[perf] array_dim_passes_fast={} computed={} uncalculable={}",
                            dim_fast_passes,
                            arr_opt.computed_dims.len(),
                            arr_opt.uncalculable.len()
                        );
                    }
                    const MAX_ARRAY_DIM_PASSES_TOTAL: usize = 64;
                    if self.validation_mode == super::ValidationMode::Full && dim_fast_passes >= 16 {
                        for _ in dim_fast_passes..MAX_ARRAY_DIM_PASSES_TOTAL {
                            let mut dim_changed = false;
                            for decl in &model.declarations {
                                if let Some(ref cond_expr) = decl.condition {
                                    let cond_sub = self.substitute(cond_expr, &context);
                                    if let Some(v) = eval_const_expr_with_param_exprs(
                                        &cond_sub,
                                        &context,
                                        &local_array_sizes,
                                    ) {
                                        if v == 0.0 {
                                            continue;
                                        }
                                    }
                                }
                                let Some(size_expr) = decl.array_size.as_ref() else {
                                    continue;
                                };
                                if local_array_sizes.contains_key(&decl.name) {
                                    continue;
                                }
                                let sub_expr = self.substitute(size_expr, &context);
                                if let Some(val) = eval_const_expr_with_param_exprs(
                                    &sub_expr,
                                    &context,
                                    &local_array_sizes,
                                ) {
                                    let n = val as usize;
                                    if n > 0 {
                                        local_array_sizes.insert(decl.name.clone(), n);
                                        dim_changed = true;
                                    }
                                }
                            }
                            if !dim_changed {
                                break;
                            }
                        }
                    }

                    for decl in &model.declarations {
                        if let Some(ref cond_expr) = decl.condition {
                            let cond_sub = self.substitute(cond_expr, &context);
                            if let Some(v) =
                                eval_const_expr_with_param_exprs(&cond_sub, &context, &local_array_sizes)
                            {
                                if v == 0.0 {
                                    continue;
                                }
                            }
                        }

                        let base_name = if prefix.is_empty() {
                            decl.name.clone()
                        } else {
                            format!("{}_{}", prefix, decl.name)
                        };

                        let array_len = if let Some(size_expr) = &decl.array_size {
                            let sub_expr = self.substitute(size_expr, &context);
                            if let Some(val) = eval_const_expr_with_param_exprs(
                                &sub_expr,
                                &context,
                                &local_array_sizes,
                            ) {
                                Some(val as usize)
                            } else if let Some(&n) = self.external_array_sizes.get(&base_name) {
                                Some(n)
                            } else {
                                let fail_flatten = matches!(
                                    self.array_size_policy,
                                    super::ArraySizePolicy::Strict
                                ) || self.warnings_level == "error";
                                if fail_flatten {
                                    return Err(FlattenError::UnevaluatedArraySize {
                                        flat_base_name: base_name.clone(),
                                    });
                                }
                                if self.warnings_level != "none"
                                    && array_size_warned_names.insert(decl.name.clone())
                                {
                                    eprintln!(
                                        "{}",
                                        crate::i18n::msg(
                                            "warning_array_size",
                                            &[&base_name as &dyn std::fmt::Display],
                                        )
                                    );
                                }
                                None
                            }
                        } else {
                            None
                        };

                        let count = array_len.unwrap_or(1);
                        let is_array = array_len.is_some();

                        let mut each_start: Option<Expression> = None;
                        for m in &decl.modifications {
                            if m.each && m.name == "start" {
                                each_start = m.value.clone();
                            }
                        }

                        if is_array {
                            flat.array_sizes.insert(base_name.clone(), count);
                            local_array_sizes.insert(decl.name.clone(), count);
                            if !decl.is_parameter || decl.start_value.is_none() {
                                context.insert(
                                    decl.name.clone(),
                                    Expression::ArrayLiteral(vec![Expression::Number(0.0); count]),
                                );
                            }
                        }

                        for i in 1..=count {
                            let name_suffix = if is_array { format!("_{}", i) } else { "".to_string() };
                            let local_name = format!("{}{}", decl.name, name_suffix);
                            let full_path = if prefix.is_empty() {
                                local_name.clone()
                            } else {
                                format!("{}_{}", prefix, local_name)
                            };

                            let loc = current_model_name
                                .as_deref()
                                .and_then(|n| self.loader.get_path_for_model(n))
                                .map(|p| SourceLocation {
                                    file: p.display().to_string(),
                                    line: 0,
                                    column: 0,
                                });

                            let mut resolved_type = resolve_type_alias(&model.type_aliases, &decl.type_name);
                            let pre_inner_alias = resolved_type.clone();
                            resolved_type = resolve_inner_class_alias(&model, &resolved_type);
                            resolved_type = Self::resolve_import_scoped_type(
                                model.as_ref(),
                                &resolved_type,
                                current_qualified,
                                &msl_import_context,
                            );
                            resolved_type =
                                Self::normalize_decl_type_name(resolved_type, &pre_inner_alias);

                            if is_primitive(&resolved_type) {
                                flat.declarations.push(Declaration {
                                    type_name: resolved_type.clone(),
                                    name: full_path.clone(),
                                    replaceable: decl.replaceable,
                                    constrainedby_type: decl.constrainedby_type.clone(),
                                    is_parameter: decl.is_parameter,
                                    is_flow: decl.is_flow,
                                    is_stream: decl.is_stream,
                                    is_discrete: decl.is_discrete,
                                    is_input: decl.is_input,
                                    is_output: decl.is_output,
                                    is_inner: decl.is_inner,
                                    is_outer: decl.is_outer,
                                    is_public: decl.is_public,
                                    is_protected: decl.is_protected,
                                    start_value: if let Some(ev) = &each_start {
                                        let sub = self.substitute(ev, &context);
                                        if is_array {
                                            Some(index_expression(&sub, i))
                                        } else {
                                            Some(sub)
                                        }
                                    } else if let Some(val) = &decl.start_value {
                                        let sub = self.substitute(val, &context);
                                        if is_array { Some(index_expression(&sub, i)) } else { Some(sub) }
                                    } else {
                                        None
                                    },
                                    array_size: None,
                                    modifications: Vec::new(),
                                    is_rest: decl.is_rest,
                                    annotation: None,
                                    condition: None,
                                });
                                flat.register_inst_path(
                                    full_path.clone(),
                                    resolved_type.clone(),
                                    if is_array { Some(i as usize) } else { None },
                                );
                                continue;
                            }

                            // Use lexical scope for candidate qualification.
                            // Some MSL libraries reference siblings via relative prefixes (e.g. `Utilities.*`,
                            // `Analog.*`) and the loader may report `current_qualified` as a relative name.
                            // When that happens, prefer the root import context as the scope anchor so
                            // upward qualification can reach `Modelica.*`.
                            let scope_for_candidates = if current_qualified.is_empty() {
                                msl_import_context.as_str()
                            } else if !msl_import_context.is_empty()
                                && msl_import_context.starts_with("Modelica.")
                                && !current_qualified.starts_with("Modelica.")
                            {
                                msl_import_context.as_str()
                            } else {
                                current_qualified
                            };
                            let load_candidates =
                                Self::build_load_candidates(&resolved_type, scope_for_candidates);
                            if perf_trace_enabled()
                                && (resolved_type == "Analog.Interfaces.NegativePin"
                                    || resolved_type.ends_with(".Analog.Interfaces.NegativePin"))
                            {
                                eprintln!(
                                    "[perf] type_load_probe resolved_type={} current={} msl_ctx={} scope={} candidates={:?}",
                                    resolved_type,
                                    current_qualified,
                                    msl_import_context,
                                    scope_for_candidates,
                                    load_candidates
                                );
                            }
                            let (loaded_type, last_err) = self.try_load_sub_model(
                                model.as_ref(),
                                &resolved_type,
                                scope_for_candidates,
                                &load_candidates,
                            );

                            let mut sub_model = match loaded_type {
                                Some((resolved_candidate, m)) => {
                                    resolved_type = resolved_candidate;
                                    m
                                }
                                None => {
                                    let e = last_err.unwrap_or_else(|| LoadError::NotFound(resolved_type.clone()));
                                    if matches!(&e, LoadError::NotFound(_)) {
                                        if let Some((prefix_type, suffix_type)) = resolved_type.rsplit_once('.') {
                                            if let Ok(owner) = self.loader.load_model(prefix_type) {
                                                if let Some((_, base)) =
                                                    owner.type_aliases.iter().find(|(a, _)| a == suffix_type)
                                                {
                                                    let base = resolve_type_alias(&owner.type_aliases, base);
                                                    if is_primitive(&base) {
                                                        flat.declarations.push(Declaration {
                                                            type_name: base.clone(),
                                                            name: full_path.clone(),
                                                            replaceable: decl.replaceable,
                                                            constrainedby_type: decl.constrainedby_type.clone(),
                                                            is_parameter: decl.is_parameter,
                                                            is_flow: decl.is_flow,
                                                            is_stream: decl.is_stream,
                                                            is_discrete: decl.is_discrete,
                                                            is_input: decl.is_input,
                                                            is_output: decl.is_output,
                                                            is_inner: decl.is_inner,
                                                            is_outer: decl.is_outer,
                                                            is_public: decl.is_public,
                                                            is_protected: decl.is_protected,
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
                                                            condition: None,
                                                        });
                                                        flat.register_inst_path(
                                                            full_path.clone(),
                                                            base,
                                                            if is_array { Some(i as usize) } else { None },
                                                        );
                                                        continue;
                                                    }
                                                }
                                            }
                                        }
                                        if !resolved_type.contains('.') {
                                            for (_alias, qual) in &model.imports {
                                                if qual.is_empty() {
                                                    continue;
                                                }
                                                let candidate = format!("{}.{}", qual, resolved_type);
                                                if let Some((prefix_type, suffix_type)) = candidate.rsplit_once('.')
                                                {
                                                    if let Ok(owner) = self.loader.load_model(prefix_type) {
                                                        if let Some((_, base)) = owner
                                                            .type_aliases
                                                            .iter()
                                                            .find(|(a, _)| a == suffix_type)
                                                        {
                                                            let base =
                                                                resolve_type_alias(&owner.type_aliases, base);
                                                            if is_primitive(&base) {
                                                                flat.declarations.push(Declaration {
                                                                    type_name: base.clone(),
                                                                    name: full_path.clone(),
                                                                    replaceable: decl.replaceable,
                                                                    constrainedby_type: decl.constrainedby_type.clone(),
                                                                    is_parameter: decl.is_parameter,
                                                                    is_flow: decl.is_flow,
                                                                    is_stream: decl.is_stream,
                                                                    is_discrete: decl.is_discrete,
                                                                    is_input: decl.is_input,
                                                                    is_output: decl.is_output,
                                                                    is_inner: decl.is_inner,
                                                                    is_outer: decl.is_outer,
                                                                    is_public: decl.is_public,
                                                                    is_protected: decl.is_protected,
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
                                                                    condition: None,
                                                                });
                                                                flat.register_inst_path(
                                                                    full_path.clone(),
                                                                    base,
                                                                    if is_array { Some(i as usize) } else { None },
                                                                );
                                                                continue;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if decl.is_parameter
                                        && (resolved_type.eq_ignore_ascii_case("distribution")
                                            || resolved_type.contains("PartialDistribution")
                                            || resolved_type.contains(".Distributions.Interfaces.")
                                            || resolved_type.ends_with(".Distribution"))
                                    {
                                        flat.declarations.push(Declaration {
                                            type_name: "Real".to_string(),
                                            name: full_path.clone(),
                                            replaceable: decl.replaceable,
                                            constrainedby_type: decl.constrainedby_type.clone(),
                                            is_parameter: decl.is_parameter,
                                            is_flow: decl.is_flow,
                                            is_stream: decl.is_stream,
                                            is_discrete: decl.is_discrete,
                                            is_input: decl.is_input,
                                            is_output: decl.is_output,
                                            is_inner: decl.is_inner,
                                            is_outer: decl.is_outer,
                                            is_public: decl.is_public,
                                            is_protected: decl.is_protected,
                                            start_value: if let Some(val) = &decl.start_value {
                                                Some(self.substitute(val, &context))
                                            } else {
                                                Some(Expression::Number(0.0))
                                            },
                                            array_size: None,
                                            modifications: Vec::new(),
                                            is_rest: decl.is_rest,
                                            annotation: None,
                                            condition: None,
                                        });
                                        flat.register_inst_path(
                                            full_path.clone(),
                                            "Real".to_string(),
                                            if is_array { Some(i as usize) } else { None },
                                        );
                                        continue;
                                    }
                                    return match e {
                                        LoadError::NotFound(_) => Err(FlattenError::UnknownType(
                                            resolved_type.clone(),
                                            full_path.clone(),
                                            loc,
                                        )),
                                        _ => Err(FlattenError::Load(e)),
                                    };
                                }
                            };

                            if let Some((_, base)) = sub_model
                                .type_aliases
                                .iter()
                                .find(|(a, _)| a == &sub_model.name)
                            {
                                let base = resolve_type_alias(&sub_model.type_aliases, base);
                                if is_primitive(&base) {
                                    flat.declarations.push(Declaration {
                                        type_name: base.clone(),
                                        name: full_path.clone(),
                                        replaceable: decl.replaceable,
                                        constrainedby_type: decl.constrainedby_type.clone(),
                                        is_parameter: decl.is_parameter,
                                        is_flow: decl.is_flow,
                                        is_stream: decl.is_stream,
                                        is_discrete: decl.is_discrete,
                                        is_input: decl.is_input,
                                        is_output: decl.is_output,
                                        is_inner: decl.is_inner,
                                        is_outer: decl.is_outer,
                                        is_public: decl.is_public,
                                        is_protected: decl.is_protected,
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
                                        condition: None,
                                    });
                                    flat.register_inst_path(
                                        full_path.clone(),
                                        base.clone(),
                                        if is_array { Some(i as usize) } else { None },
                                    );
                                    continue;
                                }
                            }

                            self.flatten_inheritance(&mut sub_model, &resolved_type)?;
                            let mod_ctx = ModifyContext::for_declaration_expand(
                                current_qualified,
                                &msl_import_context,
                                self.coarse_constrainedby_only,
                            );
                            for modification in &decl.modifications {
                                apply_modification_to_model(
                                    Arc::make_mut(&mut sub_model),
                                    modification,
                                    &mod_ctx,
                                    Some(&mut self.loader),
                                )?;
                            }

                            flat.instances.insert(full_path.clone(), resolved_type.clone());
                            flat.register_inst_path(
                                full_path.clone(),
                                resolved_type.clone(),
                                if is_array { Some(i as usize) } else { None },
                            );

                            if mode == ExpandDeclMode::DeclAndSubEq {
                                stack.push(Task::ExpandEquations {
                                    model: Arc::clone(&sub_model),
                                    prefix: full_path.clone(),
                                });
                            }
                            stack.push(Task::Process {
                                model: sub_model,
                                prefix: full_path,
                                current_model_name: Some(resolved_type),
                                msl_import_context: msl_import_context.clone(),
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
