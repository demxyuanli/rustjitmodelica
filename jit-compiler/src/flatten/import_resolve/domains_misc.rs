use crate::ast::Model;

use super::context::ResolveContext;
pub(super) fn resolve_mechanics_clocked_thermal_domain(
        name: &str,
        current_qualified: &str,
        ctx: &ResolveContext,
    ) -> Option<String> {
        let in_rotational = ctx.in_rotational;
        let in_translational = ctx.in_translational;
        let in_mechanics = ctx.in_mechanics;
        let in_clocked_clocksignals = ctx.in_clocked_clocksignals;
        let in_clocked_realsignals = ctx.in_clocked_realsignals;
        let in_clocked_booleansignals = ctx.in_clocked_booleansignals;
        let in_clocked_integersignals = ctx.in_clocked_integersignals;
        let in_clocked_examples = ctx.in_clocked_examples;
        let in_multibody = ctx.in_multibody;
        let in_multibody_loops = ctx.in_multibody_loops;
        let in_heattransfer = ctx.in_heattransfer;
        let in_thermal = ctx.in_thermal;
        if in_rotational {
            if name == "Flange_a" || name == "Flange_b" || name == "Support" {
                return Some(format!("Modelica.Mechanics.Rotational.Interfaces.{}", name));
            }
            if name == "Sources" {
                return Some("Modelica.Mechanics.Rotational.Sources".to_string());
            }
            if name.starts_with("Sources.") {
                return Some(format!("Modelica.Mechanics.Rotational.{}", name));
            }
            if name == "Components" {
                return Some("Modelica.Mechanics.Rotational.Components".to_string());
            }
            if name.starts_with("Components.") {
                return Some(format!("Modelica.Mechanics.Rotational.{}", name));
            }
            if name == "Sensors" {
                return Some("Modelica.Mechanics.Rotational.Sensors".to_string());
            }
            if name.starts_with("Sensors.") {
                return Some(format!("Modelica.Mechanics.Rotational.{}", name));
            }
            if name == "Interfaces" {
                return Some("Modelica.Mechanics.Rotational.Interfaces".to_string());
            }
            if name.starts_with("Interfaces.") {
                return Some(format!("Modelica.Mechanics.Rotational.{}", name));
            }
        }
        if in_translational {
            if name == "Flange_a" || name == "Flange_b" || name == "Support" {
                return Some(format!("Modelica.Mechanics.Translational.Interfaces.{}", name));
            }
            if name == "Components" {
                return Some("Modelica.Mechanics.Translational.Components".to_string());
            }
            if name.starts_with("Components.") {
                return Some(format!("Modelica.Mechanics.Translational.{}", name));
            }
            if name == "Sources" {
                return Some("Modelica.Mechanics.Translational.Sources".to_string());
            }
            if name.starts_with("Sources.") {
                return Some(format!("Modelica.Mechanics.Translational.{}", name));
            }
            if name == "Interfaces" {
                return Some("Modelica.Mechanics.Translational.Interfaces".to_string());
            }
            if name.starts_with("Interfaces.") {
                return Some(format!("Modelica.Mechanics.Translational.{}", name));
            }
        }
        if in_mechanics {
            if name == "MultiBody" {
                return Some("Modelica.Mechanics.MultiBody".to_string());
            }
            if name.starts_with("MultiBody.") {
                return Some(format!("Modelica.Mechanics.{}", name));
            }
            if name == "Rotational" {
                return Some("Modelica.Mechanics.Rotational".to_string());
            }
            if name.starts_with("Rotational.") {
                return Some(format!("Modelica.Mechanics.{}", name));
            }
            if name == "Translational" {
                return Some("Modelica.Mechanics.Translational".to_string());
            }
            if name.starts_with("Translational.") {
                return Some(format!("Modelica.Mechanics.{}", name));
            }
            if name == "HeatTransfer" {
                return Some("Modelica.Thermal.HeatTransfer".to_string());
            }
            if name.starts_with("HeatTransfer.") {
                return Some(format!("Modelica.Thermal.{}", name));
            }
        }
        if in_clocked_clocksignals && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return Some("Modelica.Clocked.ClockSignals.Interfaces".to_string());
            }
            return Some(format!("Modelica.Clocked.ClockSignals.{}", name));
        }
        if in_clocked_realsignals && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return Some("Modelica.Clocked.RealSignals.Interfaces".to_string());
            }
            return Some(format!("Modelica.Clocked.RealSignals.{}", name));
        }
        if in_clocked_booleansignals && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return Some("Modelica.Clocked.BooleanSignals.Interfaces".to_string());
            }
            return Some(format!("Modelica.Clocked.BooleanSignals.{}", name));
        }
        if in_clocked_integersignals && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return Some("Modelica.Clocked.IntegerSignals.Interfaces".to_string());
            }
            return Some(format!("Modelica.Clocked.IntegerSignals.{}", name));
        }
        if in_clocked_examples {
            if current_qualified.starts_with("Modelica.Clocked.Examples.Systems") {
                if name == "Utilities" {
                    return Some("Modelica.Clocked.Examples.Systems.Utilities".to_string());
                }
                if name.starts_with("Utilities.") {
                    return Some(format!("Modelica.Clocked.Examples.Systems.{}", name));
                }
            }
            if name == "ClockSignals" {
                return Some("Modelica.Clocked.ClockSignals".to_string());
            }
            if name.starts_with("ClockSignals.") {
                return Some(format!("Modelica.Clocked.{}", name));
            }
            if name == "BooleanSignals" {
                return Some("Modelica.Clocked.BooleanSignals".to_string());
            }
            if name.starts_with("BooleanSignals.") {
                return Some(format!("Modelica.Clocked.{}", name));
            }
            if name == "RealSignals" {
                return Some("Modelica.Clocked.RealSignals".to_string());
            }
            if name.starts_with("RealSignals.") {
                return Some(format!("Modelica.Clocked.{}", name));
            }
            if name == "IntegerSignals" {
                return Some("Modelica.Clocked.IntegerSignals".to_string());
            }
            if name.starts_with("IntegerSignals.") {
                return Some(format!("Modelica.Clocked.{}", name));
            }
        }
        if in_multibody {
            if current_qualified.starts_with("Modelica.Mechanics.MultiBody.Examples.Loops.Utilities")
                && !name.contains('.')
                && !matches!(name, "Real" | "Integer" | "Boolean" | "String")
            {
                return Some(format!(
                    "Modelica.Mechanics.MultiBody.Examples.Loops.Utilities.{}",
                    name
                ));
            }
            if current_qualified
                .starts_with("Modelica.Mechanics.MultiBody.Examples.Systems.RobotR3")
            {
                if name == "Utilities" {
                    return Some(
                        "Modelica.Mechanics.MultiBody.Examples.Systems.RobotR3.Utilities".to_string(),
                    );
                }
                if name.starts_with("Utilities.") {
                    return Some(format!(
                        "Modelica.Mechanics.MultiBody.Examples.Systems.RobotR3.{}",
                        name
                    ));
                }
            }
            if in_multibody_loops {
                if name == "Utilities" {
                    return Some("Modelica.Mechanics.MultiBody.Examples.Loops.Utilities".to_string());
                }
                if name.starts_with("Utilities.") {
                    return Some(format!("Modelica.Mechanics.MultiBody.Examples.Loops.{}", name));
                }
            }
            if name == "world" || name.starts_with("world.") {
                let rest = name.trim_start_matches("world").trim_start_matches('.');
                if rest.is_empty() {
                    return Some("Modelica.Mechanics.MultiBody.World".to_string());
                }
                return Some(format!("Modelica.Mechanics.MultiBody.World.{}", rest));
            }
            if name == "World" {
                return Some("Modelica.Mechanics.MultiBody.World".to_string());
            }
            if name == "Joints" {
                return Some("Modelica.Mechanics.MultiBody.Joints".to_string());
            }
            if name.starts_with("Joints.") {
                return Some(format!("Modelica.Mechanics.MultiBody.{}", name));
            }
        }
        if in_heattransfer {
            if name == "HeatPort_a" || name == "HeatPort_b" || name == "HeatPort" {
                return Some(format!("Modelica.Thermal.HeatTransfer.Interfaces.{}", name));
            }
            if name == "Components" || name.starts_with("Components.") {
                return Some(format!("Modelica.Thermal.HeatTransfer.{}", name));
            }
            if name == "Celsius" || name.starts_with("Celsius.") {
                return Some(format!("Modelica.Thermal.HeatTransfer.{}", name));
            }
            if name == "Interfaces" {
                return Some("Modelica.Thermal.HeatTransfer.Interfaces".to_string());
            }
            if name.starts_with("Interfaces.") {
                return Some(format!("Modelica.Thermal.HeatTransfer.{}", name));
            }
        }
        if in_thermal {
            if name == "HeatTransfer" {
                return Some("Modelica.Thermal.HeatTransfer".to_string());
            }
            if name.starts_with("HeatTransfer.") {
                return Some(format!("Modelica.Thermal.{}", name));
            }
        }
        None
    }

