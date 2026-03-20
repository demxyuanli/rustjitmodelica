use crate::ast::{AlgorithmStatement, Declaration, Equation, Expression, ExtendsClause, Model};
use crate::compiler::inline::is_builtin_function;
use crate::diag::SourceLocation;
use crate::loader::{LoadError, ModelLoader};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

#[derive(Debug)]
pub enum FlattenError {
    Load(LoadError),
    UnknownType(String, String, Option<SourceLocation>),
    IncompatibleConnector(String, String, String, String, Option<SourceLocation>),
}

impl From<LoadError> for FlattenError {
    fn from(e: LoadError) -> Self {
        FlattenError::Load(e)
    }
}

impl fmt::Display for FlattenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlattenError::Load(e) => write!(f, "[FLATTEN_LOAD] {}", e),
            FlattenError::UnknownType(ty, inst, loc) => {
                write!(f, "[FLATTEN_UNKNOWN_TYPE] Unknown type '{}' for instance '{}'", ty, inst)?;
                if let Some(ref l) = loc {
                    write!(f, "{}", l.fmt_suffix())?;
                }
                Ok(())
            }
            FlattenError::IncompatibleConnector(a, b, ta, tb, loc) => {
                write!(f, "[FLATTEN_INCOMPATIBLE_CONNECTOR] Error: Incompatible connector types in connect({}, {}): type '{}' vs '{}' (model/connector paths as shown)", a, b, ta, tb)?;
                if let Some(ref l) = loc {
                    write!(f, "{}", l.fmt_suffix())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for FlattenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FlattenError::Load(e) => Some(e),
            _ => None,
        }
    }
}

pub mod connections;
mod expand;
pub mod expressions;
pub mod structures;
mod substitute;
pub mod utils;
mod clock_infer;

use self::connections::resolve_connections;
#[allow(unused_imports)]
pub use self::expressions::{
    eval_const_expr, eval_const_expr_with_array_sizes, expr_to_path, index_expression,
    prefix_expression,
};
pub use self::structures::FlattenedModel;
use self::utils::{apply_modification, is_primitive, merge_models, resolve_type_alias};

pub(crate) struct ExpandTarget<'a> {
    pub equations: &'a mut Vec<Equation>,
    pub algorithms: &'a mut Vec<AlgorithmStatement>,
    pub connections: &'a mut Vec<(String, String)>,
    pub conditional_connections: &'a mut Vec<(Expression, (String, String))>,
    pub array_sizes: &'a HashMap<String, usize>,
}

pub struct Flattener {
    pub loader: ModelLoader,
}

impl Flattener {
    /// Map static `Modelica.Magnetic.FundamentalWave.Utilities` types to the QS package when the
    /// flattened root lives under QuasiStatic.FundamentalWave (imports may still point at static MSL).
    fn qs_fw_utilities_redirect(resolved: &str, msl_import_context: &str) -> String {
        if !msl_import_context
            .starts_with("Modelica.Magnetic.QuasiStatic.FundamentalWave")
        {
            return resolved.to_string();
        }
        const STATIC_UTIL: &str = "Modelica.Magnetic.FundamentalWave.Utilities";
        if resolved == STATIC_UTIL {
            return "Modelica.Magnetic.QuasiStatic.FundamentalWave.Utilities".to_string();
        }
        if let Some(rest) = resolved.strip_prefix(STATIC_UTIL) {
            if rest.is_empty() || rest.starts_with('.') {
                return format!("Modelica.Magnetic.QuasiStatic.FundamentalWave.Utilities{rest}");
            }
        }
        resolved.to_string()
    }

