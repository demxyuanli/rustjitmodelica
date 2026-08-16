use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::compiler::{CompilePerfReport, Compiler};
use crate::jit::native::builtin_jit_symbol_names;

pub(crate) fn load_finish_libs(
    compiler: &mut Compiler,
    external_list: &[(String, String, Option<String>)],
    user_stub_ptrs: HashMap<String, *const u8>,
    external_names: &HashSet<String>,
    external_t0: Instant,
    perf_trace: bool,
    perf_report: &mut CompilePerfReport,
) -> Result<(Vec<std::path::PathBuf>, HashMap<String, *const u8>), Box<dyn std::error::Error + Send + Sync>>
{
        let lib_paths: Vec<std::path::PathBuf> = if !compiler.options.external_libs.is_empty() {
            compiler.options
                .external_libs
                .iter()
                .map(|p| std::path::PathBuf::from(p))
                .collect()
        } else {
            let mut from_annotation: Vec<std::path::PathBuf> = external_list
                .iter()
                .filter_map(|(_, _, hint)| hint.as_ref())
                .map(|lib_name| {
                    let ext = std::env::consts::DLL_EXTENSION;
                    std::path::PathBuf::from(format!("{}.{}", lib_name, ext))
                })
                .collect();
            from_annotation.sort();
            from_annotation.dedup();
            from_annotation
        };
        if !lib_paths.is_empty() {
            compiler.external_libraries.0.clear();
            compiler.external_symbol_ptrs.clear();
            for path in &lib_paths {
                let lib = unsafe { libloading::Library::new(path.as_path()) }.map_err(|e| {
                    format!("Failed to load external lib '{}': {}", path.display(), e)
                })?;
                for (modelica_name, c_name, _) in external_list {
                    if compiler.external_symbol_ptrs.contains_key(modelica_name) {
                        continue;
                    }
                    if let Ok(sym) = unsafe { lib.get::<extern "C" fn()>(c_name.as_bytes()) } {
                        let ptr = *sym as *const u8;
                        compiler.external_symbol_ptrs.insert(modelica_name.clone(), ptr);
                    }
                }
                compiler.external_libraries.0.push(lib);
            }
            for (modelica_name, _c_name, _) in external_list {
                if !compiler.external_symbol_ptrs.contains_key(modelica_name) {
                    return Err(format!(
                        "EXT-2: external function '{}' not found in any loaded library (--external-lib or annotation Library)",
                        modelica_name
                    ).into());
                }
            }
        }

        let mut all_symbols = compiler.external_symbol_ptrs.clone();
        for (k, v) in user_stub_ptrs {
            all_symbols.insert(k, v);
        }
        for (modelica_name, c_name, _) in external_list {
            if all_symbols.contains_key(modelica_name) {
                continue;
            }
            if let Some(ptr) = crate::jit::native::jit_stub_for_external_c_name(c_name) {
                all_symbols.insert(modelica_name.clone(), ptr);
            }
        }

        let builtins = builtin_jit_symbol_names();
        for name in external_names {
            if builtins.contains(name.as_str()) || all_symbols.contains_key(name) {
                continue;
            }
            // Named JIT builtin policy handlers cover validate-mode placeholders
            // (e.g. Modelica.Utilities.Strings.substring) without a shared library.
            if crate::jit::jit_policy::match_function_builtin_rule(name).is_some() {
                continue;
            }
            return Err(format!(
                "External function '{}' is not linked. Provide a shared library with this symbol (e.g. --external-lib=<path> or Library annotation).",
                name
            ).into());
        }
        if perf_trace {
            perf_report.external_resolve_ms = external_t0.elapsed().as_millis() as u64;
            eprintln!(
                "[perf] compile_phase.external_resolve_ms={}",
                perf_report.external_resolve_ms
            );
        } else {
            perf_report.external_resolve_ms = external_t0.elapsed().as_millis() as u64;
        }
        Ok((lib_paths, all_symbols))
}
