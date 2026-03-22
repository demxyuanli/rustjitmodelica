use cranelift::prelude::*;
use cranelift_module::{DataDescription, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use cranelift::codegen::ir::{StackSlot, UserFuncName};
use std::collections::HashMap;
use crate::ast::*;

pub struct Codegen {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    data_ctx: DataDescription,
    module: ObjectModule,
    counter: usize,
}

impl Codegen {
    pub fn new() -> Self {
        let flag_builder = settings::builder();
        let isa_builder = cranelift_native::builder().unwrap();
        let isa = isa_builder.finish(settings::Flags::new(flag_builder)).unwrap();

        let builder = ObjectBuilder::new(isa, "modelica_module", cranelift_module::default_libcall_names()).unwrap();
        let module = ObjectModule::new(builder);

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            data_ctx: DataDescription::new(),
            module,
            counter: 0,
        }
    }

    pub fn compile(mut self, model: &Model) -> Result<Vec<u8>, String> {
        // 1. Pre-define strings for printf
        let mut string_data_ids = HashMap::new();
        for decl in &model.declarations {
            let s = format!("{} = %f\n\0", decl.name);
            let id = self.create_data(&s)?;
            string_data_ids.insert(decl.name.clone(), id);
        }

        // 2. Define main function
        let mut sig = self.module.make_signature();
        sig.returns.push(AbiParam::new(types::I32));

        let main_func_id = self.module.declare_function("main", Linkage::Export, &sig)
            .map_err(|e| e.to_string())?;

        self.ctx.func.signature = sig;
        self.ctx.func.name = UserFuncName::user(0, main_func_id.as_u32());

        {
            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            // Declare printf
            let mut printf_sig = self.module.make_signature();
            printf_sig.params.push(AbiParam::new(self.module.target_config().pointer_type()));
            printf_sig.params.push(AbiParam::new(types::F64));
            printf_sig.returns.push(AbiParam::new(types::I32));
            
            let printf_func = self.module.declare_function("printf", Linkage::Import, &printf_sig)
                .map_err(|e| e.to_string())?;
            let printf_func_ref = self.module.declare_func_in_func(printf_func, &mut builder.func);

            // Stack slots
            let mut var_map = HashMap::new();
            for decl in &model.declarations {
                // Size 8 (f64), Align 8 (2^3)
                let slot = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
                var_map.insert(decl.name.clone(), slot);
            }

            // Equations
            for eq in &model.equations {
                if let Expression::Variable(ref var_id) = eq.lhs {
                    let var_name = crate::string_intern::resolve_id(*var_id);
                    if let Some(&slot) = var_map.get(&var_name) {
                        let val = compile_expression(&eq.rhs, &mut builder, &var_map, &mut self.module)?;
                        builder.ins().stack_store(val, slot, 0);

                        // Print
                        if let Some(&data_id) = string_data_ids.get(&var_name) {
                            let data_ref = self.module.declare_data_in_func(data_id, &mut builder.func);
                            let fmt_ptr = builder.ins().global_value(self.module.target_config().pointer_type(), data_ref);
                            builder.ins().call(printf_func_ref, &[fmt_ptr, val]);
                        }
                    }
                }
            }

            // Return 0
            let ret_val = builder.ins().iconst(types::I32, 0);
            builder.ins().return_(&[ret_val]);

            builder.finalize();
        }

        self.module.define_function(main_func_id, &mut self.ctx).map_err(|e| e.to_string())?;
        self.module.clear_context(&mut self.ctx);

        let product = self.module.finish();
        let obj_bytes = product.emit().map_err(|e| e.to_string())?;

        Ok(obj_bytes)
    }

    fn create_data(&mut self, content: &str) -> Result<cranelift_module::DataId, String> {
        self.counter += 1;
        let name = format!("str_{}", self.counter); 
        self.data_ctx.define(content.as_bytes().to_vec().into_boxed_slice());
        let id = self.module.declare_data(&name, Linkage::Local, true, false).map_err(|e| e.to_string())?;
        self.module.define_data(id, &self.data_ctx).map_err(|e| e.to_string())?;
        self.data_ctx.clear();
        Ok(id)
    }
}

fn compile_expression(
    expr: &Expression, 
    builder: &mut FunctionBuilder, 
    var_map: &HashMap<String, StackSlot>,
    module: &mut ObjectModule
) -> Result<Value, String> {
    match expr {
        Expression::Number(n) => Ok(builder.ins().f64const(*n)),
        Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            if let Some(&slot) = var_map.get(&name) {
                Ok(builder.ins().stack_load(types::F64, slot, 0))
            } else {
                Err(format!("Variable {} not found", name))
            }
        }
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = compile_expression(lhs, builder, var_map, module)?;
            let r = compile_expression(rhs, builder, var_map, module)?;
            match op {
                Operator::Add => Ok(builder.ins().fadd(l, r)),
                Operator::Sub => Ok(builder.ins().fsub(l, r)),
                Operator::Mul => Ok(builder.ins().fmul(l, r)),
                Operator::Div => Ok(builder.ins().fdiv(l, r)),
            }
        }
        Expression::Call(func_name, arg) => {
            let arg_val = compile_expression(arg, builder, var_map, module)?;
            
            let mut sig = module.make_signature();
            sig.params.push(AbiParam::new(types::F64));
            sig.returns.push(AbiParam::new(types::F64));
            
            let func_id = module.declare_function(func_name, Linkage::Import, &sig)
                .map_err(|e| e.to_string())?;
            let func_ref = module.declare_func_in_func(func_id, &mut builder.func);
            
            let call_inst = builder.ins().call(func_ref, &[arg_val]);
            Ok(builder.inst_results(call_inst)[0])
        }
    }
}