pub(super) fn resolve_fluid_domain(
        name: &str,
        current_qualified: &str,
        ctx: &ResolveContext,
    ) -> Option<String> {
        let in_fluid = ctx.in_fluid;
        if !in_fluid {
            return None;
        }
        if current_qualified.contains(".Examples.") {
            let parent = current_qualified
                .rsplit_once('.')
                .map(|(p, _)| p)
                .unwrap_or(current_qualified);
            if name == "Components" || name == "BaseClasses" || name == "Utilities" {
                return Some(format!("{}.{}", parent, name));
            }
        }
        if name == "Fittings" {
            return Some("Modelica.Fluid.Fittings".to_string());
        }
        if name.starts_with("Fittings.") {
            return Some(format!("Modelica.Fluid.{}", name));
        }
        if name == "System" {
            return Some("Modelica.Fluid.System".to_string());
        }
        if name.starts_with("System.") {
            return Some(format!("Modelica.Fluid.{}", name));
        }
        if name == "Interfaces" {
            return Some("Modelica.Fluid.Interfaces".to_string());
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
                return Some(format!("Modelica.StateGraph.Interfaces.{rest}"));
            }
            return Some(format!("Modelica.Fluid.{}", name));
        }
        if name == "Types" || name.starts_with("Types.") {
            return Some(format!("Modelica.Fluid.{}", name));
        }
        None
    }

