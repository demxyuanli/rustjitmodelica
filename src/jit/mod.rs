use cranelift::prelude::{*, types as cl_types};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, Linkage, Module};
use cranelift::codegen::ir::UserFuncName;
use std::collections::{HashMap, HashSet};
use std::mem;
use crate::ast::*;

pub mod types;
pub mod native;
pub mod context;
pub mod translator;
pub mod analysis;

pub use self::types::{ArrayType, ArrayInfo, CalcDerivsFunc};
use self::context::TranslationContext;
use self::translator::{compile_algorithm_stmt, compile_equation};
use self::analysis::{collect_modified, collect_modified_equations};
use self::native::register_symbols;

pub struct Jit {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    #[allow(dead_code)]
    data_ctx: DataDescription,
    module: JITModule,
}

impl Jit {
    pub fn new() -> Self {
        let mut builder = JITBuilder::new(cranelift_module::default_libcall_names()).unwrap();
        
        register_symbols(&mut builder);

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
        algorithms: &[AlgorithmStatement]
    ) -> Result<(CalcDerivsFunc, usize, usize), String> {
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(cl_types::F64)); // time
        sig.params.push(AbiParam::new(self.module.target_config().pointer_type())); // states ptr (mut)
        sig.params.push(AbiParam::new(self.module.target_config().pointer_type())); // discrete ptr (mut)
        sig.params.push(AbiParam::new(self.module.target_config().pointer_type())); // derivs ptr
        sig.params.push(AbiParam::new(self.module.target_config().pointer_type())); // params ptr
        sig.params.push(AbiParam::new(self.module.target_config().pointer_type())); // outputs ptr
        sig.params.push(AbiParam::new(self.module.target_config().pointer_type())); // when_states ptr (mut)
        sig.params.push(AbiParam::new(self.module.target_config().pointer_type())); // crossings ptr
        sig.params.push(AbiParam::new(self.module.target_config().pointer_type())); // pre_states ptr (const)
        sig.params.push(AbiParam::new(self.module.target_config().pointer_type())); // pre_discrete ptr (const)
        sig.returns.push(AbiParam::new(cl_types::I32)); // Return status code

        let func_id = self.module.declare_function("calc_derivs", Linkage::Export, &sig)
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

                let mut var_map = HashMap::new();
                var_map.insert("time".to_string(), time_val);

                for (i, name) in state_vars.iter().enumerate() {
                    let offset = (i * 8) as i32;
                    let val = builder.ins().load(cl_types::F64, MemFlags::new(), states_ptr, offset);
                    var_map.insert(name.clone(), val);
                }
                for (i, name) in discrete_vars.iter().enumerate() {
                    let offset = (i * 8) as i32;
                    let val = builder.ins().load(cl_types::F64, MemFlags::new(), discrete_ptr, offset);
                    var_map.insert(name.clone(), val);
                }
                for (i, name) in param_vars.iter().enumerate() {
                    let offset = (i * 8) as i32;
                    let val = builder.ins().load(cl_types::F64, MemFlags::new(), params_ptr, offset);
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
                    let slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0));
                    stack_slots.insert(var.clone(), slot);
                    if let Some(val) = var_map.get(var) {
                        builder.ins().stack_store(*val, slot, 0);
                    } else {
                        let zero = builder.ins().f64const(0.0);
                        builder.ins().stack_store(zero, slot, 0);
                    }
                }

            let state_var_index: HashMap<String, usize> = state_vars.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();
            let discrete_var_index: HashMap<String, usize> = discrete_vars.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();
            let output_var_index: HashMap<String, usize> = output_vars.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();

            let mut when_idx = 0;
            let mut crossings_idx = 0;
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
                &output_var_index,
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
                    builder.ins().store(MemFlags::new(), val, discrete_ptr, offset);
                }
            }

            let success_code = builder.ins().iconst(cl_types::I32, 0);
            builder.ins().return_(&[success_code]);
            builder.finalize();
        }


        self.module.define_function(func_id, &mut self.ctx).map_err(|e| e.to_string())?;
        self.module.clear_context(&mut self.ctx);
        self.module.finalize_definitions().map_err(|e| e.to_string())?;

        let code = self.module.get_finalized_function(func_id);
        let func: CalcDerivsFunc = unsafe { mem::transmute(code) };
        Ok((func, when_count, crossings_count))
    }
}
