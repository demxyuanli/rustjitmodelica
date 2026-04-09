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
            .unwrap_or(128)
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
            interner: StringInterner::new(),
            inst_records: Vec::new(),
            path_to_inst: HashMap::new(),
        };
        self.expand_declarations_with_mode(
            root,
            "",
            &mut flat,
            Some(root_path.as_str()),
            crate::flatten::decl_expand::ExpandDeclMode::DeclOnly,
        )?;
        Ok(flat)
    }

    pub(crate) fn eq_expand_root_preinherited(
        &mut self,
        root: &Model,
        flat: &mut FlattenedModel,
    ) {
        self.expand_equations(root, "", flat);
        self.expand_algorithms(root, "", flat);
        self.expand_initial_equations(root, "", flat);
        if matches!(self.validation_mode, ValidationMode::Full) {
            self.expand_initial_algorithms(root, "", flat);
        }
    }

    pub(crate) fn infer_clocked_variables_preinherited(&self, flat: &mut FlattenedModel) {
        self.infer_clocked_variables(flat);
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
        let instances = &flat.instances;
        let parallel_enabled = self.flatten_eq_parallel_enabled()
            && prefix.is_empty()
            && model.equations.len() >= self.flatten_eq_parallel_min_items();
        if parallel_enabled {
            crate::query_db::perf_record_add("flatten_parallel_poc_enabled", 1);
            let library_paths = self.loader.library_paths.clone();
            let quiet = self.loader.quiet;
            let array_sizes = flat.array_sizes.clone();
            let instances_cloned = instances.clone();
            let base_context_stack = context_stack.clone();
            let micro_batch = self.flatten_eq_parallel_micro_batch_size();
            let micro_budget = self.flatten_eq_parallel_micro_batch_budget();
            let bucket_count = rayon::current_num_threads().max(1);
            let buckets = Self::balanced_buckets_by_weight(
                &model.equations,
                Self::equation_complexity,
                bucket_count,
            );
            let mut partials: Vec<(usize, Vec<(usize, Equation)>, Vec<(usize, AlgorithmStatement)>, Vec<(usize, (String, String))>, Vec<(usize, (Expression, (String, String)))>)> = buckets
                .into_par_iter()
                .enumerate()
                .map(|(bucket_idx, bucket)| {
                    let mut local = self.clone_for_parallel_expand();
                    local.loader.set_quiet(quiet);
                    for p in &library_paths {
                        local.loader.add_path(p.clone());
                    }
                    let mut eq_out_idx = Vec::new();
                    let mut alg_out_idx = Vec::new();
                    let mut conn_out_idx = Vec::new();
                    let mut cconn_out_idx = Vec::new();
                    let batches = Self::adaptive_batches_by_budget(
                        &bucket,
                        Self::equation_complexity,
                        micro_batch,
                        micro_budget,
                    );
                    for batch in batches {
                        let mut eq_out = Vec::new();
                        let mut alg_out = Vec::new();
                        let mut conn_out = Vec::new();
                        let mut cconn_out = Vec::new();
                        let mut target = ExpandTarget {
                            equations: &mut eq_out,
                            algorithms: &mut alg_out,
                            connections: &mut conn_out,
                            conditional_connections: &mut cconn_out,
                            array_sizes: &array_sizes,
                        };
                        let mut local_context_stack = base_context_stack.clone();
                        let single: Vec<Equation> = batch.iter().map(|(_, eq)| eq.clone()).collect();
                        local.expand_equation_list(
                            &single,
                            prefix,
                            &mut target,
                            &mut local_context_stack,
                            &instances_cloned,
                            None,
                        );
                        let mut fallback_idx = batch[0].0;
                        for (k, v) in eq_out.into_iter().enumerate() {
                            if k < batch.len() {
                                fallback_idx = batch[k].0;
                            }
                            eq_out_idx.push((fallback_idx, v));
                        }
                        fallback_idx = batch[0].0;
                        for (k, v) in alg_out.into_iter().enumerate() {
                            if k < batch.len() {
                                fallback_idx = batch[k].0;
                            }
                            alg_out_idx.push((fallback_idx, v));
                        }
                        fallback_idx = batch[0].0;
                        for (k, v) in conn_out.into_iter().enumerate() {
                            if k < batch.len() {
                                fallback_idx = batch[k].0;
                            }
                            conn_out_idx.push((fallback_idx, v));
                        }
                        fallback_idx = batch[0].0;
                        for (k, v) in cconn_out.into_iter().enumerate() {
                            if k < batch.len() {
                                fallback_idx = batch[k].0;
                            }
                            cconn_out_idx.push((fallback_idx, v));
                        }
                    }
                    (bucket_idx, eq_out_idx, alg_out_idx, conn_out_idx, cconn_out_idx)
                })
                .collect();
            partials.sort_by_key(|x| x.0);
            let mut eq_all = Vec::new();
            let mut alg_all = Vec::new();
            let mut conn_all = Vec::new();
            let mut cconn_all = Vec::new();
            for (_, eqs, algs, conns, cconns) in partials {
                eq_all.extend(eqs);
                alg_all.extend(algs);
                conn_all.extend(conns);
                cconn_all.extend(cconns);
            }
            eq_all.sort_by_key(|(idx, _)| *idx);
            alg_all.sort_by_key(|(idx, _)| *idx);
            conn_all.sort_by_key(|(idx, _)| *idx);
            cconn_all.sort_by_key(|(idx, _)| *idx);
            flat.equations.extend(eq_all.into_iter().map(|(_, v)| v));
            flat.algorithms.extend(alg_all.into_iter().map(|(_, v)| v));
            flat.connections.extend(conn_all.into_iter().map(|(_, v)| v));
            flat.conditional_connections
                .extend(cconn_all.into_iter().map(|(_, v)| v));
        } else {
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
                instances,
                None,
            );
        }
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
        let instances = &flat.instances;
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
            instances,
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
        let parallel_enabled = self.flatten_eq_parallel_enabled()
            && prefix.is_empty()
            && model.algorithms.len() >= self.flatten_eq_parallel_min_items();
        if parallel_enabled {
            crate::query_db::perf_record_add("flatten_parallel_poc_enabled", 1);
            let library_paths = self.loader.library_paths.clone();
            let quiet = self.loader.quiet;
            let array_sizes = flat.array_sizes.clone();
            let base_context_stack = context_stack.clone();
            let micro_batch = self.flatten_eq_parallel_micro_batch_size();
            let micro_budget = self.flatten_eq_parallel_micro_batch_budget();
            let bucket_count = rayon::current_num_threads().max(1);
            let buckets = Self::balanced_buckets_by_weight(
                &model.algorithms,
                Self::algorithm_complexity,
                bucket_count,
            );
            let mut partials: Vec<(usize, Vec<(usize, Equation)>, Vec<(usize, AlgorithmStatement)>)> = buckets
                .into_par_iter()
                .enumerate()
                .map(|(bucket_idx, bucket)| {
                    let mut local = self.clone_for_parallel_expand();
                    local.loader.set_quiet(quiet);
                    for p in &library_paths {
                        local.loader.add_path(p.clone());
                    }
                    let mut eq_out_idx = Vec::new();
                    let mut alg_out_idx = Vec::new();
                    let batches = Self::adaptive_batches_by_budget(
                        &bucket,
                        Self::algorithm_complexity,
                        micro_batch,
                        micro_budget,
                    );
                    for batch in batches {
                        let mut eq_out = Vec::new();
                        let mut alg_out = Vec::new();
                        let mut conn_out = Vec::new();
                        let mut cconn_out = Vec::new();
                        let mut target = ExpandTarget {
                            equations: &mut eq_out,
                            algorithms: &mut alg_out,
                            connections: &mut conn_out,
                            conditional_connections: &mut cconn_out,
                            array_sizes: &array_sizes,
                        };
                        let mut local_context_stack = base_context_stack.clone();
                        let single: Vec<AlgorithmStatement> =
                            batch.iter().map(|(_, stmt)| stmt.clone()).collect();
                        local.expand_algorithm_list(
                            &single,
                            prefix,
                            &mut target,
                            &mut local_context_stack,
                        );
                        let mut fallback_idx = batch[0].0;
                        for (k, v) in eq_out.into_iter().enumerate() {
                            if k < batch.len() {
                                fallback_idx = batch[k].0;
                            }
                            eq_out_idx.push((fallback_idx, v));
                        }
                        fallback_idx = batch[0].0;
                        for (k, v) in alg_out.into_iter().enumerate() {
                            if k < batch.len() {
                                fallback_idx = batch[k].0;
                            }
                            alg_out_idx.push((fallback_idx, v));
                        }
                    }
                    (bucket_idx, eq_out_idx, alg_out_idx)
                })
                .collect();
            partials.sort_by_key(|x| x.0);
            let mut eq_all = Vec::new();
            let mut alg_all = Vec::new();
            for (_, eqs, algs) in partials {
                eq_all.extend(eqs);
                alg_all.extend(algs);
            }
            eq_all.sort_by_key(|(idx, _)| *idx);
            alg_all.sort_by_key(|(idx, _)| *idx);
            flat.equations.extend(eq_all.into_iter().map(|(_, v)| v));
            flat.algorithms.extend(alg_all.into_iter().map(|(_, v)| v));
        } else {
            let mut target = ExpandTarget {
                equations: &mut flat.equations,
                algorithms: &mut flat.algorithms,
                connections: &mut flat.connections,
                conditional_connections: &mut flat.conditional_connections,
                array_sizes: &flat.array_sizes,
            };
            self.expand_algorithm_list(&model.algorithms, prefix, &mut target, &mut context_stack);
        }
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