pub(super) fn resolve_blocks_utilities_domain(name: &str, ctx: &ResolveContext) -> Option<String> {
        let in_blocks = ctx.in_blocks;
        let in_blocks_math = ctx.in_blocks_math;
        let in_blocks_sources = ctx.in_blocks_sources;
        let in_blocks_tables = ctx.in_blocks_tables;
        let in_utilities = ctx.in_utilities;
        if in_blocks {
            if in_blocks_math {
                if name == "MultiProduct" {
                    return Some("Modelica.Blocks.Math.MultiProduct".to_string());
                }
                if name == "Mean" {
                    return Some("Modelica.Blocks.Math.Mean".to_string());
                }
            }
            if name == "Interfaces" {
                return Some("Modelica.Blocks.Interfaces".to_string());
            }
            if name.starts_with("Interfaces.") {
                return Some(format!("Modelica.Blocks.{}", name));
            }
            if name == "Types" {
                return Some("Modelica.Blocks.Types".to_string());
            }
            if name.starts_with("Types.") {
                return Some(format!("Modelica.Blocks.{}", name));
            }
            if name == "Sources" {
                return Some("Modelica.Blocks.Sources".to_string());
            }
            if name.starts_with("Sources.") {
                return Some(format!("Modelica.Blocks.{}", name));
            }
            if name == "Math" {
                return Some("Modelica.Blocks.Math".to_string());
            }
            if name.starts_with("Math.") {
                return Some(format!("Modelica.Blocks.{}", name));
            }
            if name == "Nonlinear" || name.starts_with("Nonlinear.") {
                return Some(format!("Modelica.Blocks.{}", name));
            }
            if name == "Continuous" || name.starts_with("Continuous.") {
                return Some(format!("Modelica.Blocks.{}", name));
            }
            if name == "Logical" || name.starts_with("Logical.") {
                return Some(format!("Modelica.Blocks.{}", name));
            }
            if name == "Internal" {
                if in_blocks_tables {
                    return Some("Modelica.Blocks.Tables.Internal".to_string());
                }
                return Some("Modelica.Blocks.Types.Internal".to_string());
            }
            if name.starts_with("Internal.") {
                if in_blocks_tables {
                    return Some(format!("Modelica.Blocks.Tables.{}", name));
                }
                return Some(format!("Modelica.Blocks.Types.{}", name));
            }
            if name == "CombiTimeTable" && in_blocks_sources {
                return Some("Modelica.Blocks.Sources.CombiTimeTable".to_string());
            }
            if name.starts_with("CombiTimeTable.") && in_blocks_sources {
                return Some(format!("Modelica.Blocks.Sources.{}", name));
            }
        }
        if in_utilities {
            if name == "Internal" {
                return Some("Modelica.Utilities.Internal".to_string());
            }
            if name.starts_with("Internal.") {
                return Some(format!("Modelica.Utilities.{}", name));
            }
        }
        None
    }

