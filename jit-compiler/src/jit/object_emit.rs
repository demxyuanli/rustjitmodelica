//! Relocatable object emission for Windows codegen disk cache.

use cranelift::codegen::ir::UserFuncName;
use cranelift::prelude::{settings, Configurable};
use cranelift_module::{Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::jit::config::jit_opt_level_from_env;

pub(crate) fn build_object_isa() -> Result<std::sync::Arc<dyn cranelift::codegen::isa::TargetIsa>, String> {
    let mut flag_builder = settings::builder();
    let _ = flag_builder.set("opt_level", &jit_opt_level_from_env());
    let _ = flag_builder.set("is_pic", "true");
    let isa_builder = cranelift_native::builder().map_err(|e| e.to_string())?;
    isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| e.to_string())
}

pub(crate) fn emit_object_cache_artifact(
    ir_func: &cranelift::codegen::ir::Function,
    // IR for deferred block callee functions (`__block_N`). They must be
    // declared+defined in the object module alongside `calc_derivs`, otherwise
    // a cached load of `calc_derivs` will call into absent functions.
    block_irs: &[cranelift::codegen::ir::Function],
) -> Result<Vec<u8>, String> {
    let isa = build_object_isa()?;
    let builder = ObjectBuilder::new(isa, "jit_cache_obj", cranelift_module::default_libcall_names())
        .map_err(|e| e.to_string())?;
    let mut module = ObjectModule::new(builder);
    let mut ctx = module.make_context();

    // Declare + define each block callee first (Linkage::Local since they are
    // not called from outside the object).
    for f in block_irs {
        let fid = module
            .declare_function(&f.name.to_string(), Linkage::Local, &f.signature)
            .map_err(|e| e.to_string())?;
        ctx.func = f.clone();
        ctx.func.name = UserFuncName::user(0, fid.as_u32());
        module
            .define_function(fid, &mut ctx)
            .map_err(|e| e.to_string())?;
        module.clear_context(&mut ctx);
    }

    let name = "calc_derivs";
    let func_id = module
        .declare_function(name, Linkage::Export, &ir_func.signature)
        .map_err(|e| e.to_string())?;
    ctx.func = ir_func.clone();
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());
    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| e.to_string())?;
    module.clear_context(&mut ctx);
    let product = module.finish();
    product.emit().map_err(|e| e.to_string())
}
