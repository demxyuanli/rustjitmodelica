use crate::ast::*;
use cranelift::codegen::ir::UserFuncName;
use cranelift::prelude::{types as cl_types, *};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, FuncId, Linkage, Module};
use std::collections::{HashMap, HashSet};
use std::mem;

pub mod analysis;
pub mod context;
pub mod native;
pub mod translator;
pub mod types;

use self::analysis::{collect_modified, collect_modified_equations};
use self::context::TranslationContext;
use self::native::register_symbols;
use self::translator::expr::compile_expression;
use self::translator::{compile_algorithm_stmt, compile_equation};
pub use self::types::{ArrayInfo, ArrayType, CalcDerivsFunc};

pub struct Jit {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    #[allow(dead_code)]
    data_ctx: DataDescription,
    module: JITModule,
}

impl Jit {
    pub fn new() -> Self {
        Self::new_with_extra_symbols(None)
    }

    /// EXT-2: Create JIT with optional extra symbols (e.g. from --external-lib loaded libraries).
    pub fn new_with_extra_symbols(
        extra: Option<&std::collections::HashMap<String, *const u8>>,
    ) -> Self {
        let mut builder = JITBuilder::new(cranelift_module::default_libcall_names()).unwrap();

        register_symbols(&mut builder);
        if let Some(map) = extra {
            for (name, ptr) in map {
                builder.symbol(name, *ptr);
            }
        }

        let module = JITModule::new(builder);

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            data_ctx: DataDescription::new(),
            module,
        }
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
        _t_end: f64,
        newton_tearing_var_names: &[String],
    ) -> Result<(CalcDerivsFunc, usize, usize), String> {
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(cl_types::F64)); // time
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type())); // states ptr (mut)
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type())); // discrete ptr (mut)
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type())); // derivs ptr
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type())); // params ptr
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type())); // outputs ptr
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type())); // when_states ptr (mut)
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type())); // crossings ptr
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type())); // pre_states ptr (const)
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type())); // pre_discrete ptr (const)
        sig.params.push(AbiParam::new(cl_types::F64)); // t_end
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type())); // diag_residual (mut f64)
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type())); // diag_x (mut f64)
        sig.returns.push(AbiParam::new(cl_types::I32)); // Return status code

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

            let (diag_res, diag_x) = if newton_tearing_var_names.is_empty() {
                (None, None)
            } else {
                (Some(diag_residual_ptr), Some(diag_x_ptr))
            };

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
                Some(&mut declared_imports),
                Some(&mut string_literal_cache),
                Some(&mut self.data_ctx),
                Some(&mut string_data_counter),
            );

            for stmt in algorithms {
                compile_algorithm_stmt(stmt, &mut t_ctx, &mut builder)?;
            }
            when_count = *t_ctx.when_idx;
            crossings_count = *t_ctx.crossings_idx;
            builder.seal_all_blocks();

            for eq in alg_equations {
                compile_equation(eq, &mut t_ctx, &mut builder)?;
            }
            for eq in diff_equations {
                compile_equation(eq, &mut t_ctx, &mut builder)?;
            }

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
        let func: CalcDerivsFunc = unsafe { mem::transmute(code) };
        Ok((func, when_count, crossings_count))
    }

    /// FUNC-2: Compile a single-output user function to a JIT stub (f64, ...) -> f64 and return its pointer.
    /// The stub is defined in this module; call finalize_definitions and get_finalized_function after this.
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
                None,
                Some(&mut string_literal_cache),
                Some(&mut self.data_ctx),
                Some(&mut string_data_counter),
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