pub(super) fn resolve_global_shortcuts(model: &Model, name: &str) -> Option<String> {
        if name == "Modelica.Fluid.Pipes.BaseClasses.PartialValve" {
            return Some("Modelica.Fluid.Valves.BaseClasses.PartialValve".to_string());
        }
        if name == "Modelica.Electrical.Analog.Interfaces.PositivePlug" {
            return Some("Modelica.Electrical.Polyphase.Interfaces.PositivePlug".to_string());
        }
        if name == "Modelica.Electrical.Analog.Interfaces.NegativePlug" {
            return Some("Modelica.Electrical.Polyphase.Interfaces.NegativePlug".to_string());
        }
        if name == "FluidHeatFlow" {
            return Some("Modelica.Thermal.FluidHeatFlow.Interfaces.FluidHeatFlow".to_string());
        }
        if name.starts_with("FluidHeatFlow.") {
            let rest = name.trim_start_matches("FluidHeatFlow.");
            return Some(format!("Modelica.Thermal.FluidHeatFlow.{rest}"));
        }
        if name == "Interfaces.FluidHeatFlow" {
            return Some("Modelica.Thermal.FluidHeatFlow.Interfaces.FluidHeatFlow".to_string());
        }
        for (alias, qual) in &model.imports {
            if alias.is_empty() || qual.is_empty() {
                continue;
            }
            if name == alias {
                return Some(qual.clone());
            }
            let prefix = format!("{}.", alias);
            if let Some(rest) = name.strip_prefix(&prefix) {
                return Some(format!("{}.{}", qual, rest));
            }
        }
        if name == "Electrical" || name.starts_with("Electrical.") {
            return Some(format!("Modelica.{}", name));
        }
        if name == "Magnetic" || name.starts_with("Magnetic.") {
            return Some(format!("Modelica.{}", name));
        }
        if name == "Thermal" || name.starts_with("Thermal.") {
            return Some(format!("Modelica.{}", name));
        }
        if name == "StateGraph" || name.starts_with("StateGraph.") {
            if name == "StateGraph" {
                return Some("Modelica.StateGraph".to_string());
            }
            return Some(format!("Modelica.{}", name));
        }
        if name == "FluxTubes" || name.starts_with("FluxTubes.") {
            return Some(format!("Modelica.Magnetic.{}", name));
        }
        if name == "FundamentalWave" || name.starts_with("FundamentalWave.") {
            return Some(format!("Modelica.Magnetic.{}", name));
        }
        None
    }

