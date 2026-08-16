use std::collections::{HashMap, HashSet};
use std::time::Instant;

use rayon::prelude::*;

use crate::ast::Expression;
use crate::compiler::{inline, collect_all_called_names, CompilePerfReport, Compiler};
use crate::jit::Jit;
use crate::ast::Equation;
use crate::ast::AlgorithmStatement;

use super::super::env_perf::jit_stub_parallel_enabled;

#[derive(Clone)]
struct StubPlan {
    name: String,
    input_names: Vec<String>,
    output_expr: Expression,
}

fn candidate_load_names(name: &str) -> Vec<String> {
            let mut cands: Vec<String> = Vec::new();
            if name == "valveCharacteristic" {
                cands.push("Modelica.Fluid.Valves.BaseClasses.ValveCharacteristics.linear".to_string());
            }
            if name.starts_with("world.") {
                cands.push(format!(
                    "Modelica.Mechanics.MultiBody.World.{}",
                    name.trim_start_matches("world.")
                ));
            }
            if name.starts_with("BaseClasses.") {
                let rest = name.trim_start_matches("BaseClasses.");
                cands.push(format!("Modelica.Fluid.Valves.BaseClasses.{}", rest));
                cands.push(format!("Modelica.Fluid.Pipes.BaseClasses.{}", rest));
                cands.push(format!("Modelica.Fluid.Machines.BaseClasses.{}", rest));
                cands.push(format!("Modelica.Fluid.Utilities.{}", name));
            }
            if name.starts_with("Utilities.") {
                cands.push(format!("Modelica.Fluid.{}", name));
                cands.push(format!("Modelica.{}", name));
            }
            if let Some(rest) = name.strip_prefix("from_") {
                cands.push(format!(
                    "Modelica.Blocks.Math.UnitConversions.From_{}",
                    rest
                ));
            }
            if let Some(rest) = name.strip_prefix("to_") {
                cands.push(format!(
                    "Modelica.Blocks.Math.UnitConversions.To_{}",
                    rest
                ));
            }
            if name == "arg" {
                cands.push("Modelica.ComplexMath.arg".to_string());
            }
            if name == "distribution" {
                cands.push("Modelica.Blocks.Noise.Interfaces.distribution".to_string());
                cands.push("Modelica.Math.Distributions.distribution".to_string());
            }
            if name == "oneTrue" {
                cands.push("Modelica.Electrical.Batteries.Utilities.oneTrue".to_string());
            }
            if name == "isPowerOf2" {
                cands.push("Modelica.Electrical.Polyphase.Functions.isPowerOf2".to_string());
                cands.push("Modelica.Electrical.QuasiStatic.Polyphase.Functions.isPowerOf2".to_string());
            }
            if name == "numberOfSymmetricBaseSystems" {
                cands.push("Modelica.Electrical.Polyphase.Functions.numberOfSymmetricBaseSystems".to_string());
                cands.push("Modelica.Electrical.QuasiStatic.Polyphase.Functions.numberOfSymmetricBaseSystems".to_string());
            }
            if name == "factorY2DC" {
                cands.push("Modelica.Electrical.Polyphase.Functions.factorY2DC".to_string());
                cands.push("Modelica.Electrical.QuasiStatic.Polyphase.Functions.factorY2DC".to_string());
            }
            if name == "exlin" {
                cands.push("Modelica.Electrical.Analog.Semiconductors.exlin".to_string());
            }
            if name == "exlin2" {
                cands.push("Modelica.Electrical.Analog.Semiconductors.exlin2".to_string());
            }
            if name.starts_with("Machines.") {
                cands.push(format!("Modelica.Electrical.{}", name));
            }
            if name.starts_with("Mechanics.") {
                cands.push(format!("Modelica.{}", name));
            }
            if name == "Cv" {
                cands.push("Modelica.Units.Conversions".to_string());
            } else if let Some(rest) = name.strip_prefix("Cv.") {
                cands.push(format!("Modelica.Units.Conversions.{}", rest));
            }
            cands.push(name.to_string());
            // Deduplicate while preserving order.
            let mut dedup = HashSet::new();
            cands
                .into_iter()
                .filter(|n| dedup.insert(n.clone()))
                .collect()
}

pub(crate) struct StubCompileResult {
    pub user_stub_jits: Vec<Jit>,
    pub user_stub_ptrs: HashMap<String, *const u8>,
    pub user_function_bodies: HashMap<String, (Vec<String>, Expression)>,
}

