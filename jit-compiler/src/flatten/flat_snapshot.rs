//! Tier S structural regression snapshots (rust-only, deterministic JSON).
//!
//! ## Acceptance tiers (MLS / instantiate alignment work)
//! - **Tier S**: Compare canonical `FlatSnapshot` JSON to checked-in goldens (no simulator, no OMC).
//! - **Tier N**: Numeric comparison via `compare_omc.ps1` (last row / trajectory tolerances).
//! - **Tier O**: Optional OpenModelica `instantiateModel` log via `scripts/run_omc_instantiate_flat.ps1`.
//! Use `--coarse-constrainedby` on the CLI if you need legacy string `constrainedby` matching for a comparison run.
//!
//! ## Initial curated models
//! Grow from `ModelicaTest.RedeclareSmoke.*` (see `run_flat_snapshot_regress.ps1`).
//!
//! Schema version is embedded in JSON as `schema` for tooling.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::ast::Declaration;
use crate::flatten::FlattenedModel;
use crate::unparse::equation_to_string;

pub const FLAT_SNAPSHOT_SCHEMA: &str = "rustmodlica_flat_snapshot_v1";

#[derive(Serialize)]
pub struct FlatSnapshot<'a> {
    pub schema: &'a str,
    pub declarations: Vec<DeclSnapshot>,
    pub instances: BTreeMap<String, String>,
    pub array_sizes: BTreeMap<String, usize>,
    pub equations: Vec<String>,
    pub initial_equations: Vec<String>,
    pub connections: Vec<(String, String)>,
}

#[derive(Serialize)]
pub struct DeclSnapshot {
    pub name: String,
    pub type_name: String,
    pub is_flow: bool,
    pub is_parameter: bool,
    pub is_input: bool,
    pub is_output: bool,
    pub replaceable: bool,
    pub constrainedby_type: Option<String>,
}

fn decl_to_snap(d: &Declaration) -> DeclSnapshot {
    DeclSnapshot {
        name: d.name.clone(),
        type_name: d.type_name.clone(),
        is_flow: d.is_flow,
        is_parameter: d.is_parameter,
        is_input: d.is_input,
        is_output: d.is_output,
        replaceable: d.replaceable,
        constrainedby_type: d.constrainedby_type.clone(),
    }
}

/// Build canonical snapshot value (sorted collections, stable equation strings).
pub fn build_flat_snapshot(flat: &FlattenedModel) -> FlatSnapshot<'_> {
    let mut decls: Vec<DeclSnapshot> = flat.declarations.iter().map(decl_to_snap).collect();
    decls.sort_by(|a, b| a.name.cmp(&b.name));

    let instances: BTreeMap<String, String> = flat
        .instances
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let array_sizes: BTreeMap<String, usize> = flat
        .array_sizes
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();

    let mut equations: Vec<String> = flat.equations.iter().map(|e| equation_to_string(e)).collect();
    equations.sort();

    let mut initial_equations: Vec<String> = flat
        .initial_equations
        .iter()
        .map(|e| equation_to_string(e))
        .collect();
    initial_equations.sort();

    let mut connections = flat.connections.clone();
    connections.sort();

    FlatSnapshot {
        schema: FLAT_SNAPSHOT_SCHEMA,
        declarations: decls,
        instances,
        array_sizes,
        equations,
        initial_equations,
        connections,
    }
}

/// Serialize snapshot to pretty JSON (UTF-8).
pub fn flat_snapshot_json(flat: &FlattenedModel) -> Result<String, serde_json::Error> {
    let snap = build_flat_snapshot(flat);
    serde_json::to_string_pretty(&snap)
}

/// Write Tier S snapshot to `path` (creates parent dirs if needed).
pub fn write_flat_snapshot(path: &Path, flat: &FlattenedModel) -> Result<(), String> {
    let json = flat_snapshot_json(flat).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, json).map_err(|e| e.to_string())
}
