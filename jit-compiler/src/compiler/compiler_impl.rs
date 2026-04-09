/// Deferred artifact write data — stored here until confirm_artifact_stored() is called.
pub(crate) struct DeferredArtifactWrite {
    pub cache_root: std::path::PathBuf,
    pub key: String,
    pub bundle: crate::cache::artifact_bundle::CompiledArtifactBundle,
}

impl Compiler {
    fn function_has_output_in_hierarchy(
        &mut self,
        model: &crate::ast::Model,
        current_qualified: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if model.declarations.iter().any(|d| d.is_output) {
            return Ok(true);
        }
        for clause in &model.extends {
            let base_name =
                Flattener::resolve_import_prefix(model, &clause.model_name, current_qualified);
            let base_name = Flattener::qualify_in_scope(current_qualified, &base_name);
            let base_model = self
                .loader
                .load_model(&base_name)
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
            if self.function_has_output_in_hierarchy(base_model.as_ref(), &base_name)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn new() -> Self {
        Compiler {
            loader: ModelLoader::new(),
            options: CompilerOptions::default(),
            warnings: Vec::new(),
            external_libraries: ExternalLibs::default(),
            external_symbol_ptrs: HashMap::new(),
            interner: crate::string_intern::StringInterner::new(),
            last_compile_perf: None,
            last_provenance_index: None,
            deferred_artifact: None,
        }
    }

    pub fn take_warnings(&mut self) -> Vec<WarningInfo> {
        std::mem::take(&mut self.warnings)
    }

    pub fn take_compile_perf_report(&mut self) -> Option<CompilePerfReport> {
        self.last_compile_perf.take()
    }

    /// Compile a model from source code in memory (for IDE / single-file). Caller may add_path
    /// for StandardLib/TestLib before this if the model has dependencies.
    pub fn compile_from_source(
        &mut self,
        model_name: &str,
        code: &str,
    ) -> Result<CompileOutput, Box<dyn std::error::Error + Send + Sync>> {
        self.loader
            .load_model_from_source(model_name, code)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        self.compile(model_name)
    }

    /// Build equation/variable dependency graph from source (for analysis/debug). Does not run full compile.
    pub fn get_equation_graph_from_source(
        &mut self,
        model_name: &str,
        code: &str,
        mode: EquationGraphMode,
    ) -> Result<equation_graph::EquationGraph, Box<dyn std::error::Error + Send + Sync>> {
        self.loader
            .load_model_from_source(model_name, code)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        let mut root_model = self
            .loader
            .load_model(model_name)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        if root_model.as_ref().is_function {
            return Err("Equation graph is not supported for functions.".into());
        }
        if matches!(mode, EquationGraphMode::Structural) {
            return Ok(equation_graph::build_structural_graph(root_model.as_ref()));
        }
        let stage_trace = stage_trace_enabled();
        let snap_path = self
            .options
            .emit_flat_snapshot
            .as_deref()
            .map(std::path::Path::new);
        let array_sizes_path = self
            .options
            .array_sizes_json
            .as_deref()
            .map(std::path::Path::new);
        let array_size_policy = ArraySizePolicy::parse(self.options.array_size_policy.as_str());
        let stage = flatten_and_inline(
            &mut root_model,
            model_name,
            &mut self.loader,
            self.options.compile_stop.clone(),
            false,
            self.options.quiet,
            stage_trace,
            snap_path,
            self.options.coarse_constrainedby_only,
            crate::flatten::ValidationMode::parse(self.options.validation_mode.as_str()),
            array_size_policy,
            array_sizes_path,
            self.options.warnings_level.as_str(),
        )?;
        self.last_provenance_index = Some(stage.provenance_index.clone());
        let flat_model = stage.flat_model;
        Ok(equation_graph::build_equation_graph(&flat_model, mode))
    }

    /// Run a function once with given inputs (or 0.0 per input if not provided) and return the output (F3-1).
    fn run_function_once(
        &mut self,
        model_name: &str,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        let mut root_model = self
            .loader
            .load_model(model_name)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        if !root_model.extends.is_empty() {
            let mut flattener = Flattener::new();
            flattener.loader.library_paths = self.loader.library_paths.clone();
            if let Some(p) = self.loader.get_path_for_model(model_name) {
                flattener.loader.register_path(model_name, p);
            }
            flattener
                .flatten_inheritance(&mut root_model, model_name)
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        }
        if let Some((input_names, outputs)) = inline::get_function_body(root_model.as_ref()) {
            let body = outputs
                .first()
                .ok_or("Function has no output expression.")?
                .1
                .clone();
            let args = self.options.function_args.as_deref().unwrap_or(&[]);
            let mut vars = HashMap::new();
            for (i, name) in input_names.iter().enumerate() {
                let val = args.get(i).copied().unwrap_or(0.0);
                vars.insert(name.clone(), val);
            }
            return expr_eval::eval_expr(&body, &vars).map_err(|e| e.into());
        }
        if self.options.quiet {
            return Ok(0.0);
        }
        if self.function_has_output_in_hierarchy(root_model.as_ref(), model_name)? {
            return Ok(0.0);
        }
        if root_model.external_info.is_some() {
            return Ok(0.0);
        }
        Err("Function must have at least one output and assignments in algorithm.".into())
    }

    pub fn compile(
        &mut self,
        model_name: &str,
    ) -> Result<CompileOutput, Box<dyn std::error::Error + Send + Sync>> {
        compile_model::compile(self, model_name)
    }

    /// Reuse a compiled system with a new runtime parameter vector without recompiling native code.
    pub fn reuse_compiled_with_new_params(
        &self,
        mut artifacts: Artifacts,
        new_params: Vec<f64>,
    ) -> Artifacts {
        artifacts.params = new_params;
        artifacts.param_only_update = true;
        artifacts
    }

    pub fn options_mut(&mut self) -> &mut CompilerOptions {
        &mut self.options
    }

    /// L1-T06: Call after successful execution validation to persist the artifact bundle.
    /// In strict mode (`RUSTMODLICA_ARTIFACT_DEFERRED_WRITE=1`, default), the artifact
    /// is not written to SQLite until this method is called. In immediate mode, this is
    /// a no-op since the artifact was already written during compile().
    pub fn confirm_artifact_stored(&mut self, _model_name: &str) -> Result<(), String> {
        if let Some(deferred) = self.deferred_artifact.take() {
            crate::cache::artifact_cache::put(
                deferred.cache_root.as_path(),
                &deferred.key,
                &deferred.bundle,
            )?;
            eprintln!(
                "[artifact] deferred write confirmed and flushed for key={}",
                &deferred.key.chars().take(40).collect::<String>()
            );
        }
        Ok(())
    }

    /// DBG-4: suffix for error messages (file path or model name).
    fn source_loc_suffix(&self, model_name: &str) -> String {
        self.loader
            .get_path_for_model(model_name)
            .map(|p| format!("\n  --> {}", p.display()))
            .unwrap_or_else(|| format!(" (model: {})", model_name))
    }
}