pub(crate) fn compile_user_stubs(
    compiler: &mut Compiler,
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    algorithms: &[AlgorithmStatement],
    external_list: &[(String, String, Option<String>)],
    perf_report: &mut CompilePerfReport,
) -> Result<StubCompileResult, Box<dyn std::error::Error + Send + Sync>> {
        let all_called = collect_all_called_names(alg_equations, diff_equations, algorithms);
        let external_names: HashSet<String> =
            external_list.iter().map(|(n, _, _)| n.clone()).collect();
        let stub_cap = all_called.len().saturating_sub(external_names.len());
        let mut user_stub_jits: Vec<Jit> = Vec::new();
        let mut user_stub_ptrs: HashMap<String, *const u8> = HashMap::with_capacity(stub_cap);
        let mut user_function_bodies: HashMap<String, (Vec<String>, Expression)> =
            HashMap::with_capacity(stub_cap);
        let stub_compile_t0 = Instant::now();
        let mut stub_plans: Vec<StubPlan> = Vec::new();
        perf_report.stub_parallel_enabled = jit_stub_parallel_enabled();
        if perf_report.stub_parallel_enabled {
            let loader_paths = compiler.loader.library_paths.clone();
            let scan_results: Vec<Result<Option<StubPlan>, String>> = all_called
                .par_iter()
                .map(|name| {
                    if inline::is_builtin_function(name) || external_names.contains(name) {
                        return Ok(None);
                    }
                    let mut local_loader = crate::loader::ModelLoader::new();
                    local_loader.set_quiet(true);
                    for p in &loader_paths {
                        local_loader.add_path(p.clone());
                    }
                    let mut func_model = None;
                    for cand in candidate_load_names(name) {
                        if let Ok(m) = local_loader.load_model(&cand) {
                            func_model = Some(m);
                            break;
                        }
                    }
                    let Some(func_model) = func_model else {
                        return Ok(None);
                    };
                    if func_model.external_info.is_some() {
                        return Ok(None);
                    }
                    let Some((input_names, outputs)) = inline::get_function_body(func_model.as_ref()) else {
                        return Ok(None);
                    };
                    if outputs.len() != 1 {
                        return Err(format!(
                            "Function '{}' has {} outputs; JIT callable supports single-output only (FUNC-2).",
                            name, outputs.len()
                        ));
                    }
                    Ok(Some(StubPlan {
                        name: name.clone(),
                        input_names: input_names.clone(),
                        output_expr: outputs[0].1.clone(),
                    }))
                })
                .collect();
            for item in scan_results {
                match item {
                    Ok(Some(p)) => stub_plans.push(p),
                    Ok(None) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            stub_plans.sort_by(|a, b| a.name.cmp(&b.name));
            stub_plans.dedup_by(|a, b| a.name == b.name);
        } else {
            for name in &all_called {
                if inline::is_builtin_function(name) || external_names.contains(name) {
                    continue;
                }
                let mut func_model = None;
                for cand in candidate_load_names(name) {
                    if let Ok(m) = compiler.loader.load_model(&cand) {
                        func_model = Some(m);
                        break;
                    }
                }
                let Some(func_model) = func_model else {
                    continue;
                };
                if func_model.external_info.is_some() {
                    continue;
                }
                let Some((input_names, outputs)) = inline::get_function_body(func_model.as_ref()) else {
                    continue;
                };
                if outputs.len() != 1 {
                    return Err(format!(
                        "Function '{}' has {} outputs; JIT callable supports single-output only (FUNC-2).",
                        name, outputs.len()
                    ).into());
                }
                stub_plans.push(StubPlan {
                    name: name.clone(),
                    input_names: input_names.clone(),
                    output_expr: outputs[0].1.clone(),
                });
            }
        }
        perf_report.stub_candidate_count = stub_plans.len();
        for p in stub_plans {
            let mut stub_jit = Jit::new();
            let ptr = stub_jit
                .compile_user_function_stub(&p.name, &p.input_names, &p.output_expr)
                .map_err(|e| format!("JIT stub for '{}': {}", p.name, e))?;
            user_stub_ptrs.insert(p.name.clone(), ptr);
            user_function_bodies.insert(p.name, (p.input_names, p.output_expr));
            user_stub_jits.push(stub_jit);
        }
        let stub_elapsed = stub_compile_t0.elapsed();
        perf_report.stub_compile_ms = stub_elapsed.as_millis() as u64;
        perf_report.stub_compile_us = stub_elapsed.as_micros() as u64;
        Ok(StubCompileResult {
            user_stub_jits,
            user_stub_ptrs,
            user_function_bodies,
        })
}
