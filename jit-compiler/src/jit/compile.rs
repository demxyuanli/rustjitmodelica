use crate::ast::*;
use cranelift::codegen::ir::UserFuncName;
use cranelift::prelude::{types as cl_types, *};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, FuncId, Linkage, Module};
use std::collections::{HashMap, HashSet};
use std::mem;

use super::analysis::{collect_modified, collect_modified_equations};
use super::clock_lowering::{compile_guarded_partition, emit_sample_trigger, jit_executable_allocation_len};
use super::codegen_cache;
use super::config::{
    compute_jit_compile_cache_key, jit_cache_variant_from_env, jit_opt_level_from_env,
    jit_verifier_dump_enabled, param_signature, type_profile_hash,
};
use super::context::TranslationContext;
use super::jit_policy;
use super::native::{builtin_jit_symbol_ptrs, register_symbols};
use super::object_emit::emit_object_cache_artifact;
use super::translator::expr::compile_expression;
use super::translator::vectorize;
use super::translator::{compile_algorithm_stmt, compile_equation};
use super::types::{ArrayInfo, CalcDerivsFunc};
use crate::compiler::{ClockPartitionScheduleEntry, ClockPartitionTrigger};

pub struct Jit {
    pub(super) builder_context: FunctionBuilderContext,
    pub(super) ctx: codegen::Context,
    #[allow(dead_code)]
    pub(super) data_ctx: DataDescription,
    pub(super) module: JITModule,
    pub(super) codegen_cache: Option<codegen_cache::CodegenCache>,
    pub(super) func_cache: HashMap<String, (CalcDerivsFunc, usize, usize)>,
    pub(super) disk_cache_live: Vec<codegen_cache::CachedFunction>,
    pub(super) runtime_symbols: HashMap<String, *const u8>,
    /// Current model identity used as the disk-cache key discriminator. Must be set by the
    /// caller before `compile` so that each model's codegen artifacts are stored under a
    /// model-specific key, preventing cross-model contamination when models share the same
    /// state / discrete / output var shapes.
    pub(super) active_model_name: Option<String>,
}

impl Jit {
    pub fn new() -> Self {
        Self::new_with_extra_symbols(None)
    }

