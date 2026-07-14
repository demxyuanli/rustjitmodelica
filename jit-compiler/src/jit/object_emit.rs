//! Relocatable object emission for Windows codegen disk cache.

use std::collections::HashMap;

use cranelift::codegen::ir::{Function, UserExternalName, UserFuncName};
use cranelift::prelude::{settings, Configurable};
use cranelift_module::{FuncId, FunctionDeclaration, Linkage, Module, ModuleDeclarations};
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

/// Recover the JIT-module `FuncId` a cloned IR function was defined under.
fn jit_func_id_of(func: &Function) -> Option<FuncId> {
    match &func.name {
        UserFuncName::User(un) if un.namespace == 0 => Some(FuncId::from_u32(un.index)),
        _ => None,
    }
}

/// Rewrite every `ExternalName::User` reference in `func` from JIT-module
/// FuncId indices to the corresponding object-module FuncId indices.
///
/// The IR cloned out of the JIT module indexes callees by the JIT module's
/// FuncIds. The object module assigns its own independent ids, so without
/// this remap a cached object either fails to build (unknown id) or -- worse
/// -- silently binds a call to the wrong symbol.
fn remap_external_names(
    func: &mut Function,
    jit_fns: &HashMap<u32, (FuncId, &FunctionDeclaration)>,
    module: &mut ObjectModule,
    declared: &mut HashMap<String, FuncId>,
) -> Result<(), String> {
    let entries: Vec<(cranelift::codegen::ir::UserExternalNameRef, UserExternalName)> = func
        .params
        .user_named_funcs()
        .iter()
        .map(|(r, n)| (r, n.clone()))
        .collect();
    for (name_ref, ext) in entries {
        if ext.namespace != 0 {
            // Namespace 1 is data objects (array/string literals). Their
            // contents live in the JIT module's memory and cannot be resolved
            // when reloading the object, so refuse to emit an artifact.
            return Err(format!(
                "external name namespace {} (data object) not supported in object cache",
                ext.namespace
            ));
        }
        let (jit_id, decl) = match jit_fns.get(&ext.index) {
            Some(v) => *v,
            None => {
                return Err(format!(
                    "referenced JIT function id {} has no declaration",
                    ext.index
                ))
            }
        };
        let sym = decl.linkage_name(jit_id).into_owned();
        let obj_id = match declared.get(&sym) {
            Some(fid) => *fid,
            None => {
                // Only true runtime imports may be re-declared as imports in
                // the object: their addresses are re-resolved from the
                // runtime symbol table at load time. Functions *defined* in
                // the JIT module (user Modelica functions, stubs) live in JIT
                // memory only, so a cached object cannot reference them.
                if decl.linkage != Linkage::Import {
                    return Err(format!(
                        "callee '{}' is defined in the JIT module (linkage {:?}); not cacheable",
                        sym, decl.linkage
                    ));
                }
                let fid = module
                    .declare_function(&sym, Linkage::Import, &decl.signature)
                    .map_err(|e| e.to_string())?;
                declared.insert(sym, fid);
                fid
            }
        };
        func.params
            .reset_user_func_name(name_ref, UserExternalName::new(0, obj_id.as_u32()));
    }
    Ok(())
}

pub(crate) fn emit_object_cache_artifact(
    ir_func: &Function,
    // IR for deferred block callee functions (`__block_N`). They must be
    // declared+defined in the object module alongside `calc_derivs`, otherwise
    // a cached load of `calc_derivs` will call into absent functions.
    block_irs: &[Function],
    jit_decls: &ModuleDeclarations,
) -> Result<Vec<u8>, String> {
    let isa = build_object_isa()?;
    let builder = ObjectBuilder::new(isa, "jit_cache_obj", cranelift_module::default_libcall_names())
        .map_err(|e| e.to_string())?;
    let mut module = ObjectModule::new(builder);
    let mut ctx = module.make_context();

    let jit_fns: HashMap<u32, (FuncId, &FunctionDeclaration)> = jit_decls
        .get_functions()
        .map(|(id, decl)| (id.as_u32(), (id, decl)))
        .collect();

    // Symbol name -> object-module FuncId for everything declared so far.
    let mut declared: HashMap<String, FuncId> = HashMap::new();

    // Pre-declare block callees under their JIT linkage names (Linkage::Local
    // since they are not called from outside the object) so that calls from
    // `calc_derivs` remap onto the locally defined symbols.
    let mut block_defs: Vec<(FuncId, &Function)> = Vec::with_capacity(block_irs.len());
    for f in block_irs {
        let jit_id = jit_func_id_of(f)
            .ok_or_else(|| "block function has no user func name".to_string())?;
        let (jit_id, decl) = match jit_fns.get(&jit_id.as_u32()) {
            Some(v) => *v,
            None => {
                return Err(format!(
                    "block function id {} has no JIT declaration",
                    jit_id.as_u32()
                ))
            }
        };
        let sym = decl.linkage_name(jit_id).into_owned();
        let fid = module
            .declare_function(&sym, Linkage::Local, &f.signature)
            .map_err(|e| e.to_string())?;
        declared.insert(sym, fid);
        block_defs.push((fid, f));
    }

    let name = "calc_derivs";
    let func_id = module
        .declare_function(name, Linkage::Export, &ir_func.signature)
        .map_err(|e| e.to_string())?;
    declared.insert(name.to_string(), func_id);

    for (fid, f) in block_defs {
        ctx.func = (*f).clone();
        ctx.func.name = UserFuncName::user(0, fid.as_u32());
        remap_external_names(&mut ctx.func, &jit_fns, &mut module, &mut declared)?;
        module
            .define_function(fid, &mut ctx)
            .map_err(|e| e.to_string())?;
        module.clear_context(&mut ctx);
    }

    ctx.func = ir_func.clone();
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());
    remap_external_names(&mut ctx.func, &jit_fns, &mut module, &mut declared)?;
    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| e.to_string())?;
    module.clear_context(&mut ctx);
    let product = module.finish();
    product.emit().map_err(|e| e.to_string())
}