pub(super) fn resolve_context_prechecks(name: &str, current_qualified: &str) -> Option<String> {
        let cq = current_qualified.replace('/', ".");
        if cq.contains("Modelica.Electrical.Spice3") || cq.contains("ModelicaTest.Electrical.Spice3")
        {
            if name == "Types" {
                return Some("Modelica.Electrical.Spice3.Types".to_string());
            }
            if let Some(rest) = name.strip_prefix("Types.") {
                return Some(format!("Modelica.Electrical.Spice3.Types.{rest}"));
            }
        }
        if cq.contains("FluidHeatFlow") {
            if name == "semiLinear" {
                return Some("Modelica.Utilities.Math.semiLinear".to_string());
            }
            if name == "Interfaces.FluidHeatFlow" {
                return Some("Modelica.Thermal.FluidHeatFlow.Interfaces.FluidHeatFlow".to_string());
            }
        }
        let in_magnetic_fluxtubes = cq.contains("Modelica.Magnetic.FluxTubes")
            || cq.contains("ModelicaTest.Magnetic.FluxTubes")
            || (cq.contains("FluxTubes") && cq.contains("Magnetic"));
        if in_magnetic_fluxtubes {
            if name == "Material" {
                return Some("Modelica.Magnetic.FluxTubes.Material".to_string());
            }
            if let Some(rest) = name.strip_prefix("Material.") {
                return Some(format!("Modelica.Magnetic.FluxTubes.Material.{rest}"));
            }
        }
        if cq == "Modelica.Electrical.Digital" || cq.starts_with("Modelica.Electrical.Digital.") {
            if let Some(rest) = name.strip_prefix("D.") {
                return Some(format!("Modelica.Electrical.Digital.{rest}"));
            }
        }
        if name == "D"
            && (cq == "Modelica.Electrical.Digital" || cq.starts_with("Modelica.Electrical.Digital."))
        {
            return Some("Modelica.Electrical.Digital".to_string());
        }
        if current_qualified.starts_with("Modelica.StateGraph") {
            if name == "StateGraph" || name.starts_with("StateGraph.") {
                if name == "StateGraph" {
                    return Some("Modelica.StateGraph".to_string());
                }
                return Some(format!("Modelica.{}", name));
            }
            if name == "Interfaces" || name.starts_with("Interfaces.") {
                if name == "Interfaces" {
                    return Some("Modelica.StateGraph.Interfaces".to_string());
                }
                return Some(format!("Modelica.StateGraph.{}", name));
            }
        }
        if (current_qualified.starts_with("Modelica.Fluid")
            || current_qualified.starts_with("ModelicaTest.Fluid"))
            && (name == "FlowModel" || name.starts_with("FlowModel."))
        {
            let rest = name.trim_start_matches("FlowModel");
            let base = "Modelica.Fluid.Pipes.BaseClasses.FlowModels.DetailedPipeFlow";
            if rest.is_empty() {
                return Some(base.to_string());
            }
            return Some(format!("{base}{rest}"));
        }
        if current_qualified.starts_with("Modelica.Electrical.Polyphase")
            && (name == "Basic" || name.starts_with("Basic."))
        {
            if name == "Basic" {
                return Some("Modelica.Electrical.Polyphase.Basic".to_string());
            }
            return Some(format!("Modelica.Electrical.Polyphase.{}", name));
        }
        if current_qualified.starts_with("Modelica.Electrical.Polyphase")
            && (name == "Ideal" || name.starts_with("Ideal."))
        {
            if name == "Ideal" {
                return Some("Modelica.Electrical.Analog.Ideal".to_string());
            }
            return Some(format!("Modelica.Electrical.Analog.{}", name));
        }
        if current_qualified.starts_with("Modelica.Electrical.QuasiStatic.SinglePhase")
            && (name == "Ideal" || name.starts_with("Ideal."))
        {
            if name == "Ideal" {
                return Some("Modelica.Electrical.QuasiStatic.SinglePhase.Ideal".to_string());
            }
            return Some(format!("Modelica.Electrical.QuasiStatic.SinglePhase.{}", name));
        }
        if name == "Mechanics" {
            return Some("Modelica.Mechanics".to_string());
        }
        if name.starts_with("Mechanics.") {
            return Some(format!("Modelica.{}", name));
        }
        None
    }

pub(super) fn resolve_global_namespace_aliases(name: &str) -> Option<String> {
        if name == "SI" {
            return Some("Modelica.Units.SI".to_string());
        }
        if let Some(rest) = name.strip_prefix("SI.") {
            return Some(format!("Modelica.Units.SI.{}", rest));
        }
        if name == "Cv" {
            return Some("Modelica.Units.Conversions".to_string());
        }
        if let Some(rest) = name.strip_prefix("Cv.") {
            return Some(format!("Modelica.Units.Conversions.{}", rest));
        }
        if name == "Constants" || name.starts_with("Constants.") {
            return Some(format!("Modelica.{}", name));
        }
        if name == "StateSelect" {
            return Some("Modelica.StateSelect".to_string());
        }
        if name.starts_with("StateSelect.") {
            return Some(format!("Modelica.{}", name));
        }
        if name == "Blocks" {
            return Some("Modelica.Blocks".to_string());
        }
        if name == "Clocked" {
            return Some("Modelica.Clocked".to_string());
        }
        if name.starts_with("Clocked.") {
            return Some(format!("Modelica.{}", name));
        }
        if name == "MultiBody" {
            return Some("Modelica.Mechanics.MultiBody".to_string());
        }
        if name.starts_with("MultiBody.") {
            return Some(format!("Modelica.Mechanics.{}", name));
        }
        if name == "ComplexBlocks" {
            return Some("Modelica.ComplexBlocks".to_string());
        }
        if name.starts_with("ComplexBlocks.") {
            return Some(format!("Modelica.{}", name));
        }
        if name.starts_with("Blocks.") {
            return Some(format!("Modelica.{}", name));
        }
        if name == "RealInput"
            || name == "RealOutput"
            || name == "BooleanInput"
            || name == "BooleanOutput"
            || name == "IntegerInput"
            || name == "IntegerOutput"
        {
            return Some(format!("Modelica.Blocks.Interfaces.{}", name));
        }
        None
    }
