use cranelift::codegen::ir::UserFuncName;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{FuncId, Linkage, Module};
use std::collections::HashMap;

use super::compile::{Jit, StandaloneBlock};
use super::context::TranslationContext;
use super::translator::expr::compile_expression;
use super::types::CalcDerivsFunc;
use crate::ast::Expression;

impl Jit {
    /// Compile deferred solvable blocks into their own Cranelift functions.
    /// Called after the main calc_derivs function is finalized but before
    /// finalize_definitions(), so all functions are in the same module.
    ///
    /// When RUSTMODLICA_BLOCK_COMPILE=1, this enables per-block recompilation
    /// and hot-swapping in future tier-up / deopt cycles.
    pub fn compile_deferred_blocks(
        &mut self,
        deferred: &[(
            FuncId,
            Vec<String>,           // unknowns
            Option<String>,        // tearing_var
            Vec<crate::ast::Equation>,  // inner equations
            Vec<crate::ast::Expression>, // residuals
        )],
        stack_slots: &std::collections::HashMap<String, cranelift::codegen::ir::StackSlot>,
        var_map: &std::collections::HashMap<String, Value>,
        array_info: &std::collections::HashMap<String, crate::jit::types::ArrayInfo>,
        state_vars: &[String],
        discrete_vars: &[String],
        output_vars: &[String],
        state_var_index: &std::collections::HashMap<String, usize>,
        discrete_var_index: &std::collections::HashMap<String, usize>,
        param_var_index: &std::collections::HashMap<String, usize>,
        output_var_index: &std::collections::HashMap<String, usize>,
        connector_connection_degree: &std::collections::HashMap<String, usize>,
        stream_connection_set: &std::collections::HashMap<String, Vec<String>>,
        stream_flow_map: &std::collections::HashMap<String, String>,
        enumerations: &std::collections::HashMap<String, Vec<String>>,
    ) -> Result<(), String> {
        if deferred.is_empty() {
            return Ok(());
        }

        let ptr_ty = self.module.target_config().pointer_type();

        for (func_id, unknowns, tearing_var, inner_eqs, residuals) in deferred {
            // Build the block function signature
            let sig = crate::jit::translator::equation::block_compile::make_block_signature(
                &self.module,
            );

            // Clear context and start a fresh function
            self.module.clear_context(&mut self.ctx);
            let mut bc = cranelift::frontend::FunctionBuilderContext::new();
            let mut builder = cranelift::frontend::FunctionBuilder::new(
                &mut self.ctx.func,
                &mut bc,
            );

            let entry = super::translator::equation::block_compile::setup_block_entry(&mut builder);

            let time_val = builder.block_params(entry)[0];
            let states_ptr = builder.block_params(entry)[1];
            let discrete_ptr = builder.block_params(entry)[2];
            let derivs_ptr = builder.block_params(entry)[3];
            let params_ptr = builder.block_params(entry)[4];
            let outputs_ptr = builder.block_params(entry)[5];
            let when_states_ptr = builder.block_params(entry)[6];
            let crossings_ptr = builder.block_params(entry)[7];
            let pre_states_ptr = builder.block_params(entry)[8];
            let pre_discrete_ptr = builder.block_params(entry)[9];
            let _t_end = builder.block_params(entry)[10];
            let diag_res = builder.block_params(entry)[11];
            let diag_x = builder.block_params(entry)[12];
            let homotopy = builder.block_params(entry)[13];

            // Create sub-context for block body compilation
            let mut when_idx = 0usize;
            let mut crossings_idx = 0usize;
            let mut sub_var_map: HashMap<String, Value> = HashMap::new();
            // `time` and `t_end` are pure block-param Values (not backed by a
            // pointer + index like states/params), so they must be seeded into
            // var_map here. Without this, any `time`/`t_end` reference inside a
            // solvable block compiles to the 0.0 unknown-variable fallback.
            sub_var_map.insert("time".to_string(), time_val);
            sub_var_map.insert("t_end".to_string(), _t_end);
            let mut sub_ctx = TranslationContext {
                module: &mut self.module,
                var_map: &mut sub_var_map,
                stack_slots,
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
                when_idx: &mut when_idx,
                crossings_idx: &mut crossings_idx,
                state_vars,
                discrete_vars,
                output_vars,
                state_var_index,
                discrete_var_index,
                param_var_index,
                output_var_index,
                diag_residual_ptr: Some(diag_res),
                diag_x_ptr: Some(diag_x),
                homotopy_lambda_ptr: homotopy,
                declared_imports: None,
                string_literal_cache: None,
                string_literal_data_ctx: None,
                string_data_counter: None,
                external_modelica_names: None,
                function_return_block: None,
                connector_connection_degree,
                stream_connection_set,
                stream_flow_map,
                enumerations,
                varid_state_index: HashMap::new(),
                varid_discrete_index: HashMap::new(),
                varid_param_index: HashMap::new(),
                varid_output_index: HashMap::new(),
                loop_break_stack: Vec::new(),
                suppress_zero_crossings: false,
                delay_call_counter: 0,
                block_funcs: Vec::new(),
                block_index_counter: 0,
                deferred_blocks: Vec::new(),
            };

            // Compile the solvable block body into this function.
            // Dispatch based on whether this is a tearing block.
            if tearing_var.is_some() || residuals.len() == 1 {
                super::translator::equation::solvable_tearing::compile_single_unknown_or_tearing_solvable_block(
                    &unknowns[..],
                    tearing_var,
                    &inner_eqs[..],
                    &residuals[..],
                    &mut sub_ctx,
                    &mut builder,
                )?;
            } else {
                super::translator::equation::solvable::compile_solvable_block_general_n(
                    &unknowns[..],
                    &residuals[..],
                    &mut sub_ctx,
                    &mut builder,
                )?;
            }

            // Emit success return
            let success_code = builder.ins().iconst(cl_types::I32, 0);
            builder.ins().return_(&[success_code]);
            builder.seal_all_blocks();
            builder.finalize();

            // Save a copy of the block IR before define_function consumes
            // it, so emit_object_cache_artifact can include all __block_N
            // callees alongside calc_derivs (J11).
            self.block_irs.push(self.ctx.func.clone());

            self.module
                .define_function(*func_id, &mut self.ctx)
                .map_err(|e| format!("{:?}", e))?;
        }

        Ok(())
    }

