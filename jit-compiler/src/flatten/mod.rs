use crate::ast::{AlgorithmStatement, Declaration, Equation, Expression, ExtendsClause, Model};
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

use self::connections::resolve_connections;
#[allow(unused_imports)]
pub use self::expressions::{eval_const_expr, expr_to_path, index_expression, prefix_expression};
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
    fn resolve_import_prefix(model: &Model, name: &str, current_qualified: &str) -> String {
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
        let in_rotational = current_qualified.starts_with("Modelica.Mechanics.Rotational");
        let in_translational = current_qualified.starts_with("Modelica.Mechanics.Translational");
        let in_mechanics = current_qualified.starts_with("Modelica.Mechanics");
        let in_multibody = current_qualified.starts_with("Modelica.Mechanics.MultiBody");
        let in_multibody_loops = current_qualified.starts_with("Modelica.Mechanics.MultiBody.Examples.Loops");
        let in_heattransfer = current_qualified.starts_with("Modelica.Thermal.HeatTransfer");
        let in_thermal = current_qualified.starts_with("Modelica.Thermal");
        let in_fluid = current_qualified.starts_with("Modelica.Fluid");
        let in_utilities = current_qualified.starts_with("Modelica.Utilities");

        // --- Units (SI) ---
        if name == "SI" {
            return "Modelica.Units.SI".to_string();
        }
        if let Some(rest) = name.strip_prefix("SI.") {
            return format!("Modelica.Units.SI.{}", rest);
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
            if name == "Interfaces" {
                return "Modelica.Electrical.Analog.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
                return format!("Modelica.Electrical.Analog.{}", name);
            }
        }
        // --- Electrical.Polyphase: Interfaces ---
        if in_polyphase && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return "Modelica.Electrical.Polyphase.Interfaces".to_string();
            }
            return format!("Modelica.Electrical.Polyphase.{}", name);
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
        // --- Mechanics.MultiBody: World, Joints, Utilities, Frames, Interfaces, Types ---
        if in_multibody {
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
            if name == "Interfaces" {
                return "Modelica.Fluid.Interfaces".to_string();
            }
            if name.starts_with("Interfaces.") {
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

    fn qualify_in_scope(current_qualified: &str, name: &str) -> String {
        if name.contains('.') || name.contains('/') {
            return name.to_string();
        }
        if let Some((parent, _)) = current_qualified.rsplit_once('.') {
            return format!("{}.{}", parent, name);
        }
        name.to_string()
    }

    /// Iterative flatten_inheritance to avoid stack overflow on deep extends chains.
    /// Frame: (parent_arc_or_none, current_model_arc, qualified_name, extends_clauses, next_index).
    fn flatten_inheritance(
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
            },
            ExpandEquations {
                model: Arc<Model>,
                prefix: String,
            },
        }

        let mut stack: Vec<Task> = vec![Task::Process {
            model,
            prefix: prefix.to_string(),
            current_model_name: current_model_name.map(|s| s.to_string()),
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
                } => {
                    let current_qualified = current_model_name.as_deref().unwrap_or("");

                    // Build context from parameters in this model
                    let mut context: HashMap<String, Expression> = HashMap::new();
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
                            resolved_type =
                                Self::resolve_import_prefix(model.as_ref(), &resolved_type, current_qualified);
                            if resolved_type == "Modelica.Fluid.Pipes.BaseClasses.PartialValve" {
                                resolved_type =
                                    "Modelica.Fluid.Valves.BaseClasses.PartialValve".to_string();
                            }
                            if resolved_type.starts_with("Medium.") {
                                resolved_type = "Real".to_string();
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

                            // Load complex type
                            let mut sub_model =
                                match self.loader.load_model_silent(&resolved_type, true) {
                                    Ok(m) => m,
                                    Err(e) => {
                                        // MSL: qualified type alias (e.g. Modelica.Units.SI.Time) is a `type`
                                        // inside the package, not a loadable class.
                                        if let LoadError::NotFound(_) = e {
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
        let m = self.loader.load_model(type_name).ok()?;
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

    fn infer_clocked_variables(&self, flat: &mut FlattenedModel) {
        fn expr_contains_clock(e: &Expression) -> bool {
            match e {
                Expression::Sample(inner)
                | Expression::Interval(inner)
                | Expression::Hold(inner)
                | Expression::Previous(inner) => expr_contains_clock(inner),
                Expression::SubSample(c, n)
                | Expression::SuperSample(c, n)
                | Expression::ShiftSample(c, n) => expr_contains_clock(c) || expr_contains_clock(n),
                Expression::BinaryOp(l, _, r) => expr_contains_clock(l) || expr_contains_clock(r),
                Expression::Call(_, args) => args.iter().any(expr_contains_clock),
                Expression::ArrayAccess(base, idx) => {
                    expr_contains_clock(base) || expr_contains_clock(idx)
                }
                Expression::Dot(base, _) => expr_contains_clock(base),
                Expression::If(c, t, f) => {
                    expr_contains_clock(c) || expr_contains_clock(t) || expr_contains_clock(f)
                }
                Expression::Range(a, b, c) => {
                    expr_contains_clock(a) || expr_contains_clock(b) || expr_contains_clock(c)
                }
                Expression::ArrayLiteral(items) => items.iter().any(expr_contains_clock),
                _ => false,
            }
        }

        fn collect_lhs_vars(expr: &Expression, out: &mut std::collections::HashSet<String>) {
            match expr {
                Expression::Variable(name) => {
                    out.insert(name.clone());
                }
                Expression::Der(inner) => collect_lhs_vars(inner, out),
                Expression::ArrayAccess(base, _) => collect_lhs_vars(base, out),
                Expression::Dot(base, _) => collect_lhs_vars(base, out),
                Expression::ArrayLiteral(items) => {
                    for e in items {
                        collect_lhs_vars(e, out);
                    }
                }
                _ => {}
            }
        }

        fn walk_algorithms(
            stmts: &[AlgorithmStatement],
            clocked: bool,
            out: &mut std::collections::HashSet<String>,
        ) {
            for stmt in stmts {
                match stmt {
                    AlgorithmStatement::Assignment(lhs, _) => {
                        if clocked {
                            collect_lhs_vars(lhs, out);
                        }
                    }
                    AlgorithmStatement::MultiAssign(lhss, _) => {
                        if clocked {
                            for lhs in lhss {
                                collect_lhs_vars(lhs, out);
                            }
                        }
                    }
                    AlgorithmStatement::CallStmt(_) => {}
                    AlgorithmStatement::NoOp => {}
                    AlgorithmStatement::If(_, then_stmts, else_ifs, else_stmts) => {
                        walk_algorithms(then_stmts, clocked, out);
                        for (_, s) in else_ifs {
                            walk_algorithms(s, clocked, out);
                        }
                        if let Some(s) = else_stmts {
                            walk_algorithms(s, clocked, out);
                        }
                    }
                    AlgorithmStatement::While(_, body) => {
                        walk_algorithms(body, clocked, out);
                    }
                    AlgorithmStatement::For(_, _, body) => {
                        walk_algorithms(body, clocked, out);
                    }
                    AlgorithmStatement::When(cond, body, else_whens) => {
                        let is_clock = expr_contains_clock(cond);
                        let new_clocked = clocked || is_clock;
                        walk_algorithms(body, new_clocked, out);
                        for (c, s) in else_whens {
                            let else_clocked = clocked || expr_contains_clock(c);
                            walk_algorithms(s, else_clocked, out);
                        }
                    }
                    AlgorithmStatement::Reinit(var, _) => {
                        if clocked {
                            out.insert(var.clone());
                        }
                    }
                    AlgorithmStatement::Assert(_, _) | AlgorithmStatement::Terminate(_) => {}
                }
            }
        }

        let mut clocked = std::collections::HashSet::new();
        walk_algorithms(&flat.algorithms, false, &mut clocked);
        walk_algorithms(&flat.initial_algorithms, false, &mut clocked);
        flat.clocked_var_names = clocked.clone();
        if !clocked.is_empty() {
            flat.clock_partitions
                .push(self::structures::ClockPartition {
                    id: "default".to_string(),
                    var_names: clocked,
                });
        }
    }
}