    pub(crate) fn resolve_import_prefix(model: &Model, name: &str, current_qualified: &str) -> String {
        let name = name.trim_start_matches('.');
        if name == "Modelica.Fluid.Pipes.BaseClasses.PartialValve" {
            return "Modelica.Fluid.Valves.BaseClasses.PartialValve".to_string();
        }
        if name == "Modelica.Electrical.Analog.Interfaces.PositivePlug" {
            return "Modelica.Electrical.Polyphase.Interfaces.PositivePlug".to_string();
        }
        if name == "Modelica.Electrical.Analog.Interfaces.NegativePlug" {
            return "Modelica.Electrical.Polyphase.Interfaces.NegativePlug".to_string();
        }
        for (alias, qual) in &model.imports {
            if alias.is_empty() || qual.is_empty() {
                continue;
            }
            if name == alias {
                return qual.clone();
            }
            let prefix = format!("{}.", alias);
            if let Some(rest) = name.strip_prefix(&prefix) {
                return format!("{}.{}", qual, rest);
            }
        }
        // Top-level MSL package shorthands must win over inner-classes of the same short name
        // (e.g. a local class named Magnetic must not shadow the global Magnetic library).
        if name == "Electrical" || name.starts_with("Electrical.") {
            return format!("Modelica.{}", name);
        }
        if name == "Magnetic" || name.starts_with("Magnetic.") {
            return format!("Modelica.{}", name);
        }
        if name == "Thermal" || name.starts_with("Thermal.") {
            return format!("Modelica.{}", name);
        }
        if name == "StateGraph" || name.starts_with("StateGraph.") {
            if name == "StateGraph" {
                return "Modelica.StateGraph".to_string();
            }
            return format!("Modelica.{}", name);
        }
        if name == "FluidHeatFlow" || name.starts_with("FluidHeatFlow.") {
            return format!("Modelica.Thermal.{}", name);
        }
        if name == "FluxTubes" || name.starts_with("FluxTubes.") {
            return format!("Modelica.Magnetic.{}", name);
        }
        if name == "FundamentalWave" || name.starts_with("FundamentalWave.") {
            return format!("Modelica.Magnetic.{}", name);
        }
        // MSL: StateGraph.mo uses local prefixes `Interfaces.*` and `StateGraph.*` (library self-ref);
        // qualify so the loader does not see bare `Interfaces` / `StateGraph`.
        if current_qualified.starts_with("Modelica.StateGraph") {
            if name == "StateGraph" || name.starts_with("StateGraph.") {
                if name == "StateGraph" {
                    return "Modelica.StateGraph".to_string();
                }
                return format!("Modelica.{}", name);
            }
            if name == "Interfaces" || name.starts_with("Interfaces.") {
                if name == "Interfaces" {
                    return "Modelica.StateGraph.Interfaces".to_string();
                }
                return format!("Modelica.StateGraph.{}", name);
            }
        }
        // MSL: Pipes.mo `FlowModel` is replaceable; short name must not stop at inner-class lookup.
        if (current_qualified.starts_with("Modelica.Fluid")
            || current_qualified.starts_with("ModelicaTest.Fluid"))
            && (name == "FlowModel" || name.starts_with("FlowModel."))
        {
            let rest = name.trim_start_matches("FlowModel");
            let base = "Modelica.Fluid.Pipes.BaseClasses.FlowModels.DetailedPipeFlow";
            if rest.is_empty() {
                return base.to_string();
            }
            return format!("{base}{rest}");
        }
        // Polyphase `Basic.*` must win over same-named inner classes in Sensors/Examples (e.g. PlugToPins_*).
        if current_qualified.starts_with("Modelica.Electrical.Polyphase")
            && (name == "Basic" || name.starts_with("Basic."))
        {
            if name == "Basic" {
                return "Modelica.Electrical.Polyphase.Basic".to_string();
            }
            return format!("Modelica.Electrical.Polyphase.{}", name);
        }
        // Polyphase: unqualified `Ideal.*` refers to Electrical.Analog.Ideal (e.g. IdealDiode in Rectifier).
        if current_qualified.starts_with("Modelica.Electrical.Polyphase")
            && (name == "Ideal" || name.starts_with("Ideal."))
        {
            if name == "Ideal" {
                return "Modelica.Electrical.Analog.Ideal".to_string();
            }
            return format!("Modelica.Electrical.Analog.{}", name);
        }
        // QuasiStatic single-phase `Ideal.*` (e.g. IdealTransformer) vs. inner `Ideal`.
        if current_qualified.starts_with("Modelica.Electrical.QuasiStatic.SinglePhase")
            && (name == "Ideal" || name.starts_with("Ideal."))
        {
            if name == "Ideal" {
                return "Modelica.Electrical.QuasiStatic.SinglePhase.Ideal".to_string();
            }
            return format!("Modelica.Electrical.QuasiStatic.SinglePhase.{}", name);
        }
        if !name.contains('.') && model.inner_classes.iter().any(|c| c.name == name) {
            return name.to_string();
        }
        if name == "Mechanics" {
            return "Modelica.Mechanics".to_string();
        }
        if name.starts_with("Mechanics.") {
            return format!("Modelica.{}", name);
        }
        // Library context flags for fallback rules (by sublibrary)
        let in_blocks = current_qualified.starts_with("Modelica.Blocks");
        let in_blocks_math = current_qualified.starts_with("Modelica.Blocks.Math");
        let in_blocks_sources = current_qualified.starts_with("Modelica.Blocks.Sources");
        let in_electrical_analog = current_qualified.starts_with("Modelica.Electrical.Analog");
        let in_electrical = current_qualified.starts_with("Modelica.Electrical");
        let in_polyphase = current_qualified.starts_with("Modelica.Electrical.Polyphase");
        let in_machines = current_qualified.starts_with("Modelica.Electrical.Machines");
        let in_qs_polyphase_basic =
            current_qualified.starts_with("Modelica.Electrical.QuasiStatic.Polyphase.Basic");
        let in_qs_single_phase =
            current_qualified.starts_with("Modelica.Electrical.QuasiStatic.SinglePhase");
        let in_rotational = current_qualified.starts_with("Modelica.Mechanics.Rotational");
        let in_translational = current_qualified.starts_with("Modelica.Mechanics.Translational");
        let in_mechanics = current_qualified.starts_with("Modelica.Mechanics");
        let in_clocked_clocksignals =
            current_qualified.starts_with("Modelica.Clocked.ClockSignals");
        let in_clocked_realsignals = current_qualified.starts_with("Modelica.Clocked.RealSignals");
        let in_clocked_booleansignals =
            current_qualified.starts_with("Modelica.Clocked.BooleanSignals");
        let in_clocked_integersignals =
            current_qualified.starts_with("Modelica.Clocked.IntegerSignals");
        let in_clocked_examples = current_qualified.starts_with("Modelica.Clocked.Examples");
        let in_powerconverters =
            current_qualified.starts_with("Modelica.Electrical.PowerConverters");
        let in_batteries = current_qualified.starts_with("Modelica.Electrical.Batteries");
        let in_magnetic = current_qualified.starts_with("Modelica.Magnetic");
        let in_magnetic_fundamental_wave =
            current_qualified.starts_with("Modelica.Magnetic.FundamentalWave");
        let in_magnetic_fw_components =
            current_qualified.starts_with("Modelica.Magnetic.FundamentalWave.Components");
        let in_magnetic_fluxtubes = current_qualified.starts_with("Modelica.Magnetic.FluxTubes");
        let in_magnetic_qs_fluxtubes =
            current_qualified.starts_with("Modelica.Magnetic.QuasiStatic.FluxTubes");
        let in_magnetic_qs_fundamental_wave =
            current_qualified.starts_with("Modelica.Magnetic.QuasiStatic.FundamentalWave");
        let in_modelicatest_magnetic_fluxtubes =
            current_qualified.starts_with("ModelicaTest.Magnetic.FluxTubes");
        let in_multibody = current_qualified.starts_with("Modelica.Mechanics.MultiBody");
        let in_multibody_loops = current_qualified.starts_with("Modelica.Mechanics.MultiBody.Examples.Loops");
        let in_heattransfer = current_qualified.starts_with("Modelica.Thermal.HeatTransfer");
        let in_thermal = current_qualified.starts_with("Modelica.Thermal");
        let in_fluid = current_qualified.starts_with("Modelica.Fluid")
            || current_qualified.starts_with("ModelicaTest.Fluid");
        let in_utilities = current_qualified.starts_with("Modelica.Utilities");

        // --- Units (SI) ---
        if name == "SI" {
            return "Modelica.Units.SI".to_string();
        }
        if let Some(rest) = name.strip_prefix("SI.") {
            return format!("Modelica.Units.SI.{}", rest);
        }
        if name == "Cv" {
            return "Modelica.Units.Conversions".to_string();
        }
        if let Some(rest) = name.strip_prefix("Cv.") {
            return format!("Modelica.Units.Conversions.{}", rest);
        }
        // --- Constants / StateSelect (global) ---
        if name == "Constants" || name.starts_with("Constants.") {
            return format!("Modelica.{}", name);
        }
        if name == "StateSelect" {
            return "Modelica.StateSelect".to_string();
        }
        if name.starts_with("StateSelect.") {
            return format!("Modelica.{}", name);
        }
        // --- Electrical.Analog: pins and Interfaces ---
        if in_electrical_analog {
            if name == "PositivePin" || name == "NegativePin" || name == "Pin" {
                return format!("Modelica.Electrical.Analog.Interfaces.{}", name);
            }
            if name == "Sources" {
                return "Modelica.Electrical.Analog.Sources".to_string();
            }
            if name.starts_with("Sources.") {
                return format!("Modelica.Electrical.Analog.{}", name);
            }
            if name == "Basic" {
                return "Modelica.Electrical.Analog.Basic".to_string();
            }
            if name.starts_with("Basic.") {
                return format!("Modelica.Electrical.Analog.{}", name);
            }
            if name == "Semiconductors" {
                return "Modelica.Electrical.Analog.Semiconductors".to_string();
            }
            if name.starts_with("Semiconductors.") {
                return format!("Modelica.Electrical.Analog.{}", name);
            }
            if name == "Ideal" {
                return "Modelica.Electrical.Analog.Ideal".to_string();
            }
            if name.starts_with("Ideal.") {
                return format!("Modelica.Electrical.Analog.{}", name);
            }
            if name == "Sensors" {
                return "Modelica.Electrical.Analog.Sensors".to_string();
            }
            if name.starts_with("Sensors.") {
                return format!("Modelica.Electrical.Analog.{}", name);
            }
            if name == "Interfaces" {
                return "Modelica.Electrical.Analog.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
                return format!("Modelica.Electrical.Analog.{}", name);
            }
        }
        if in_batteries {
            if name == "ParameterRecords" {
                return "Modelica.Electrical.Batteries.ParameterRecords".to_string();
            }
            if name.starts_with("ParameterRecords.") {
                return format!("Modelica.Electrical.Batteries.{}", name);
            }
            if name == "Utilities" {
                return "Modelica.Electrical.Batteries.Utilities".to_string();
            }
            if name.starts_with("Utilities.") {
                return format!("Modelica.Electrical.Batteries.{}", name);
            }
        }
        if in_magnetic_fundamental_wave {
            if name == "FundamentalWave" {
                return "Modelica.Magnetic.FundamentalWave".to_string();
            }
            if name.starts_with("FundamentalWave.") {
                return format!("Modelica.Magnetic.{}", name);
            }
            if name == "Interfaces" {
                return "Modelica.Magnetic.FundamentalWave.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
                return format!("Modelica.Magnetic.FundamentalWave.{}", name);
            }
            if name == "Utilities" {
                return "Modelica.Magnetic.FundamentalWave.Utilities".to_string();
            }
            if name.starts_with("Utilities.") {
                return format!("Modelica.Magnetic.FundamentalWave.{}", name);
            }
            if current_qualified.contains(".BasicMachines.") {
                if name == "Components" {
                    return "Modelica.Magnetic.FundamentalWave.BasicMachines.Components".to_string();
                }
                if name.starts_with("Components.") {
                    return format!("Modelica.Magnetic.FundamentalWave.BasicMachines.{}", name);
                }
            }
            if name == "Components" {
                return "Modelica.Magnetic.FundamentalWave.Components".to_string();
            }
            if name.starts_with("Components.") {
                return format!("Modelica.Magnetic.FundamentalWave.{}", name);
            }
            if name == "Machines" {
                return "Modelica.Magnetic.FundamentalWave.BasicMachines".to_string();
            }
            if name.starts_with("Machines.") {
                let rest = name.trim_start_matches("Machines.");
                return format!("Modelica.Magnetic.FundamentalWave.BasicMachines.{}", rest);
            }
        }
        if in_magnetic_qs_fundamental_wave {
            if name == "ExampleUtilities" {
                return "Modelica.Magnetic.QuasiStatic.FundamentalWave.Examples.ExampleUtilities"
                    .to_string();
            }
            if name.starts_with("ExampleUtilities.") {
                return format!(
                    "Modelica.Magnetic.QuasiStatic.FundamentalWave.Examples.{}",
                    name
                );
            }
            if name == "Interfaces" {
                return "Modelica.Magnetic.QuasiStatic.FundamentalWave.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
                return format!("Modelica.Magnetic.QuasiStatic.FundamentalWave.{}", name);
            }
            if current_qualified.contains(".BasicMachines.") {
                if name == "Components" {
                    return "Modelica.Magnetic.QuasiStatic.FundamentalWave.BasicMachines.Components"
                        .to_string();
                }
                if name.starts_with("Components.") {
                    return format!(
                        "Modelica.Magnetic.QuasiStatic.FundamentalWave.BasicMachines.{}",
                        name
                    );
                }
            }
            if name == "Components" {
                return "Modelica.Magnetic.QuasiStatic.FundamentalWave.Components".to_string();
            }
            if name.starts_with("Components.") {
                return format!("Modelica.Magnetic.QuasiStatic.FundamentalWave.{}", name);
            }
            if name == "BaseClasses" {
                return "Modelica.Magnetic.QuasiStatic.FundamentalWave.BaseClasses".to_string();
            }
            if name.starts_with("BaseClasses.") {
                return format!("Modelica.Magnetic.QuasiStatic.FundamentalWave.{}", name);
            }
            if name == "Utilities" {
                return "Modelica.Magnetic.QuasiStatic.FundamentalWave.Utilities".to_string();
            }
            if name.starts_with("Utilities.") {
                return format!("Modelica.Magnetic.QuasiStatic.FundamentalWave.{}", name);
            }
            if name == "Sensors" {
                return "Modelica.Magnetic.QuasiStatic.FundamentalWave.Sensors".to_string();
            }
            if name.starts_with("Sensors.") {
                return format!("Modelica.Magnetic.QuasiStatic.FundamentalWave.{}", name);
            }
            if name == "Machines" {
                return "Modelica.Magnetic.QuasiStatic.FundamentalWave.BasicMachines".to_string();
            }
            if name.starts_with("Machines.") {
                let rest = name.trim_start_matches("Machines.");
                return format!(
                    "Modelica.Magnetic.QuasiStatic.FundamentalWave.BasicMachines.{}",
                    rest
                );
            }
        }
        if in_magnetic_fluxtubes || in_modelicatest_magnetic_fluxtubes {
            if name == "Material" {
                return "Modelica.Magnetic.FluxTubes.Material".to_string();
            }
            if name.starts_with("Material.") {
                return format!("Modelica.Magnetic.FluxTubes.{}", name);
            }
            if name == "BaseClasses" {
                return "Modelica.Magnetic.FluxTubes.BaseClasses".to_string();
            }
            if name.starts_with("BaseClasses.") {
                return format!("Modelica.Magnetic.FluxTubes.{}", name);
            }
            if name == "Interfaces" {
                return "Modelica.Magnetic.FluxTubes.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
                return format!("Modelica.Magnetic.FluxTubes.{}", name);
            }
            if name == "Basic" {
                return "Modelica.Magnetic.FluxTubes.Basic".to_string();
            }
            if name.starts_with("Basic.") {
                return format!("Modelica.Magnetic.FluxTubes.{}", name);
            }
            if name == "Sources" {
                return "Modelica.Magnetic.FluxTubes.Sources".to_string();
            }
            if name.starts_with("Sources.") {
                return format!("Modelica.Magnetic.FluxTubes.{}", name);
            }
            if name == "Sensors" {
                return "Modelica.Magnetic.FluxTubes.Sensors".to_string();
            }
            if name.starts_with("Sensors.") {
                return format!("Modelica.Magnetic.FluxTubes.{}", name);
            }
            if name == "Shapes" {
                return "Modelica.Magnetic.FluxTubes.Shapes".to_string();
            }
            if name.starts_with("Shapes.") {
                return format!("Modelica.Magnetic.FluxTubes.{}", name);
            }
        }
        if in_magnetic_qs_fluxtubes {
            if name == "Interfaces" {
                return "Modelica.Magnetic.QuasiStatic.FluxTubes.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
                return format!("Modelica.Magnetic.QuasiStatic.FluxTubes.{}", name);
            }
            if name == "Basic" {
                return "Modelica.Magnetic.QuasiStatic.FluxTubes.Basic".to_string();
            }
            if name.starts_with("Basic.") {
                return format!("Modelica.Magnetic.QuasiStatic.FluxTubes.{}", name);
            }
            if name == "Sources" {
                return "Modelica.Magnetic.QuasiStatic.FluxTubes.Sources".to_string();
            }
            if name.starts_with("Sources.") {
                return format!("Modelica.Magnetic.QuasiStatic.FluxTubes.{}", name);
            }
            if name == "Sensors" {
                return "Modelica.Magnetic.QuasiStatic.FluxTubes.Sensors".to_string();
            }
            if name.starts_with("Sensors.") {
                return format!("Modelica.Magnetic.QuasiStatic.FluxTubes.{}", name);
            }
            if name == "Shapes" {
                return "Modelica.Magnetic.QuasiStatic.FluxTubes.Shapes".to_string();
            }
            if name.starts_with("Shapes.") {
                return format!("Modelica.Magnetic.QuasiStatic.FluxTubes.{}", name);
            }
        }
        if in_magnetic_fw_components && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return "Modelica.Magnetic.FundamentalWave.Interfaces".to_string();
            }
            return format!("Modelica.Magnetic.FundamentalWave.{}", name);
        }
        if in_magnetic && current_qualified.contains(".Examples.") {
            let parent = current_qualified
                .rsplit_once('.')
                .map(|(p, _)| p)
                .unwrap_or(current_qualified);
            if name == "Components" || name == "BaseClasses" || name == "Utilities" {
                return format!("{}.{}", parent, name);
            }
            if let Some(rest) = name.strip_prefix("Components.") {
                return format!("{}.Components.{}", parent, rest);
            }
            if let Some(rest) = name.strip_prefix("BaseClasses.") {
                return format!("{}.BaseClasses.{}", parent, rest);
            }
            if let Some(rest) = name.strip_prefix("Utilities.") {
                return format!("{}.Utilities.{}", parent, rest);
            }
        }
        if current_qualified.starts_with("Modelica.Electrical.Analog.Examples.OpAmps") {
            if name == "OpAmpCircuits" {
                return "Modelica.Electrical.Analog.Examples.OpAmps.OpAmpCircuits".to_string();
            }
            if name.starts_with("OpAmpCircuits.") {
                return format!("Modelica.Electrical.Analog.Examples.OpAmps.{}", name);
            }
            if name == "OpAmps" {
                return "Modelica.Electrical.Analog.Examples.OpAmps".to_string();
            }
            if name.starts_with("OpAmps.") {
                return format!("Modelica.Electrical.Analog.Examples.{}", name);
            }
        }
        // --- Electrical.Polyphase: Interfaces ---
        if in_polyphase && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return "Modelica.Electrical.Polyphase.Interfaces".to_string();
            }
            return format!("Modelica.Electrical.Polyphase.{}", name);
        }
        if in_polyphase {
            if name == "Basic" {
                return "Modelica.Electrical.Polyphase.Basic".to_string();
            }
            if name.starts_with("Basic.") {
                return format!("Modelica.Electrical.Polyphase.{}", name);
            }
            if name == "Sources" {
                return "Modelica.Electrical.Polyphase.Sources".to_string();
            }
            if name.starts_with("Sources.") {
                return format!("Modelica.Electrical.Polyphase.{}", name);
            }
            if name == "Sensors" {
                return "Modelica.Electrical.Polyphase.Sensors".to_string();
            }
            if name.starts_with("Sensors.") {
                return format!("Modelica.Electrical.Polyphase.{}", name);
            }
        }
        if current_qualified.starts_with("Modelica.Electrical.QuasiStatic.Polyphase")
            && (name == "Interfaces" || name.starts_with("Interfaces."))
        {
            if name == "Interfaces" {
                return "Modelica.Electrical.QuasiStatic.Polyphase.Interfaces".to_string();
            }
            return format!("Modelica.Electrical.QuasiStatic.Polyphase.{}", name);
        }
        if current_qualified.starts_with("Modelica.Electrical.QuasiStatic.Polyphase") {
            if name == "Basic" {
                return "Modelica.Electrical.QuasiStatic.Polyphase.Basic".to_string();
            }
            if name.starts_with("Basic.") {
                return format!("Modelica.Electrical.QuasiStatic.Polyphase.{}", name);
            }
            if name == "Sources" {
                return "Modelica.Electrical.QuasiStatic.Polyphase.Sources".to_string();
            }
            if name.starts_with("Sources.") {
                return format!("Modelica.Electrical.QuasiStatic.Polyphase.{}", name);
            }
            if name == "Sensors" {
                return "Modelica.Electrical.QuasiStatic.Polyphase.Sensors".to_string();
            }
            if name.starts_with("Sensors.") {
                return format!("Modelica.Electrical.QuasiStatic.Polyphase.{}", name);
            }
        }
        if in_qs_single_phase {
            if name == "Basic" {
                return "Modelica.Electrical.QuasiStatic.SinglePhase.Basic".to_string();
            }
            if name.starts_with("Basic.") {
                return format!("Modelica.Electrical.QuasiStatic.SinglePhase.{}", name);
            }
            if name == "Interfaces" {
                return "Modelica.Electrical.QuasiStatic.SinglePhase.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
                return format!("Modelica.Electrical.QuasiStatic.SinglePhase.{}", name);
            }
            if name == "Sources" {
                return "Modelica.Electrical.QuasiStatic.SinglePhase.Sources".to_string();
            }
            if name.starts_with("Sources.") {
                return format!("Modelica.Electrical.QuasiStatic.SinglePhase.{}", name);
            }
            if name == "Sensors" {
                return "Modelica.Electrical.QuasiStatic.SinglePhase.Sensors".to_string();
            }
            if name.starts_with("Sensors.") {
                return format!("Modelica.Electrical.QuasiStatic.SinglePhase.{}", name);
            }
            if name == "Utilities" {
                return "Modelica.Electrical.QuasiStatic.SinglePhase.Utilities".to_string();
            }
            if name.starts_with("Utilities.") {
                return format!("Modelica.Electrical.QuasiStatic.SinglePhase.{}", name);
            }
        }
        // --- Electrical.QuasiStatic.Polyphase.Basic: local helpers like PlugToPins_* ---
        if in_qs_polyphase_basic {
            if name == "PlugToPins_p"
                || name == "PlugToPins_n"
                || name == "PlugToPin_p"
                || name == "PlugToPin_n"
            {
                return format!("Modelica.Electrical.QuasiStatic.Polyphase.Basic.{}", name);
            }
        }
        // --- Electrical (Machines etc.): allow direct pin shorthand ---
        if in_electrical {
            // PowerConverters.DCAC.* is referenced from DCDC/HBridge while flattening non-PC packages.
            if name == "DCAC" {
                return "Modelica.Electrical.PowerConverters.DCAC".to_string();
            }
            if name.starts_with("DCAC.") {
                return format!("Modelica.Electrical.PowerConverters.{}", name);
            }
            if in_powerconverters && current_qualified.starts_with("Modelica.Electrical.PowerConverters")
            {
                if name == "Interfaces" {
                    return "Modelica.Electrical.PowerConverters.Interfaces".to_string();
                }
                if name.starts_with("Interfaces.") {
                    return format!("Modelica.Electrical.PowerConverters.{}", name);
                }
            }
            if current_qualified.starts_with("Modelica.Electrical.PowerConverters.Examples") {
                let parent = current_qualified
                    .rsplit_once('.')
                    .map(|(p, _)| p)
                    .unwrap_or(current_qualified);
                if name == "ExampleTemplates" {
                    return format!("{}.ExampleTemplates", parent);
                }
                if let Some(rest) = name.strip_prefix("ExampleTemplates.") {
                    return format!("{}.ExampleTemplates.{}", parent, rest);
                }
            }
            if current_qualified.starts_with("Modelica.Electrical.PowerConverters") {
                if name == "Icons" {
                    return "Modelica.Electrical.PowerConverters.Icons".to_string();
                }
                if name.starts_with("Icons.") {
                    return format!("Modelica.Electrical.PowerConverters.{}", name);
                }
            }
            if name == "ComplexBlocks" {
                return "Modelica.ComplexBlocks".to_string();
            }
            if name.starts_with("ComplexBlocks.") {
                return format!("Modelica.{}", name);
            }
            if name == "Mechanics" {
                return "Modelica.Mechanics".to_string();
            }
            if name.starts_with("Mechanics.") {
                return format!("Modelica.{}", name);
            }
            if name == "Analog" {
                return "Modelica.Electrical.Analog".to_string();
            }
            if name.starts_with("Analog.") {
                return format!("Modelica.Electrical.{}", name);
            }
            if name == "Polyphase" {
                return "Modelica.Electrical.Polyphase".to_string();
            }
            if name.starts_with("Polyphase.") {
                return format!("Modelica.Electrical.{}", name);
            }
            if name == "QuasiStatic" {
                return "Modelica.Electrical.QuasiStatic".to_string();
            }
            if name.starts_with("QuasiStatic.") {
                return format!("Modelica.Electrical.{}", name);
            }
            if name == "PowerConverters" {
                return "Modelica.Electrical.PowerConverters".to_string();
            }
            if name.starts_with("PowerConverters.") {
                return format!("Modelica.Electrical.{}", name);
            }
            if name == "SinglePhase" {
                return "Modelica.Electrical.QuasiStatic.SinglePhase".to_string();
            }
            if name.starts_with("SinglePhase.") {
                return format!("Modelica.Electrical.QuasiStatic.{}", name);
            }
            if name == "PositivePin" || name == "NegativePin" || name == "Pin" {
                return format!("Modelica.Electrical.Analog.Interfaces.{}", name);
            }
        }
        // --- Electrical (Machines, etc.): extend to full Electrical when in Electrical ---
        if in_electrical && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return "Modelica.Electrical.Analog.Interfaces".to_string();
            }
            return format!("Modelica.Electrical.Analog.{}", name);
        }
        if in_machines {
            if name == "ControlledDCDrives" {
                return "Modelica.Electrical.Machines.Examples.ControlledDCDrives".to_string();
            }
            if name.starts_with("ControlledDCDrives.") {
                return format!("Modelica.Electrical.Machines.Examples.{}", name);
            }
            if name == "BasicMachines" {
                return "Modelica.Electrical.Machines.BasicMachines".to_string();
            }
            if name.starts_with("BasicMachines.") {
                return format!("Modelica.Electrical.Machines.{}", name);
            }
            if name == "Utilities" {
                return "Modelica.Electrical.Machines.Utilities".to_string();
            }
            if name.starts_with("Utilities.") {
                return format!("Modelica.Electrical.Machines.{}", name);
            }
            if name == "SpacePhasors" {
                return "Modelica.Electrical.Machines.SpacePhasors".to_string();
            }
            if name.starts_with("SpacePhasors.") {
                return format!("Modelica.Electrical.Machines.{}", name);
            }
            if name == "Sensors" {
                return "Modelica.Electrical.Machines.Sensors".to_string();
            }
            if name.starts_with("Sensors.") {
                return format!("Modelica.Electrical.Machines.{}", name);
            }
            if name == "Components" {
                return "Modelica.Electrical.Machines.BasicMachines.Components".to_string();
            }
            if name.starts_with("Components.") {
                let rest = name.trim_start_matches("Components.");
                return format!("Modelica.Electrical.Machines.BasicMachines.Components.{}", rest);
            }
            if name == "Machines" {
                return "Modelica.Electrical.Machines".to_string();
            }
            if name.starts_with("Machines.") {
                let rest = name.trim_start_matches("Machines.");
                return format!("Modelica.Electrical.Machines.{}", rest);
            }
        }
        // --- Mechanics.Rotational: flanges, Support, Interfaces ---
        if in_rotational {
            if name == "Flange_a" || name == "Flange_b" || name == "Support" {
                return format!("Modelica.Mechanics.Rotational.Interfaces.{}", name);
            }
            if name == "Sources" {
                return "Modelica.Mechanics.Rotational.Sources".to_string();
            }
            if name.starts_with("Sources.") {
                return format!("Modelica.Mechanics.Rotational.{}", name);
            }
            if name == "Components" {
                return "Modelica.Mechanics.Rotational.Components".to_string();
            }
            if name.starts_with("Components.") {
                return format!("Modelica.Mechanics.Rotational.{}", name);
            }
            if name == "Sensors" {
                return "Modelica.Mechanics.Rotational.Sensors".to_string();
            }
            if name.starts_with("Sensors.") {
                return format!("Modelica.Mechanics.Rotational.{}", name);
            }
            if name == "Interfaces" {
                return "Modelica.Mechanics.Rotational.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
                return format!("Modelica.Mechanics.Rotational.{}", name);
            }
        }
        // --- Mechanics.Translational: flanges, Support, Interfaces ---
        if in_translational {
            if name == "Flange_a" || name == "Flange_b" || name == "Support" {
                return format!("Modelica.Mechanics.Translational.Interfaces.{}", name);
            }
            if name == "Components" {
                return "Modelica.Mechanics.Translational.Components".to_string();
            }
            if name.starts_with("Components.") {
                return format!("Modelica.Mechanics.Translational.{}", name);
            }
            if name == "Sources" {
                return "Modelica.Mechanics.Translational.Sources".to_string();
            }
            if name.starts_with("Sources.") {
                return format!("Modelica.Mechanics.Translational.{}", name);
            }
            if name == "Interfaces" {
                return "Modelica.Mechanics.Translational.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
                return format!("Modelica.Mechanics.Translational.{}", name);
            }
        }
        if in_mechanics {
            if name == "MultiBody" {
                return "Modelica.Mechanics.MultiBody".to_string();
            }
            if name.starts_with("MultiBody.") {
                return format!("Modelica.Mechanics.{}", name);
            }
            if name == "Rotational" {
                return "Modelica.Mechanics.Rotational".to_string();
            }
            if name.starts_with("Rotational.") {
                return format!("Modelica.Mechanics.{}", name);
            }
            if name == "Translational" {
                return "Modelica.Mechanics.Translational".to_string();
            }
            if name.starts_with("Translational.") {
                return format!("Modelica.Mechanics.{}", name);
            }
            if name == "HeatTransfer" {
                return "Modelica.Thermal.HeatTransfer".to_string();
            }
            if name.starts_with("HeatTransfer.") {
                return format!("Modelica.Thermal.{}", name);
            }
        }
        // --- Clocked.*: local Interfaces shorthand inside subpackages ---
        if in_clocked_clocksignals && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return "Modelica.Clocked.ClockSignals.Interfaces".to_string();
            }
            return format!("Modelica.Clocked.ClockSignals.{}", name);
        }
        if in_clocked_realsignals && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return "Modelica.Clocked.RealSignals.Interfaces".to_string();
            }
            return format!("Modelica.Clocked.RealSignals.{}", name);
        }
        if in_clocked_booleansignals && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return "Modelica.Clocked.BooleanSignals.Interfaces".to_string();
            }
            return format!("Modelica.Clocked.BooleanSignals.{}", name);
        }
        if in_clocked_integersignals && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return "Modelica.Clocked.IntegerSignals.Interfaces".to_string();
            }
            return format!("Modelica.Clocked.IntegerSignals.{}", name);
        }
        if in_clocked_examples {
            if current_qualified.starts_with("Modelica.Clocked.Examples.Systems") {
                if name == "Utilities" {
                    return "Modelica.Clocked.Examples.Systems.Utilities".to_string();
                }
                if name.starts_with("Utilities.") {
                    return format!("Modelica.Clocked.Examples.Systems.{}", name);
                }
            }
            if name == "ClockSignals" {
                return "Modelica.Clocked.ClockSignals".to_string();
            }
            if name.starts_with("ClockSignals.") {
                return format!("Modelica.Clocked.{}", name);
            }
            if name == "BooleanSignals" {
                return "Modelica.Clocked.BooleanSignals".to_string();
            }
            if name.starts_with("BooleanSignals.") {
                return format!("Modelica.Clocked.{}", name);
            }
            if name == "RealSignals" {
                return "Modelica.Clocked.RealSignals".to_string();
            }
            if name.starts_with("RealSignals.") {
                return format!("Modelica.Clocked.{}", name);
            }
            if name == "IntegerSignals" {
                return "Modelica.Clocked.IntegerSignals".to_string();
            }
            if name.starts_with("IntegerSignals.") {
                return format!("Modelica.Clocked.{}", name);
            }
        }
        // --- Mechanics.MultiBody: World, Joints, Utilities, Frames, Interfaces, Types ---
        if in_multibody {
            if current_qualified.starts_with("Modelica.Mechanics.MultiBody.Examples.Loops.Utilities")
                && !name.contains('.')
                && !matches!(name, "Real" | "Integer" | "Boolean" | "String")
            {
                return format!(
                    "Modelica.Mechanics.MultiBody.Examples.Loops.Utilities.{}",
                    name
                );
            }
            if current_qualified
                .starts_with("Modelica.Mechanics.MultiBody.Examples.Systems.RobotR3")
            {
                if name == "Utilities" {
                    return "Modelica.Mechanics.MultiBody.Examples.Systems.RobotR3.Utilities"
                        .to_string();
                }
                if name.starts_with("Utilities.") {
                    return format!(
                        "Modelica.Mechanics.MultiBody.Examples.Systems.RobotR3.{}",
                        name
                    );
                }
            }
            // MultiBody.Examples.Loops.Utilities is a real subpackage used by Engine examples.
            if in_multibody_loops {
                if name == "Utilities" {
                    return "Modelica.Mechanics.MultiBody.Examples.Loops.Utilities".to_string();
                }
                if name.starts_with("Utilities.") {
                    return format!("Modelica.Mechanics.MultiBody.Examples.Loops.{}", name);
                }
            }
            // MultiBody: common short name for the inner World instance.
            if name == "world" || name.starts_with("world.") {
                let rest = name.trim_start_matches("world");
                let rest = rest.trim_start_matches('.');
                if rest.is_empty() {
                    return "Modelica.Mechanics.MultiBody.World".to_string();
                }
                return format!("Modelica.Mechanics.MultiBody.World.{}", rest);
            }
            if name == "World" {
                return "Modelica.Mechanics.MultiBody.World".to_string();
            }
            if name == "Joints" {
                return "Modelica.Mechanics.MultiBody.Joints".to_string();
            }
            if name.starts_with("Joints.") {
                return format!("Modelica.Mechanics.MultiBody.{}", name);
            }
            // Common subpackages referenced with short names.
            if name == "Parts" {
                return "Modelica.Mechanics.MultiBody.Parts".to_string();
            }
            if name.starts_with("Parts.") {
                return format!("Modelica.Mechanics.MultiBody.{}", name);
            }
            if name == "Forces" {
                return "Modelica.Mechanics.MultiBody.Forces".to_string();
            }
            if name.starts_with("Forces.") {
                return format!("Modelica.Mechanics.MultiBody.{}", name);
            }
            // Inside MultiBody.Parts, unqualified type names commonly refer to Parts.*
            // (e.g. BodyBox contains "Body body" and "FixedTranslation ...").
            let is_primitive_short = matches!(name, "Real" | "Integer" | "Boolean" | "String");
            if !name.contains('.')
                && !is_primitive_short
                && current_qualified.contains(".MultiBody.Parts.")
            {
                return format!("Modelica.Mechanics.MultiBody.Parts.{}", name);
            }
            // Inside MultiBody.Sensors, unqualified type names commonly refer to Sensors.*
            if !name.contains('.')
                && !is_primitive_short
                && current_qualified.contains(".MultiBody.Sensors.")
            {
                return format!("Modelica.Mechanics.MultiBody.Sensors.{}", name);
            }
            if name == "Utilities" {
                return "Modelica.Mechanics.MultiBody.Utilities".to_string();
            }
            if name.starts_with("Utilities.") {
                return format!("Modelica.Mechanics.MultiBody.{}", name);
            }
            if name == "Interfaces" {
                return "Modelica.Mechanics.MultiBody.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
                return format!("Modelica.Mechanics.MultiBody.{}", name);
            }
            if name == "Sensors" {
                return "Modelica.Mechanics.MultiBody.Sensors".to_string();
            }
            if name.starts_with("Sensors.") {
                return format!("Modelica.Mechanics.MultiBody.{}", name);
            }
            if name == "Frames" || name.starts_with("Frames.") {
                return format!("Modelica.Mechanics.MultiBody.{}", name);
            }
            if name == "Types" || name.starts_with("Types.") {
                return format!("Modelica.Mechanics.MultiBody.{}", name);
            }
            if name == "Visualizers" || name.starts_with("Visualizers.") {
                return format!("Modelica.Mechanics.MultiBody.{}", name);
            }
            if current_qualified.starts_with("Modelica.Mechanics.MultiBody.Visualizers") {
                if name == "Advanced" {
                    return "Modelica.Mechanics.MultiBody.Visualizers.Advanced".to_string();
                }
                if name.starts_with("Advanced.") {
                    return format!("Modelica.Mechanics.MultiBody.Visualizers.{}", name);
                }
            }
            // MSL: many MultiBody subpackages have an Internal package (e.g. Frames.Internal, Sensors.Internal).
            // Resolve based on local context to avoid mis-binding `Internal.*`.
            if name == "Internal" {
                if current_qualified.starts_with("Modelica.Mechanics.MultiBody.Frames") {
                    return "Modelica.Mechanics.MultiBody.Frames.Internal".to_string();
                }
                if current_qualified.starts_with("Modelica.Mechanics.MultiBody.Forces") {
                    return "Modelica.Mechanics.MultiBody.Forces.Internal".to_string();
                }
                return "Modelica.Mechanics.MultiBody.Sensors.Internal".to_string();
            }
            if name.starts_with("Internal.") {
                if current_qualified.starts_with("Modelica.Mechanics.MultiBody.Frames") {
                    return format!("Modelica.Mechanics.MultiBody.Frames.{}", name);
                }
                if current_qualified.starts_with("Modelica.Mechanics.MultiBody.Forces") {
                    return format!("Modelica.Mechanics.MultiBody.Forces.{}", name);
                }
                return format!("Modelica.Mechanics.MultiBody.Sensors.{}", name);
            }
        }
        // --- Thermal.HeatTransfer: HeatPort, Interfaces ---
        if in_heattransfer {
            if name == "HeatPort_a" || name == "HeatPort_b" || name == "HeatPort" {
                return format!("Modelica.Thermal.HeatTransfer.Interfaces.{}", name);
            }
            if name == "Components" || name.starts_with("Components.") {
                return format!("Modelica.Thermal.HeatTransfer.{}", name);
            }
            if name == "Celsius" || name.starts_with("Celsius.") {
                return format!("Modelica.Thermal.HeatTransfer.{}", name);
            }
            if name == "Interfaces" {
                return "Modelica.Thermal.HeatTransfer.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
                return format!("Modelica.Thermal.HeatTransfer.{}", name);
            }
        }
        if in_thermal {
            if name == "HeatTransfer" {
                return "Modelica.Thermal.HeatTransfer".to_string();
            }
            if name.starts_with("HeatTransfer.") {
                return format!("Modelica.Thermal.{}", name);
            }
        }
        // --- Fluid: Interfaces, Types, Media ---
        if in_fluid {
            if current_qualified.contains(".Examples.") {
                let parent = current_qualified
                    .rsplit_once('.')
                    .map(|(p, _)| p)
                    .unwrap_or(current_qualified);
                if name == "Components" || name == "BaseClasses" || name == "Utilities" {
                    return format!("{}.{}", parent, name);
                }
                if let Some(rest) = name.strip_prefix("Components.") {
                    return format!("{}.Components.{}", parent, rest);
                }
                if let Some(rest) = name.strip_prefix("BaseClasses.") {
                    return format!("{}.BaseClasses.{}", parent, rest);
                }
                if let Some(rest) = name.strip_prefix("Utilities.") {
                    return format!("{}.Utilities.{}", parent, rest);
                }
            }
            if name == "Fittings" {
                return "Modelica.Fluid.Fittings".to_string();
            }
            if name.starts_with("Fittings.") {
                return format!("Modelica.Fluid.{}", name);
            }
            if name == "System" {
                return "Modelica.Fluid.System".to_string();
            }
            if name.starts_with("System.") {
                return format!("Modelica.Fluid.{}", name);
            }
            if name == "Interfaces" {
                return "Modelica.Fluid.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
                let rest = &name["Interfaces.".len()..];
                if rest.starts_with("Step")
                    || rest.starts_with("Transition")
                    || rest.starts_with("CompositeStep")
                    || rest.starts_with("PartialStep")
                    || rest.starts_with("PartialTransition")
                    || rest.starts_with("PartialStateGraphIcon")
                {
                    return format!("Modelica.StateGraph.Interfaces.{rest}");
                }
                return format!("Modelica.Fluid.{}", name);
            }
            if name == "Types" || name.starts_with("Types.") {
                return format!("Modelica.Fluid.{}", name);
            }
            if name == "Sources" {
                return "Modelica.Fluid.Sources".to_string();
            }
            if name.starts_with("Sources.") {
                return format!("Modelica.Fluid.{}", name);
            }
            if name == "Sensors" {
                return "Modelica.Fluid.Sensors".to_string();
            }
            if name.starts_with("Sensors.") {
                return format!("Modelica.Fluid.{}", name);
            }
            // WallFriction packages are nested under Modelica.Fluid.Pipes.mo.
            // Some code refers to QuadraticTurbulent.* without full qualification.
            if name == "QuadraticTurbulent" {
                return "Modelica.Fluid.Pipes.BaseClasses.WallFriction.QuadraticTurbulent"
                    .to_string();
            }
            if name.starts_with("QuadraticTurbulent.") {
                return format!("Modelica.Fluid.Pipes.BaseClasses.WallFriction.{}", name);
            }
            if name == "Valves" {
                return "Modelica.Fluid.Valves".to_string();
            }
            if name.starts_with("Valves.") {
                return format!("Modelica.Fluid.{}", name);
            }
            if name == "Pipes" {
                return "Modelica.Fluid.Pipes".to_string();
            }
            if name.starts_with("Pipes.") {
                return format!("Modelica.Fluid.{}", name);
            }
            if name == "Vessels" {
                return "Modelica.Fluid.Vessels".to_string();
            }
            if name.starts_with("Vessels.") {
                return format!("Modelica.Fluid.{}", name);
            }
            if name == "VesselFluidPorts_b" || name.starts_with("VesselFluidPorts_b.") {
                return format!("Modelica.Fluid.Vessels.BaseClasses.{}", name);
            }
            if name == "HeatTransfer" || name.starts_with("HeatTransfer.") {
                return format!("Modelica.Fluid.Vessels.BaseClasses.{}", name);
            }
            if name == "Machines" || name.starts_with("Machines.") {
                return format!("Modelica.Fluid.{}", name);
            }
            // Common shorthand inside Modelica.Fluid.Pipes.* and Modelica.Fluid.Vessels.* sources
            if name == "BaseClasses" {
                if current_qualified.starts_with("Modelica.Fluid.Pipes") {
                    return "Modelica.Fluid.Pipes.BaseClasses".to_string();
                }
                if current_qualified.starts_with("Modelica.Fluid.Vessels") {
                    return "Modelica.Fluid.Vessels.BaseClasses".to_string();
                }
            }
            if name.starts_with("BaseClasses.") {
                if current_qualified.starts_with("Modelica.Fluid.Pipes") {
                    return format!("Modelica.Fluid.Pipes.{}", name);
                }
                if current_qualified.starts_with("Modelica.Fluid.Vessels") {
                    return format!("Modelica.Fluid.Vessels.{}", name);
                }
            }
            // Fallback: unqualified BaseClasses.* inside Fluid examples; default to Pipes.BaseClasses.*.
            if name == "BaseClasses" {
                return "Modelica.Fluid.Pipes.BaseClasses".to_string();
            }
            if name.starts_with("BaseClasses.") {
                let candidate = format!("Modelica.Fluid.Pipes.{}", name);
                if candidate == "Modelica.Fluid.Pipes.BaseClasses.PartialValve" {
                    return "Modelica.Fluid.Valves.BaseClasses.PartialValve".to_string();
                }
                return candidate;
            }
            if name.starts_with("Valves.BaseClasses.") {
                return format!("Modelica.Fluid.{}", name);
            }
            if name == "MinLimiter" || name.starts_with("MinLimiter.") {
                return format!("Modelica.Fluid.Valves.BaseClasses.PartialValve.{}", name);
            }
            if name == "Thermal" || name.starts_with("Thermal.") {
                return format!("Modelica.{}", name);
            }
            if name == "FlowModel" || name.starts_with("FlowModel.") {
                // FlowModel is a replaceable model; by default it aliases DetailedPipeFlow.
                // For validation we resolve it directly to the concrete default implementation.
                let rest = name.trim_start_matches("FlowModel");
                let base = "Modelica.Fluid.Pipes.BaseClasses.FlowModels.DetailedPipeFlow";
                if rest.is_empty() {
                    return base.to_string();
                }
                return format!("{}{}", base, rest);
            }
        }
        // --- Blocks: Interfaces, Types, Sources, Math, Nonlinear, Continuous, Logical ---
        if name == "Blocks" {
            return "Modelica.Blocks".to_string();
        }
        if name == "Clocked" {
            return "Modelica.Clocked".to_string();
        }
        if name.starts_with("Clocked.") {
            return format!("Modelica.{}", name);
        }
        if name == "MultiBody" {
            return "Modelica.Mechanics.MultiBody".to_string();
        }
        if name.starts_with("MultiBody.") {
            return format!("Modelica.Mechanics.{}", name);
        }
        if name == "ComplexBlocks" {
            return "Modelica.ComplexBlocks".to_string();
        }
        if name.starts_with("ComplexBlocks.") {
            return format!("Modelica.{}", name);
        }
        if name.starts_with("Blocks.") {
            return format!("Modelica.{}", name);
        }
        if in_blocks {
            if in_blocks_math {
                if name == "MultiProduct" {
                    return "Modelica.Blocks.Math.MultiProduct".to_string();
                }
                if name == "Mean" {
                    return "Modelica.Blocks.Math.Mean".to_string();
                }
            }
            if name == "Interfaces" {
                return "Modelica.Blocks.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
                return format!("Modelica.Blocks.{}", name);
            }
            if name == "Types" {
                return "Modelica.Blocks.Types".to_string();
            }
            if name.starts_with("Types.") {
                return format!("Modelica.Blocks.{}", name);
            }
            if name == "Sources" {
                return "Modelica.Blocks.Sources".to_string();
            }
            if name.starts_with("Sources.") {
                return format!("Modelica.Blocks.{}", name);
            }
            if name == "Math" {
                return "Modelica.Blocks.Math".to_string();
            }
            if name.starts_with("Math.") {
                return format!("Modelica.Blocks.{}", name);
            }
            if name == "Nonlinear" || name.starts_with("Nonlinear.") {
                return format!("Modelica.Blocks.{}", name);
            }
            if name == "Continuous" || name.starts_with("Continuous.") {
                return format!("Modelica.Blocks.{}", name);
            }
            if name == "Logical" || name.starts_with("Logical.") {
                return format!("Modelica.Blocks.{}", name);
            }
            if name == "Internal" {
                return "Modelica.Blocks.Types.Internal".to_string();
            }
            if name.starts_with("Internal.") {
                return format!("Modelica.Blocks.Types.{}", name);
            }
            if name == "CombiTimeTable" && in_blocks_sources {
                return "Modelica.Blocks.Sources.CombiTimeTable".to_string();
            }
            if name.starts_with("CombiTimeTable.") && in_blocks_sources {
                return format!("Modelica.Blocks.Sources.{}", name);
            }
        }
        // --- Utilities: Internal, Strings, Files, etc. ---
        if in_utilities {
            if name == "Internal" {
                return "Modelica.Utilities.Internal".to_string();
            }
            if name.starts_with("Internal.") {
                return format!("Modelica.Utilities.{}", name);
            }
        }
        // --- Blocks.Interfaces (global shorthand for RealInput etc.) ---
        if name == "RealInput"
            || name == "RealOutput"
            || name == "BooleanInput"
            || name == "BooleanOutput"
            || name == "IntegerInput"
            || name == "IntegerOutput"
        {
            return format!("Modelica.Blocks.Interfaces.{}", name);
        }
        name.to_string()
    }

    pub fn new() -> Self {
        Flattener {
            loader: ModelLoader::new(),
        }
    }

    /// root_name: model name used to load root (e.g. "TestLib/InitDummy") for DBG-4 source location in errors.
    pub fn flatten(
        &mut self,
        root: &mut Arc<Model>,
        root_name: &str,
    ) -> Result<FlattenedModel, FlattenError> {
        self.flatten_inheritance(root, root_name)?;
        let model = root.as_ref();
        let mut flat = FlattenedModel {
            declarations: Vec::new(),
            equations: Vec::new(),
            algorithms: Vec::new(),
            initial_equations: Vec::new(),
            initial_algorithms: Vec::new(),
            connections: Vec::new(),
            conditional_connections: Vec::new(),
            instances: HashMap::new(),
            array_sizes: HashMap::new(),
            clocked_var_names: std::collections::HashSet::new(),
            clock_partitions: Vec::new(),
            clock_signal_connections: Vec::new(),
        };
        self.expand_declarations(Arc::clone(root), "", &mut flat, Some(root_name))?;
        self.expand_equations(model, "", &mut flat);
        self.expand_algorithms(model, "", &mut flat);
        self.expand_initial_equations(model, "", &mut flat);
        self.expand_initial_algorithms(model, "", &mut flat);
        resolve_connections(&mut flat, Some(root_name), &self.loader)?;
        self.infer_clocked_variables(&mut flat);
        Ok(flat)
    }

    pub(crate) fn qualify_in_scope(current_qualified: &str, name: &str) -> String {
        if name.contains('.') || name.contains('/') {
            return name.to_string();
        }
        if let Some((parent, _)) = current_qualified.rsplit_once('.') {
            return format!("{}.{}", parent, name);
        }
        name.to_string()
    }

    fn qualify_in_current_class(current_qualified: &str, name: &str) -> String {
        if current_qualified.is_empty() || name.contains('.') || name.contains('/') {
            return name.to_string();
        }
        format!("{}.{}", current_qualified, name)
    }

    /// Iterative flatten_inheritance to avoid stack overflow on deep extends chains.
    /// Frame: (parent_arc_or_none, current_model_arc, qualified_name, extends_clauses, next_index).
    pub(crate) fn flatten_inheritance(
        &mut self,
        arc: &mut Arc<Model>,
        current_qualified: &str,
    ) -> Result<(), FlattenError> {
        let model = Arc::make_mut(arc);
        let extends = std::mem::take(&mut model.extends);
        type Frame = (Option<Arc<Model>>, Arc<Model>, String, Vec<ExtendsClause>, usize);
        let mut stack: Vec<Frame> = vec![(None, Arc::clone(arc), current_qualified.to_string(), extends, 0)];

        while let Some((parent, current, qual, ext, idx)) = stack.pop() {
            if idx >= ext.len() {
                if let Some(mut p) = parent {
                    merge_models(Arc::make_mut(&mut p), current.as_ref());
                }
                continue;
            }
            let clause = &ext[idx];
            let base_name = Self::resolve_import_prefix(current.as_ref(), &clause.model_name, &qual);
            let base_name = Self::qualify_in_scope(&qual, &base_name);
            if base_name.ends_with("ExternalObject") {
                stack.push((parent, current, qual, ext, idx + 1));
                continue;
            }
            let mut base_model = self.loader.load_model(&base_name)?;
            for modification in &clause.modifications {
                apply_modification(Arc::make_mut(&mut base_model), modification);
            }
            let base_extends = std::mem::take(&mut Arc::make_mut(&mut base_model).extends);
            stack.push((parent, Arc::clone(&current), qual, ext, idx + 1));
            stack.push((Some(current), base_model, base_name, base_extends, 0));
        }
        Ok(())
    }

    fn expand_declarations(
        &mut self,
        model: Arc<Model>,
        prefix: &str,
        flat: &mut FlattenedModel,
        current_model_name: Option<&str>,
    ) -> Result<(), FlattenError> {
        #[derive(Clone)]
        enum Task {
            Process {
                model: Arc<Model>,
                prefix: String,
                current_model_name: Option<String>,
                /// FQ name of the top-level model being flattened (simulation class), not nested instance types.
                /// Used only for `resolve_import_prefix` library shorthands (e.g. Utilities under QS FundamentalWave).
                msl_import_context: String,
            },
            ExpandEquations {
                model: Arc<Model>,
                prefix: String,
            },
        }

        let msl_ctx = current_model_name.unwrap_or("").to_string();
        let mut stack: Vec<Task> = vec![Task::Process {
            model,
            prefix: prefix.to_string(),
            current_model_name: current_model_name.map(|s| s.to_string()),
            msl_import_context: msl_ctx,
        }];

        while let Some(task) = stack.pop() {
            match task {
                Task::ExpandEquations { model, prefix } => {
                    self.expand_equations(model.as_ref(), &prefix, flat);
                }
                Task::Process {
                    model,
                    prefix,
                    current_model_name,
                    msl_import_context,
                } => {
                    let current_qualified = current_model_name.as_deref().unwrap_or("");

                    // Build context from parameters in this model
                    let mut context: HashMap<String, Expression> = HashMap::new();
                    let mut local_array_sizes: HashMap<String, usize> = HashMap::new();
                    for decl in &model.declarations {
                        if decl.is_parameter {
                            if let Some(val) = &decl.start_value {
                                context.insert(decl.name.clone(), val.clone());
                            }
                        }
                    }

                    for decl in &model.declarations {
                        if let Some(ref cond_expr) = decl.condition {
                            let cond_sub = self.substitute(cond_expr, &context);
                            if let Some(v) = eval_const_expr(&cond_sub) {
                                if v == 0.0 {
                                    continue;
                                }
                            }
                        }

                        // Evaluate array size
                        let array_len = if let Some(size_expr) = &decl.array_size {
                            let sub_expr = self.substitute(size_expr, &context);
                            if let Some(val) = eval_const_expr(&sub_expr) {
                                Some(val as usize)
                            } else if let Some(val) =
                                eval_const_expr_with_array_sizes(&sub_expr, &local_array_sizes)
                            {
                                Some(val as usize)
                            } else {
                                eprintln!("Warning: Could not evaluate array size for '{}'", decl.name);
                                None
                            }
                        } else {
                            None
                        };

                        let count = array_len.unwrap_or(1);
                        let is_array = array_len.is_some();

                        let base_name = if prefix.is_empty() {
                            decl.name.clone()
                        } else {
                            format!("{}_{}", prefix, decl.name)
                        };

                        if is_array {
                            flat.array_sizes.insert(base_name.clone(), count);
                            local_array_sizes.insert(decl.name.clone(), count);
                            if !decl.is_parameter || decl.start_value.is_none() {
                                context.insert(
                                    decl.name.clone(),
                                    Expression::ArrayLiteral(vec![Expression::Number(0.0); count]),
                                );
                            }
                        }

                        for i in 1..=count {
                            let name_suffix = if is_array {
                                format!("_{}", i)
                            } else {
                                "".to_string()
                            };
                            let local_name = format!("{}{}", decl.name, name_suffix);
                            let full_path = if prefix.is_empty() {
                                local_name.clone()
                            } else {
                                format!("{}_{}", prefix, local_name)
                            };

                            let loc = current_model_name
                                .as_deref()
                                .and_then(|n| self.loader.get_path_for_model(n))
                                .map(|p| SourceLocation {
                                    file: p.display().to_string(),
                                    line: 0,
                                    column: 0,
                                });

                            let mut resolved_type =
                                resolve_type_alias(&model.type_aliases, &decl.type_name);
                            // Use the simulation class FQ name for MSL shorthands (Sensors.*, Sources.*, ...),
                            // not `current_model_name` on nested tasks (which is the child component type).
                            // Exception: inside Modelica.Clocked.* library classes, `Interfaces.*` and other
                            // local prefixes must resolve against the package of the model being flattened
                            // (e.g. Sampler.ShiftSample -> ClockSignals.Interfaces), not the top-level example.
                            let import_scope = if msl_import_context.is_empty() {
                                current_qualified
                            } else if current_qualified.starts_with("Modelica.Clocked.ClockSignals")
                                || current_qualified.starts_with("Modelica.Clocked.RealSignals")
                                || current_qualified.starts_with("Modelica.Clocked.BooleanSignals")
                                || current_qualified.starts_with("Modelica.Clocked.IntegerSignals")
                                || current_qualified.starts_with("Modelica.Electrical.Analog")
                                || current_qualified.starts_with("Modelica.Electrical.Machines")
                                || current_qualified.starts_with("Modelica.Electrical.Polyphase")
                                || current_qualified.starts_with("Modelica.Thermal.HeatTransfer")
                                || current_qualified.starts_with("Modelica.Magnetic.FundamentalWave")
                                || current_qualified.starts_with("Modelica.Magnetic.FluxTubes")
                                || current_qualified.starts_with("Modelica.Blocks")
                                || current_qualified.starts_with("Modelica.Electrical.Batteries")
                                || current_qualified.starts_with("Modelica.Mechanics")
                                || current_qualified.starts_with("Modelica.Electrical.QuasiStatic")
                                || current_qualified.starts_with("Modelica.Fluid")
                                || current_qualified.starts_with("ModelicaTest.Fluid")
                            {
                                current_qualified
                            } else {
                                msl_import_context.as_str()
                            };
                            resolved_type = Self::resolve_import_prefix(
                                model.as_ref(),
                                &resolved_type,
                                import_scope,
                            );
                            resolved_type = Self::qs_fw_utilities_redirect(
                                &resolved_type,
                                &msl_import_context,
                            );
                            if resolved_type.eq_ignore_ascii_case("real") {
                                resolved_type = "Real".to_string();
                            }
                            if resolved_type == "Modelica.Fluid.Pipes.BaseClasses.PartialValve" {
                                resolved_type =
                                    "Modelica.Fluid.Valves.BaseClasses.PartialValve".to_string();
                            }
                            let medium_alias_prefix = resolved_type
                                .split_once('.')
                                .map(|(prefix, _)| prefix)
                                .filter(|p| {
                                    *p == "Medium"
                                        || p.strip_prefix("Medium_")
                                            .and_then(|s| s.parse::<u32>().ok())
                                            .is_some()
                                });
                            if resolved_type.starts_with("Medium.") || medium_alias_prefix.is_some() {
                                resolved_type = "Real".to_string();
                            }
                            if matches!(
                                resolved_type.as_str(),
                                "RealInput"
                                    | "RealOutput"
                                    | "BooleanInput"
                                    | "BooleanOutput"
                                    | "IntegerInput"
                                    | "IntegerOutput"
                            ) || resolved_type.ends_with(".RealInput")
                                || resolved_type.ends_with(".RealOutput")
                            {
                                resolved_type = "Real".to_string();
                            } else if resolved_type.ends_with(".BooleanInput")
                                || resolved_type.ends_with(".BooleanOutput")
                            {
                                resolved_type = "Boolean".to_string();
                            } else if resolved_type.ends_with(".IntegerInput")
                                || resolved_type.ends_with(".IntegerOutput")
                            {
                                resolved_type = "Integer".to_string();
                            }
                            if resolved_type.starts_with("Modelica.Fluid.Types.") {
                                resolved_type = "Real".to_string();
                            }
                            if resolved_type.ends_with(".Types.AxisLabel")
                                || resolved_type.ends_with(".Types.Axis")
                            {
                                resolved_type = "Real".to_string();
                            }

                            if is_primitive(&resolved_type) {
                                flat.declarations.push(Declaration {
                                    type_name: resolved_type.clone(),
                                    name: full_path.clone(),
                                    replaceable: decl.replaceable,
                                    is_parameter: decl.is_parameter,
                                    is_flow: decl.is_flow,
                                    is_discrete: decl.is_discrete,
                                    is_input: decl.is_input,
                                    is_output: decl.is_output,
                                    start_value: if let Some(val) = &decl.start_value {
                                        let sub = self.substitute(val, &context);
                                        if is_array {
                                            Some(index_expression(&sub, i))
                                        } else {
                                            Some(sub)
                                        }
                                    } else {
                                        None
                                    },
                                    array_size: None,
                                    modifications: Vec::new(),
                                    is_rest: decl.is_rest,
                                    annotation: None,
                                    condition: None,
                                });
                                continue;
                            }

                            // Load complex type. For short names, try current class inner classes
                            // first, then the parent package scope.
                            let mut load_candidates = vec![resolved_type.clone()];
                            if !resolved_type.contains('.') && !resolved_type.contains('/') {
                                let same_class =
                                    Self::qualify_in_current_class(current_qualified, &resolved_type);
                                if same_class != resolved_type {
                                    load_candidates.push(same_class);
                                }
                                let parent_scope =
                                    Self::qualify_in_scope(current_qualified, &resolved_type);
                                if parent_scope != resolved_type
                                    && !load_candidates.iter().any(|c| c == &parent_scope)
                                {
                                    load_candidates.push(parent_scope);
                                }
                            }

                            let mut loaded_type: Option<(String, Arc<Model>)> = None;
                            if !resolved_type.contains('.') && !resolved_type.contains('/') {
                                if let Some(inner) =
                                    model.inner_classes.iter().find(|m| m.name == resolved_type)
                                {
                                    let mut inner_model = inner.clone();
                                    for (a, q) in &model.imports {
                                        if !inner_model
                                            .imports
                                            .iter()
                                            .any(|(aa, qq)| aa == a && qq == q)
                                        {
                                            inner_model.imports.push((a.clone(), q.clone()));
                                        }
                                    }
                                    loaded_type = Some((
                                        Self::qualify_in_current_class(
                                            current_qualified,
                                            &resolved_type,
                                        ),
                                        Arc::new(inner_model),
                                    ));
                                }
                            }
                            let mut last_err: Option<LoadError> = None;
                            if loaded_type.is_none() {
                                for candidate in &load_candidates {
                                    match self.loader.load_model_silent(candidate, true) {
                                        Ok(m) => {
                                            loaded_type = Some((candidate.clone(), m));
                                            break;
                                        }
                                        Err(e) => last_err = Some(e),
                                    }
                                }
                            }

                            let mut sub_model = match loaded_type {
                                Some((resolved_candidate, m)) => {
                                    resolved_type = resolved_candidate;
                                    m
                                }
                                None => {
                                    let e = last_err
                                        .unwrap_or_else(|| LoadError::NotFound(resolved_type.clone()));
                                    // MSL: qualified type alias (e.g. Modelica.Units.SI.Time) is a `type`
                                    // inside the package, not a loadable class.
                                    if matches!(&e, LoadError::NotFound(_)) {
                                            if let Some((prefix_type, suffix_type)) =
                                                resolved_type.rsplit_once('.')
                                            {
                                                if let Ok(owner) = self.loader.load_model(prefix_type) {
                                                    if let Some((_, base)) = owner
                                                        .type_aliases
                                                        .iter()
                                                        .find(|(a, _)| a == suffix_type)
                                                    {
                                                        let base =
                                                            resolve_type_alias(&owner.type_aliases, base);
                                                        if is_primitive(&base) {
                                                            flat.declarations.push(Declaration {
                                                                type_name: base,
                                                                name: full_path.clone(),
                                                                replaceable: decl.replaceable,
                                                                is_parameter: decl.is_parameter,
                                                                is_flow: decl.is_flow,
                                                                is_discrete: decl.is_discrete,
                                                                is_input: decl.is_input,
                                                                is_output: decl.is_output,
                                                                start_value: if let Some(val) =
                                                                    &decl.start_value
                                                                {
                                                                    let sub =
                                                                        self.substitute(val, &context);
                                                                    if is_array {
                                                                        Some(index_expression(&sub, i))
                                                                    } else {
                                                                        Some(sub)
                                                                    }
                                                                } else {
                                                                    None
                                                                },
                                                                array_size: None,
                                                                modifications: Vec::new(),
                                                                is_rest: decl.is_rest,
                                                                annotation: None,
                                                                condition: None,
                                                            });
                                                            continue;
                                                        }
                                                    }
                                                }
                                            }
                                            // MSL: unqualified name brought into scope by `import Some.Package;`
                                            if !resolved_type.contains('.') {
                                                for (_alias, qual) in &model.imports {
                                                    if qual.is_empty() {
                                                        continue;
                                                    }
                                                    let candidate = format!("{}.{}", qual, resolved_type);
                                                    if let Some((prefix_type, suffix_type)) =
                                                        candidate.rsplit_once('.')
                                                    {
                                                        if let Ok(owner) =
                                                            self.loader.load_model(prefix_type)
                                                        {
                                                            if let Some((_, base)) = owner
                                                                .type_aliases
                                                                .iter()
                                                                .find(|(a, _)| a == suffix_type)
                                                            {
                                                                let base = resolve_type_alias(
                                                                    &owner.type_aliases,
                                                                    base,
                                                                );
                                                                if is_primitive(&base) {
                                                                    flat.declarations.push(
                                                                        Declaration {
                                                                            type_name: base,
                                                                            name: full_path.clone(),
                                                                            replaceable: decl.replaceable,
                                                                            is_parameter: decl.is_parameter,
                                                                            is_flow: decl.is_flow,
                                                                            is_discrete: decl.is_discrete,
                                                                            is_input: decl.is_input,
                                                                            is_output: decl.is_output,
                                                                            start_value: if let Some(
                                                                                val,
                                                                            ) =
                                                                                &decl.start_value
                                                                            {
                                                                                let sub = self
                                                                                    .substitute(val, &context);
                                                                                if is_array {
                                                                                    Some(index_expression(
                                                                                        &sub, i,
                                                                                    ))
                                                                                } else {
                                                                                    Some(sub)
                                                                                }
                                                                            } else {
                                                                                None
                                                                            },
                                                                            array_size: None,
                                                                            modifications: Vec::new(),
                                                                            is_rest: decl.is_rest,
                                                                            annotation: None,
                                                                            condition: None,
                                                                        },
                                                                    );
                                                                    continue;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        return match e {
                                            LoadError::NotFound(_) => Err(FlattenError::UnknownType(
                                                resolved_type.clone(),
                                                full_path.clone(),
                                                loc,
                                            )),
                                            _ => Err(FlattenError::Load(e)),
                                        };
                                }
                            };

                            // Standalone short type definitions (Types/*.mo)
                            if let Some((_, base)) = sub_model
                                .type_aliases
                                .iter()
                                .find(|(a, _)| a == &sub_model.name)
                            {
                                let base = resolve_type_alias(&sub_model.type_aliases, base);
                                if is_primitive(&base) {
                                    flat.declarations.push(Declaration {
                                        type_name: base,
                                        name: full_path.clone(),
                                        replaceable: decl.replaceable,
                                        is_parameter: decl.is_parameter,
                                        is_flow: decl.is_flow,
                                        is_discrete: decl.is_discrete,
                                        is_input: decl.is_input,
                                        is_output: decl.is_output,
                                        start_value: if let Some(val) = &decl.start_value {
                                            let sub = self.substitute(val, &context);
                                            if is_array {
                                                Some(index_expression(&sub, i))
                                            } else {
                                                Some(sub)
                                            }
                                        } else {
                                            None
                                        },
                                        array_size: None,
                                        modifications: Vec::new(),
                                        is_rest: decl.is_rest,
                                        annotation: None,
                                        condition: None,
                                    });
                                    continue;
                                }
                            }

                            self.flatten_inheritance(&mut sub_model, &resolved_type)?;
                            for modification in &decl.modifications {
                                apply_modification(Arc::make_mut(&mut sub_model), modification);
                            }

                            flat.instances.insert(full_path.clone(), resolved_type.clone());

                            // Post-order: declarations first, then equations.
                            stack.push(Task::ExpandEquations {
                                model: Arc::clone(&sub_model),
                                prefix: full_path.clone(),
                            });
                            stack.push(Task::Process {
                                model: sub_model,
                                prefix: full_path,
                                current_model_name: Some(resolved_type),
                                msl_import_context: msl_import_context.clone(),
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn expand_equations(&mut self, model: &Model, prefix: &str, flat: &mut FlattenedModel) {
        let mut context: HashMap<String, Expression> = HashMap::new();
        for decl in &model.declarations {
            if decl.is_parameter {
                if let Some(val) = &decl.start_value {
                    context.insert(decl.name.clone(), val.clone());
                }
            }
        }
        let mut context_stack = vec![context];
        let instances = flat.instances.clone();
        let mut target = ExpandTarget {
            equations: &mut flat.equations,
            algorithms: &mut flat.algorithms,
            connections: &mut flat.connections,
            conditional_connections: &mut flat.conditional_connections,
            array_sizes: &flat.array_sizes,
        };
        self.expand_equation_list(
            &model.equations,
            prefix,
            &mut target,
            &mut context_stack,
            &instances,
            None,
        );
    }

    fn expand_initial_equations(&mut self, model: &Model, prefix: &str, flat: &mut FlattenedModel) {
        let mut context: HashMap<String, Expression> = HashMap::new();
        for decl in &model.declarations {
            if decl.is_parameter {
                if let Some(val) = &decl.start_value {
                    context.insert(decl.name.clone(), val.clone());
                }
            }
        }
        let mut context_stack = vec![context];
        let instances = flat.instances.clone();
        let mut target = ExpandTarget {
            equations: &mut flat.initial_equations,
            algorithms: &mut flat.initial_algorithms,
            connections: &mut flat.connections,
            conditional_connections: &mut flat.conditional_connections,
            array_sizes: &flat.array_sizes,
        };
        self.expand_equation_list(
            &model.initial_equations,
            prefix,
            &mut target,
            &mut context_stack,
            &instances,
            None,
        );
    }

    fn get_record_components(&mut self, type_name: &str) -> Option<Vec<String>> {
        let short = type_name.rsplit('.').next().unwrap_or(type_name);
        if short == "Complex"
            || type_name.ends_with(".Complex")
            || type_name.ends_with("ComplexOutput")
            || type_name.ends_with("ComplexInput")
        {
            return Some(vec!["re".to_string(), "im".to_string()]);
        }
        if is_builtin_function(type_name) {
            return None;
        }
        let m = self.loader.load_model_silent(type_name, true).ok()?;
        if m.is_record {
            Some(m.declarations.iter().map(|d| d.name.clone()).collect())
        } else {
            None
        }
    }

    fn expand_algorithms(&mut self, model: &Model, prefix: &str, flat: &mut FlattenedModel) {
        let mut context: HashMap<String, Expression> = HashMap::new();
        for decl in &model.declarations {
            if decl.is_parameter {
                if let Some(val) = &decl.start_value {
                    context.insert(decl.name.clone(), val.clone());
                }
            }
        }
        let mut context_stack = vec![context];
        let mut target = ExpandTarget {
            equations: &mut flat.equations,
            algorithms: &mut flat.algorithms,
            connections: &mut flat.connections,
            conditional_connections: &mut flat.conditional_connections,
            array_sizes: &flat.array_sizes,
        };
        self.expand_algorithm_list(&model.algorithms, prefix, &mut target, &mut context_stack);
    }

    fn expand_initial_algorithms(
        &mut self,
        model: &Model,
        prefix: &str,
        flat: &mut FlattenedModel,
    ) {
        let mut context: HashMap<String, Expression> = HashMap::new();
        for decl in &model.declarations {
            if decl.is_parameter {
                if let Some(val) = &decl.start_value {
                    context.insert(decl.name.clone(), val.clone());
                }
            }
        }
        let mut context_stack = vec![context];
        let mut target = ExpandTarget {
            equations: &mut flat.initial_equations,
            algorithms: &mut flat.initial_algorithms,
            connections: &mut flat.connections,
            conditional_connections: &mut flat.conditional_connections,
            array_sizes: &flat.array_sizes,
        };
        self.expand_algorithm_list(
            &model.initial_algorithms,
            prefix,
            &mut target,
            &mut context_stack,
        );
    }

}