    /// Compile a single solvable block into a standalone JIT module for hot-swapping.
    /// Returns a struct holding both the function pointer and the owning JIT instance.
    /// The caller manages the function pointer table and atomic replacement.
    pub fn compile_standalone_block(
        unknowns: &[String],
        tearing_var: &Option<String>,
        inner_eqs: &[crate::ast::Equation],
        residuals: &[crate::ast::Expression],
        array_info: &HashMap<String, crate::jit::types::ArrayInfo>,
        enumerations: &HashMap<String, Vec<String>>,
    ) -> Result<StandaloneBlock, String> {
        if unknowns.is_empty() {
            return Err("compile_standalone_block: empty unknowns".to_string());
        }

        let n_states = unknowns.len();
        let mut standalone = Jit::new();
        let sig = crate::jit::translator::equation::block_compile::make_block_signature(
            &standalone.module,
        );
        let func_id = standalone
            .module
            .declare_function("__block_hotswap", Linkage::Export, &sig)
            .map_err(|e| e.to_string())?;

        standalone.ctx.func.signature = sig.clone();
        standalone.ctx.func.name =
            cranelift::codegen::ir::UserFuncName::user(0, func_id.as_u32());

        // Build the block function
        let mut bc = cranelift::frontend::FunctionBuilderContext::new();
        let mut builder =
            cranelift::frontend::FunctionBuilder::new(&mut standalone.ctx.func, &mut bc);

        let entry = super::translator::equation::block_compile::setup_block_entry(&mut builder);

        let states_ptr = builder.block_params(entry)[1];
        let discrete_ptr = builder.block_params(entry)[2];
        let derivs_ptr = builder.block_params(entry)[3];
        let params_ptr = builder.block_params(entry)[4];
        let outputs_ptr = builder.block_params(entry)[5];
        let when_states_ptr = builder.block_params(entry)[6];
        let crossings_ptr = builder.block_params(entry)[7];
        let pre_states_ptr = builder.block_params(entry)[8];
        let pre_discrete_ptr = builder.block_params(entry)[9];
        let diag_res = builder.block_params(entry)[11];
        let diag_x = builder.block_params(entry)[12];
        let homotopy = builder.block_params(entry)[13];

        let state_vars: Vec<String> = (0..n_states).map(|i| format!("s{}", i)).collect();
        let mut var_map: HashMap<String, Value> = HashMap::new();
        let empty_slots: HashMap<String, cranelift::codegen::ir::StackSlot> = HashMap::new();
        let mut when_idx = 0usize;
        let mut crossings_idx = 0usize;
        let state_var_index: HashMap<String, usize> = state_vars.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();
        let empty_conn_deg: HashMap<String, usize> = HashMap::new();
        let empty_scs: HashMap<String, Vec<String>> = HashMap::new();
        let empty_sfm: HashMap<String, String> = HashMap::new();

        let mut sub_ctx = TranslationContext {
            module: &mut standalone.module,
            var_map: &mut var_map,
            stack_slots: &empty_slots,
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
            when_idx: &mut when_idx,
            crossings_idx: &mut crossings_idx,
            state_vars: &state_vars,
            discrete_vars: &[],
            output_vars: &[],
            state_var_index: &state_var_index,
            discrete_var_index: &HashMap::new(),
            param_var_index: &HashMap::new(),
            output_var_index: &HashMap::new(),
            diag_residual_ptr: Some(diag_res),
            diag_x_ptr: Some(diag_x),
            homotopy_lambda_ptr: homotopy,
            declared_imports: None,
            string_literal_cache: None,
            string_literal_data_ctx: None,
            string_data_counter: None,
            external_modelica_names: None,
            function_return_block: None,
            connector_connection_degree: &empty_conn_deg,
            stream_connection_set: &empty_scs,
            stream_flow_map: &empty_sfm,
            enumerations,
            varid_state_index: HashMap::new(),
            varid_discrete_index: HashMap::new(),
            varid_param_index: HashMap::new(),
            varid_output_index: HashMap::new(),
            loop_break_stack: Vec::new(),
            suppress_zero_crossings: false,
            delay_call_counter: 0,
            block_funcs: Vec::new(),
            block_index_counter: 0,
            deferred_blocks: Vec::new(),
        };

        // Compile block body
        if tearing_var.is_some() || residuals.len() == 1 {
            super::translator::equation::solvable_tearing::compile_single_unknown_or_tearing_solvable_block(
                unknowns,
                tearing_var,
                inner_eqs,
                residuals,
                &mut sub_ctx,
                &mut builder,
            )?;
        } else {
            super::translator::equation::solvable::compile_solvable_block_general_n(
                unknowns,
                residuals,
                &mut sub_ctx,
                &mut builder,
            )?;
        }

        let success_code = builder.ins().iconst(cl_types::I32, 0);
        builder.ins().return_(&[success_code]);
        builder.seal_all_blocks();
        builder.finalize();

        standalone.module.define_function(func_id, &mut standalone.ctx)
            .map_err(|e| format!("{:?}", e))?;
        standalone.module.clear_context(&mut standalone.ctx);
        standalone.module.finalize_definitions()
            .map_err(|e| e.to_string())?;

        let code = standalone.module.get_finalized_function(func_id);
        let func: CalcDerivsFunc = unsafe { std::mem::transmute(code) };
        Ok(StandaloneBlock { jit: Box::new(standalone), func })
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