    /// EXT-2: Create JIT with optional extra symbols (e.g. from --external-lib loaded libraries).
    pub fn new_with_extra_symbols(
        extra: Option<&std::collections::HashMap<String, *const u8>>,
    ) -> Self {
        let mut flag_builder = settings::builder();
        let _ = flag_builder.set("opt_level", &jit_opt_level_from_env());
        // Enable Cranelift auto-vectorization by default; manual SIMD in vectorize.rs
        // handles the primary path, but Cranelift's auto-vec helps scalar fallbacks.
        let enable_simd = std::env::var("RUSTMODLICA_CRANELIFT_ENABLE_SIMD")
            .ok()
            .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
            .unwrap_or(true);
        let _ = flag_builder.set("enable_simd", if enable_simd { "true" } else { "false" });
        // Enable jump tables for dense when/if chains (reduces code size).
        let _ = flag_builder.set("enable_jump_tables", "true");
        let isa_builder = cranelift_native::builder().unwrap();
        let isa = isa_builder.finish(settings::Flags::new(flag_builder)).unwrap();
        let mut builder =
            JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        let mut runtime_symbols = builtin_jit_symbol_ptrs();
        register_symbols(&mut builder);
        if let Some(map) = extra {
            for (name, ptr) in map {
                builder.symbol(name, *ptr);
                runtime_symbols.insert(name.clone(), *ptr);
            }
        }

        let module = JITModule::new(builder);

        let codegen_cache = if codegen_cache::codegen_cache_enabled() {
            Some(codegen_cache::CodegenCache::new())
        } else {
            None
        };

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            data_ctx: DataDescription::new(),
            module,
            codegen_cache,
            func_cache: HashMap::new(),
            disk_cache_live: Vec::new(),
            runtime_symbols,
            active_model_name: None,
        }
    }

    /// Set the active model identity for subsequent `compile` calls. This name is folded
    /// into the on-disk `flat_hash` and `CodegenCacheKey`, so distinct models cannot share
    /// a cache entry even when their state / discrete / param / output signatures coincide.
    pub fn set_active_model_name(&mut self, name: impl Into<String>) {
        let s = name.into();
        if s.is_empty() {
            self.active_model_name = None;
        } else {
            self.active_model_name = Some(s);
        }
    }

    fn codegen_cache_model_name(&self) -> &str {
        self.active_model_name.as_deref().unwrap_or("calc_derivs")
    }

    pub fn compile(
        &mut self,
        state_vars: &[String],
        discrete_vars: &[String],
        param_vars: &[String],
        output_vars: &[String],
        array_info: &HashMap<String, ArrayInfo>,
        alg_equations: &[Equation],
        diff_equations: &[Equation],
        algorithms: &[AlgorithmStatement],
        clock_partition_schedule: &[ClockPartitionScheduleEntry],
        _t_end: f64,
        param_values: &[f64],
        _newton_tearing_var_names: &[String],
        external_modelica_names: &HashSet<String>,
        const_fold_params: &[(String, f64)],
        stream_connection_set: &HashMap<String, Vec<String>>,
        stream_flow_map: &HashMap<String, String>,
        connector_connection_degree: &HashMap<String, usize>,
    ) -> Result<(CalcDerivsFunc, usize, usize), String> {
        let cache_key = compute_jit_compile_cache_key(
            state_vars,
            discrete_vars,
            param_vars,
            output_vars,
            array_info,
            alg_equations,
            diff_equations,
            algorithms,
            clock_partition_schedule,
            param_values,
            Some(connector_connection_degree),
        );

        if let Some((func, when_count, crossings_count)) = self.func_cache.get(&cache_key) {
            eprintln!(
                "[jit] FUNC_CACHE_HIT key_prefix={}... when={} crossings={}",
                &cache_key.chars().take(16).collect::<String>(),
                when_count,
                crossings_count
            );
            return Ok((*func, *when_count, *crossings_count));
        }

        let cache_variant = jit_cache_variant_from_env();
        let opt_level = jit_opt_level_from_env();
        let type_hash = type_profile_hash(param_values);
        let param_sig = param_signature(param_values);
        if let Some(ref cache) = self.codegen_cache {
            let active_model = self.codegen_cache_model_name().to_string();
            let flat_hash = codegen_cache::flat_model_hash(
                &active_model,
                state_vars,
                discrete_vars,
                param_vars,
                output_vars,
                array_info,
                &opt_level,
                &cache_variant,
                &type_hash,
                &param_sig,
                Some(connector_connection_degree),
            );
            let key = codegen_cache::CodegenCacheKey::new(
                &active_model,
                &flat_hash,
                &opt_level,
                &cache_variant,
                &type_hash,
                &param_sig,
                crate::cache::cache_scope::CacheScope::Project,
            );
            let runtime_param_map =
                codegen_cache::param_values_by_name(param_vars, param_values);
            if let Some(cached) =
                cache.get(&key, &self.runtime_symbols, Some(&runtime_param_map))
            {
                let when_count = cached.when_count;
                let crossings_count = cached.crossings_count;
                eprintln!(
                    "[jit] CODEGEN_CACHE_HIT key={} when={} crossings={}",
                    &key.stable_hash().chars().take(16).collect::<String>(),
                    when_count,
                    crossings_count
                );
                let func = cached.func;
                self.disk_cache_live.push(cached);
                self.func_cache
                    .insert(cache_key, (func, when_count, crossings_count));
                return Ok((func, when_count, crossings_count));
            }
        }

        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(cl_types::F64));
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type()));
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type()));
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type()));
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type()));
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type()));
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type()));
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type()));
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type()));
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type()));
        sig.params.push(AbiParam::new(cl_types::F64));
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type()));
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type()));
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type()));
        sig.returns.push(AbiParam::new(cl_types::I32));

        let func_id = self
            .module
            .declare_function("calc_derivs", Linkage::Export, &sig)
            .map_err(|e| e.to_string())?;

        self.ctx.func.signature = sig;
        self.ctx.func.name = UserFuncName::user(0, func_id.as_u32());

        let when_count;
        let crossings_count;

        {
            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
            let entry_block = builder.create_block();
            let epilogue_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            let time_val = builder.block_params(entry_block)[0];
            let states_ptr = builder.block_params(entry_block)[1];
            let discrete_ptr = builder.block_params(entry_block)[2];
            let derivs_ptr = builder.block_params(entry_block)[3];
            let params_ptr = builder.block_params(entry_block)[4];
            let outputs_ptr = builder.block_params(entry_block)[5];
            let when_states_ptr = builder.block_params(entry_block)[6];
            let crossings_ptr = builder.block_params(entry_block)[7];
            let pre_states_ptr = builder.block_params(entry_block)[8];
            let pre_discrete_ptr = builder.block_params(entry_block)[9];
            let t_end_val = builder.block_params(entry_block)[10];
            let diag_residual_ptr = builder.block_params(entry_block)[11];
            let diag_x_ptr = builder.block_params(entry_block)[12];
            let homotopy_lambda_ptr = builder.block_params(entry_block)[13];

            let (diag_res, diag_x) = (Some(diag_residual_ptr), Some(diag_x_ptr));

            let mut var_map = HashMap::new();
            var_map.insert("time".to_string(), time_val);
            var_map.insert("t_end".to_string(), t_end_val);

            for (i, name) in state_vars.iter().enumerate() {
                let offset = (i * 8) as i32;
                let val = builder
                    .ins()
                    .load(cl_types::F64, MemFlags::new(), states_ptr, offset);
                var_map.insert(name.clone(), val);
            }
            for (i, name) in discrete_vars.iter().enumerate() {
                let offset = (i * 8) as i32;
                let val = builder
                    .ins()
                    .load(cl_types::F64, MemFlags::new(), discrete_ptr, offset);
                var_map.insert(name.clone(), val);
            }
            for (i, name) in param_vars.iter().enumerate() {
                let offset = (i * 8) as i32;
                let val = builder
                    .ins()
                    .load(cl_types::F64, MemFlags::new(), params_ptr, offset);
                var_map.insert(name.clone(), val);
            }

            let mut stack_slots = HashMap::new();
            let mut modified_vars = HashSet::new();
            for stmt in algorithms {
                collect_modified(stmt, &mut modified_vars);
            }
            collect_modified_equations(alg_equations, &mut modified_vars);
            collect_modified_equations(diff_equations, &mut modified_vars);

            for var in &modified_vars {
                let slot =
                    builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
                        cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
                        8,
                        0,
                    ));
                stack_slots.insert(var.clone(), slot);
                if let Some(val) = var_map.get(var) {
                    builder.ins().stack_store(*val, slot, 0);
                } else {
                    let zero = builder.ins().f64const(0.0);
                    builder.ins().stack_store(zero, slot, 0);
                }
            }

            let state_var_index: HashMap<String, usize> = state_vars
                .iter()
                .enumerate()
                .map(|(i, s)| (s.clone(), i))
                .collect();
            let discrete_var_index: HashMap<String, usize> = discrete_vars
                .iter()
                .enumerate()
                .map(|(i, s)| (s.clone(), i))
                .collect();
            let param_var_index: HashMap<String, usize> = param_vars
                .iter()
                .enumerate()
                .map(|(i, s)| (s.clone(), i))
                .collect();
            let output_var_index: HashMap<String, usize> = output_vars
                .iter()
                .enumerate()
                .map(|(i, s)| (s.clone(), i))
                .collect();

            let mut when_idx = 0;
            let mut crossings_idx = 0;
            let mut declared_imports: HashMap<String, HashMap<String, FuncId>> = HashMap::new();
            let mut string_literal_cache = HashMap::new();
            let mut string_data_counter = 0usize;

            let speculation_guard_ids: Vec<u32> = {
                let mut ids = Vec::new();
                if let Ok(spec_reg) = crate::jit::speculation::global_registry().read() {
                    for (real_id, _kind) in spec_reg.active_speculations_with_ids() {
                        ids.push(real_id);
                    }
                }
                ids
            };

            let speculation_guard_fn_ref = if !speculation_guard_ids.is_empty() {
                let guard_sig = {
                    let mut s = self.module.make_signature();
                    s.params.push(AbiParam::new(cl_types::I32));
                    s.returns.push(AbiParam::new(cl_types::I32));
                    s
                };
                if let Ok(guard_fn_id) = self.module.declare_function(
                    "speculation_holds",
                    Linkage::Import,
                    &guard_sig,
                ) {
                    Some(
                        self.module
                            .declare_func_in_func(guard_fn_id, builder.func),
                    )
                } else {
                    None
                }
            } else {
                None
            };

            let mut t_ctx = TranslationContext::new(
                &mut self.module,
                &mut var_map,
                &stack_slots,
                array_info,
                states_ptr,
                discrete_ptr,
                params_ptr,
                outputs_ptr,
                derivs_ptr,
                pre_states_ptr,
                pre_discrete_ptr,
                when_states_ptr,
                crossings_ptr,
                &mut when_idx,
                &mut crossings_idx,
                state_vars,
                discrete_vars,
                output_vars,
                &state_var_index,
                &discrete_var_index,
                &param_var_index,
                &output_var_index,
                diag_res,
                diag_x,
                homotopy_lambda_ptr,
                Some(&mut declared_imports),
                Some(&mut string_literal_cache),
                Some(&mut self.data_ctx),
                Some(&mut string_data_counter),
                Some(external_modelica_names),
                Some(epilogue_block),
                connector_connection_degree,
                stream_connection_set,
                stream_flow_map,
            );

            if let Some(guard_fn_ref) = speculation_guard_fn_ref {
                if speculation_guard_ids.len() == 1 {
                    let after_guards = builder.create_block();
                    let next_guard = builder.create_block();
                    let id_val =
                        builder.ins().iconst(cl_types::I32, speculation_guard_ids[0] as i64);
                    let call_inst = builder.ins().call(guard_fn_ref, &[id_val]);
                    let ret = builder.inst_results(call_inst)[0];
                    let zero = builder.ins().iconst(cl_types::I32, 0);
                    let failed = builder.ins().icmp(IntCC::Equal, ret, zero);
                    builder.ins().brif(failed, after_guards, &[], next_guard, &[]);
                    builder.switch_to_block(next_guard);
                    builder.seal_block(next_guard);
                    builder.ins().jump(after_guards, &[]);
                    builder.switch_to_block(after_guards);
                    builder.seal_block(after_guards);
                } else if speculation_guard_ids.len() > 1 {
                    let after_guards = builder.create_block();
                    for &guard_id in &speculation_guard_ids {
                        let id_val = builder.ins().iconst(cl_types::I32, guard_id as i64);
                        let call_inst = builder.ins().call(guard_fn_ref, &[id_val]);
                        let ret = builder.inst_results(call_inst)[0];
                        let zero = builder.ins().iconst(cl_types::I32, 0);
                        let failed = builder.ins().icmp(IntCC::Equal, ret, zero);
                        let next_guard = builder.create_block();
                        builder.ins().brif(failed, after_guards, &[], next_guard, &[]);
                        builder.switch_to_block(next_guard);
                        builder.seal_block(next_guard);
                    }
                    builder.ins().jump(after_guards, &[]);
                    builder.switch_to_block(after_guards);
                    builder.seal_block(after_guards);
                }
            }

            let mut covered_algorithms = HashSet::new();
            let mut covered_alg_equations = HashSet::new();
            let mut covered_diff_equations = HashSet::new();

            for entry in clock_partition_schedule {
                covered_algorithms.extend(entry.algorithm_indices.iter().copied());
                covered_alg_equations.extend(entry.alg_equation_indices.iter().copied());
                covered_diff_equations.extend(entry.diff_equation_indices.iter().copied());

                match entry.trigger {
                    ClockPartitionTrigger::Always => {
                        for idx in &entry.algorithm_indices {
                            if let Some(stmt) = algorithms.get(*idx) {
                                compile_algorithm_stmt(stmt, &mut t_ctx, &mut builder)?;
                            }
                        }
                        let alg_eqs: Vec<crate::ast::Equation> = entry
                            .alg_equation_indices
                            .iter()
                            .filter_map(|idx| alg_equations.get(*idx).cloned())
                            .collect();
                        Self::compile_equation_group(&alg_eqs, &mut t_ctx, &mut builder)?;
                        let diff_eqs: Vec<crate::ast::Equation> = entry
                            .diff_equation_indices
                            .iter()
                            .filter_map(|idx| diff_equations.get(*idx).cloned())
                            .collect();
                        Self::compile_equation_group(&diff_eqs, &mut t_ctx, &mut builder)?;
                    }
                    ClockPartitionTrigger::Sample { start, interval } => {
                        let trigger_val = emit_sample_trigger(start, interval, &mut t_ctx, &mut builder)?;
                        compile_guarded_partition(
                            trigger_val,
                            &mut t_ctx,
                            &mut builder,
                            algorithms,
                            alg_equations,
                            diff_equations,
                            entry,
                        )?;
                    }
                }
            }

            for (idx, stmt) in algorithms.iter().enumerate() {
                if !covered_algorithms.contains(&idx) {
                    compile_algorithm_stmt(stmt, &mut t_ctx, &mut builder)?;
                }
            }

            let uncovered_alg: Vec<crate::ast::Equation> = alg_equations
                .iter()
                .enumerate()
                .filter(|(idx, _)| !covered_alg_equations.contains(idx))
                .map(|(_, eq)| eq.clone())
                .collect();
            Self::compile_equation_group(&uncovered_alg, &mut t_ctx, &mut builder)?;
            let uncovered_diff: Vec<crate::ast::Equation> = diff_equations
                .iter()
                .enumerate()
                .filter(|(idx, _)| !covered_diff_equations.contains(idx))
                .map(|(_, eq)| eq.clone())
                .collect();
            Self::compile_equation_group(&uncovered_diff, &mut t_ctx, &mut builder)?;

            when_count = *t_ctx.when_idx;
            crossings_count = *t_ctx.crossings_idx;

            builder.ins().jump(epilogue_block, &[]);
            builder.switch_to_block(epilogue_block);
            for (var_name, slot) in &stack_slots {
                if let Some(&idx) = discrete_var_index.get(var_name) {
                    let val = builder.ins().stack_load(cl_types::F64, *slot, 0);
                    let offset = (idx * 8) as i32;
                    builder
                        .ins()
                        .store(MemFlags::new(), val, discrete_ptr, offset);
                }
            }

            let success_code = builder.ins().iconst(cl_types::I32, 0);
            builder.ins().return_(&[success_code]);
            builder.seal_all_blocks();
            builder.finalize();
        }

        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| {
                if jit_verifier_dump_enabled() {
                    eprintln!("[jit-verifier] define_function failed: {:?}", e);
                    eprintln!(
                        "[jit-verifier] function-ir-begin\n{}\n[jit-verifier] function-ir-end",
                        self.ctx.func.display()
                    );
                }
                format!("{:?}", e)
            })?;
        let disk_alloc_len = jit_executable_allocation_len(&self.ctx);
        let ir_for_object_cache = self.ctx.func.clone();
        self.module.clear_context(&mut self.ctx);
        self.module
            .finalize_definitions()
            .map_err(|e| e.to_string())?;

        let code = self.module.get_finalized_function(func_id);
        // SAFETY: code is a finalized JIT function pointer returned by
        // Cranelift's get_finalized_function, guaranteed to point to
        // executable memory containing a valid calc_derivs implementation.
        let func: CalcDerivsFunc = unsafe { mem::transmute(code) };

        eprintln!(
            "[jit] FUNC_CACHE_MISS key_prefix={}... when={} crossings={}",
            &cache_key.chars().take(16).collect::<String>(),
            when_count,
            crossings_count
        );
        self.func_cache.insert(cache_key.clone(), (func, when_count, crossings_count));

        if let Some(ref cache) = self.codegen_cache {
            if jit_policy::allow_codegen_disk_put(alg_equations.len(), diff_equations.len()) {
                let active_model = self.codegen_cache_model_name().to_string();
                let flat_hash = codegen_cache::flat_model_hash(
                    &active_model,
                    state_vars,
                    discrete_vars,
                    param_vars,
                    output_vars,
                    array_info,
                    &opt_level,
                    &cache_variant,
                    &type_hash,
                    &param_sig,
                    Some(connector_connection_degree),
                );
                let key = codegen_cache::CodegenCacheKey::new(
                    &active_model,
                    &flat_hash,
                    &opt_level,
                    &cache_variant,
                    &type_hash,
                    &param_sig,
                    crate::cache::cache_scope::CacheScope::Project,
                );
                let build_obj =
                    std::panic::catch_unwind(|| emit_object_cache_artifact(&ir_for_object_cache));
                match build_obj {
                    Ok(Ok(obj)) => {
                        if let Err(e) = cache.put_object(
                            &key,
                            &obj,
                            when_count,
                            crossings_count,
                            const_fold_params,
                        ) {
                            eprintln!("[jit] CODEGEN_CACHE_OBJECT_WRITE_ERROR: {}", e);
                        } else {
                            eprintln!(
                                "[jit] CODEGEN_CACHE_OBJECT_WRITE key_prefix={}... size={} bytes",
                                &key.stable_hash().chars().take(16).collect::<String>(),
                                obj.len()
                            );
                        }
                    }
                    Ok(Err(e)) => {
                        eprintln!("[jit] CODEGEN_CACHE_OBJECT_BUILD_ERROR: {}", e);
                        if let Some(func_alloc_len) = disk_alloc_len.filter(|n| *n > 0) {
                            // SAFETY: func_alloc_len is the allocation length reported by
                            // Cranelift for this compiled function. The memory at code is
                            // valid for reads within this length.
                            let code_bytes =
                                unsafe { std::slice::from_raw_parts(code, func_alloc_len) };
                            if let Err(pe) = cache.put(
                                &key,
                                code_bytes,
                                0,
                                func_alloc_len,
                                when_count,
                                crossings_count,
                                const_fold_params,
                            ) {
                                eprintln!("[jit] CODEGEN_CACHE_RAW_FALLBACK_WRITE_ERROR: {}", pe);
                            } else {
                                eprintln!(
                                    "[jit] CODEGEN_CACHE_RAW_FALLBACK_WRITE size={} bytes",
                                    code_bytes.len()
                                );
                            }
                        } else {
                            eprintln!(
                                "[jit] CODEGEN_CACHE_RAW_FALLBACK_SKIPPED missing compiled_code size (JIT internal)"
                            );
                        }
                    }
                    Err(_) => {
                        eprintln!("[jit] CODEGEN_CACHE_OBJECT_BUILD_PANIC");
                        if let Some(func_alloc_len) = disk_alloc_len.filter(|n| *n > 0) {
                            // SAFETY: func_alloc_len is the allocation length reported by
                            // Cranelift for this compiled function. The memory at code is
                            // valid for reads within this length.
                            let code_bytes =
                                unsafe { std::slice::from_raw_parts(code, func_alloc_len) };
                            if let Err(pe) = cache.put(
                                &key,
                                code_bytes,
                                0,
                                func_alloc_len,
                                when_count,
                                crossings_count,
                                const_fold_params,
                            ) {
                                eprintln!("[jit] CODEGEN_CACHE_RAW_FALLBACK_WRITE_ERROR: {}", pe);
                            } else {
                                eprintln!(
                                    "[jit] CODEGEN_CACHE_RAW_FALLBACK_WRITE size={} bytes",
                                    code_bytes.len()
                                );
                            }
                        } else {
                            eprintln!(
                                "[jit] CODEGEN_CACHE_RAW_FALLBACK_SKIPPED missing compiled_code size (JIT internal)"
                            );
                        }
                    }
                }
            }
        }

        Ok((func, when_count, crossings_count))
    }

    /// Check whether SIMD vectorization is enabled via environment variable.
    /// Enabled by default; set `RUSTMODLICA_JIT_SIMD=0` to disable.
    fn simd_enabled() -> bool {
        std::env::var("RUSTMODLICA_JIT_SIMD")
            .ok()
            .map(|v| !matches!(v.trim(), "0" | "false" | "FALSE" | "off" | "OFF"))
            .unwrap_or(true)
    }

    /// Compile a group of equations with optional SIMD vectorization.
    /// Equations are clustered into vector groups when possible and the
    /// resulting [`vectorize::CompileUnit`]s are emitted.
    fn compile_equation_group(
        equations: &[crate::ast::Equation],
        ctx: &mut crate::jit::context::TranslationContext<'_>,
        builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<(), String> {
        if !Self::simd_enabled() || equations.len() < 4 {
            for eq in equations {
                compile_equation(eq, ctx, builder)?;
            }
            return Ok(());
        }

        let units = vectorize::cluster_equations(equations);
        for unit in units {
            match unit {
                vectorize::CompileUnit::Scalar(eq) => {
                    compile_equation(&eq, ctx, builder)?;
                }
                vectorize::CompileUnit::Vector(group) => {
                    if vectorize::emit_vector_loop(&group, ctx, builder).is_err() {
                        // SIMD emission failed; fallback to scalar for this group
                        for i in group.lo..=group.hi {
                            // Find the equation matching this index
                            let idx_opt = equations.iter().position(|eq| {
                                matches!(
                                    eq,
                                    crate::ast::Equation::Simple(
                                        crate::ast::Expression::Variable(v),
                                        _,
                                    ) if {
                                        let name = crate::string_intern::resolve_id(*v);
                                        name == format!("{}_{}", group.dst_base, i)
                                    }
                                )
                            });
                            if let Some(idx) = idx_opt {
                                compile_equation(&equations[idx], ctx, builder)?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn clear_func_cache(&mut self) {
        let count = self.func_cache.len();
        self.func_cache.clear();
        self.disk_cache_live.clear();
        if count > 0 {
            eprintln!("[jit] cleared {} cached function(s)", count);
        }
    }

    pub fn func_cache_stats(&self) -> (usize, usize) {
        (self.func_cache.len(), self.func_cache.values().map(|(_, w, c)| w + c).sum())
    }

    pub fn compile_user_function_stub(
        &mut self,
        name: &str,
        input_names: &[String],
        output_expr: &Expression,
    ) -> Result<*const u8, String> {
        let n = input_names.len();
        let mut sig = self.module.make_signature();
        for _ in 0..n {
            sig.params.push(AbiParam::new(cl_types::F64));
        }
        sig.returns.push(AbiParam::new(cl_types::F64));

        let func_id = self
            .module
            .declare_function(name, Linkage::Export, &sig)
            .map_err(|e| e.to_string())?;

        self.ctx.func.signature = sig;
        self.ctx.func.name = UserFuncName::user(0, func_id.as_u32());

        let mut var_map = HashMap::new();
        let stack_slots = HashMap::new();
        let array_info = HashMap::new();
        let state_var_index = HashMap::new();
        let discrete_var_index = HashMap::new();
        let param_var_index = HashMap::new();
        let output_var_index = HashMap::new();
        let state_vars: &[String] = &[];
        let discrete_vars: &[String] = &[];
        let output_vars: &[String] = &[];
        let mut when_idx = 0usize;
        let mut crossings_idx = 0usize;
        let mut string_literal_cache = HashMap::new();
        let mut string_data_counter = 0usize;
        let empty_stream_connection_set: HashMap<String, Vec<String>> = HashMap::new();
        let empty_stream_flow_map: HashMap<String, String> = HashMap::new();
        let empty_connector_degree: HashMap<String, usize> = HashMap::new();

        {
            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            let params = builder.block_params(entry_block);
            for (i, in_name) in input_names.iter().enumerate() {
                if i < params.len() {
                    var_map.insert(in_name.clone(), params[i]);
                }
            }

            let null_ptr = builder.ins().iconst(cl_types::I64, 0);
            let ptr_val = null_ptr;

            let mut t_ctx = TranslationContext::new(
                &mut self.module,
                &mut var_map,
                &stack_slots,
                &array_info,
                ptr_val,
                ptr_val,
                ptr_val,
                ptr_val,
                ptr_val,
                ptr_val,
                ptr_val,
                ptr_val,
                ptr_val,
                &mut when_idx,
                &mut crossings_idx,
                state_vars,
                discrete_vars,
                output_vars,
                &state_var_index,
                &discrete_var_index,
                &param_var_index,
                &output_var_index,
                None,
                None,
                ptr_val,
                None,
                Some(&mut string_literal_cache),
                Some(&mut self.data_ctx),
                Some(&mut string_data_counter),
                None,
                None,
                &empty_connector_degree,
                &empty_stream_connection_set,
                &empty_stream_flow_map,
            );

            let result = compile_expression(output_expr, &mut t_ctx, &mut builder)?;
            builder.ins().return_(&[result]);
            builder.finalize();
        }

        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| format!("{:?}", e))?;
        self.module.clear_context(&mut self.ctx);
        self.module
            .finalize_definitions()
            .map_err(|e| e.to_string())?;

        let code = self.module.get_finalized_function(func_id);
        Ok(code)
    }
}
