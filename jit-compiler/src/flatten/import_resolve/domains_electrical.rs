use super::context::{resolve_named_subpackages, ResolveContext};
pub(super) fn resolve_electrical_domain(
        name: &str,
        current_qualified: &str,
        ctx: &ResolveContext,
    ) -> Option<String> {
        if ctx.in_electrical_analog {
            if name == "PositivePin" || name == "NegativePin" || name == "Pin" {
                return Some(format!("Modelica.Electrical.Analog.Interfaces.{}", name));
            }
            if let Some(resolved) = resolve_named_subpackages(
                name,
                "Modelica.Electrical.Analog",
                &["Sources", "Basic", "Semiconductors", "Ideal", "Sensors", "Interfaces"],
            ) {
                return Some(resolved);
            }
        }
        let in_electrical = ctx.in_electrical;
        let in_polyphase = ctx.in_polyphase;
        let in_qs_polyphase_basic = ctx.in_qs_polyphase_basic;
        let in_qs_single_phase = ctx.in_qs_single_phase;
        let in_machines = ctx.in_machines;
        let in_powerconverters = ctx.in_powerconverters;
        if current_qualified.starts_with("Modelica.Electrical.Analog.Examples.OpAmps") {
            if name == "OpAmpCircuits" {
                return Some("Modelica.Electrical.Analog.Examples.OpAmps.OpAmpCircuits".to_string());
            }
            if name.starts_with("OpAmpCircuits.") {
                return Some(format!("Modelica.Electrical.Analog.Examples.OpAmps.{}", name));
            }
            if name == "OpAmps" {
                return Some("Modelica.Electrical.Analog.Examples.OpAmps".to_string());
            }
            if name.starts_with("OpAmps.") {
                return Some(format!("Modelica.Electrical.Analog.Examples.{}", name));
            }
        }
        if in_polyphase && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return Some("Modelica.Electrical.Polyphase.Interfaces".to_string());
            }
            return Some(format!("Modelica.Electrical.Polyphase.{}", name));
        }
        if in_polyphase {
            if name == "Basic" {
                return Some("Modelica.Electrical.Polyphase.Basic".to_string());
            }
            if name.starts_with("Basic.") {
                return Some(format!("Modelica.Electrical.Polyphase.{}", name));
            }
            if name == "Sources" {
                return Some("Modelica.Electrical.Polyphase.Sources".to_string());
            }
            if name.starts_with("Sources.") {
                return Some(format!("Modelica.Electrical.Polyphase.{}", name));
            }
            if name == "Sensors" {
                return Some("Modelica.Electrical.Polyphase.Sensors".to_string());
            }
            if name.starts_with("Sensors.") {
                return Some(format!("Modelica.Electrical.Polyphase.{}", name));
            }
        }
        if current_qualified.starts_with("Modelica.Electrical.QuasiStatic.Polyphase")
            && (name == "Interfaces" || name.starts_with("Interfaces."))
        {
            if name == "Interfaces" {
                return Some("Modelica.Electrical.QuasiStatic.Polyphase.Interfaces".to_string());
            }
            return Some(format!("Modelica.Electrical.QuasiStatic.Polyphase.{}", name));
        }
        if current_qualified.starts_with("Modelica.Electrical.QuasiStatic.Polyphase") {
            if name == "Basic" {
                return Some("Modelica.Electrical.QuasiStatic.Polyphase.Basic".to_string());
            }
            if name.starts_with("Basic.") {
                return Some(format!("Modelica.Electrical.QuasiStatic.Polyphase.{}", name));
            }
            if name == "Sources" {
                return Some("Modelica.Electrical.QuasiStatic.Polyphase.Sources".to_string());
            }
            if name.starts_with("Sources.") {
                return Some(format!("Modelica.Electrical.QuasiStatic.Polyphase.{}", name));
            }
            if name == "Sensors" {
                return Some("Modelica.Electrical.QuasiStatic.Polyphase.Sensors".to_string());
            }
            if name.starts_with("Sensors.") {
                return Some(format!("Modelica.Electrical.QuasiStatic.Polyphase.{}", name));
            }
        }
        if in_qs_single_phase {
            if name == "Basic" {
                return Some("Modelica.Electrical.QuasiStatic.SinglePhase.Basic".to_string());
            }
            if name.starts_with("Basic.") {
                return Some(format!("Modelica.Electrical.QuasiStatic.SinglePhase.{}", name));
            }
            if name == "Interfaces" {
                return Some("Modelica.Electrical.QuasiStatic.SinglePhase.Interfaces".to_string());
            }
            if name.starts_with("Interfaces.") {
                return Some(format!("Modelica.Electrical.QuasiStatic.SinglePhase.{}", name));
            }
            if name == "Sources" {
                return Some("Modelica.Electrical.QuasiStatic.SinglePhase.Sources".to_string());
            }
            if name.starts_with("Sources.") {
                return Some(format!("Modelica.Electrical.QuasiStatic.SinglePhase.{}", name));
            }
            if name == "Sensors" {
                return Some("Modelica.Electrical.QuasiStatic.SinglePhase.Sensors".to_string());
            }
            if name.starts_with("Sensors.") {
                return Some(format!("Modelica.Electrical.QuasiStatic.SinglePhase.{}", name));
            }
            if name == "Utilities" {
                return Some("Modelica.Electrical.QuasiStatic.SinglePhase.Utilities".to_string());
            }
            if name.starts_with("Utilities.") {
                return Some(format!("Modelica.Electrical.QuasiStatic.SinglePhase.{}", name));
            }
        }
        if in_electrical {
            if name == "DCAC" {
                return Some("Modelica.Electrical.PowerConverters.DCAC".to_string());
            }
            if name.starts_with("DCAC.") {
                return Some(format!("Modelica.Electrical.PowerConverters.{}", name));
            }
            if in_powerconverters
                && current_qualified.starts_with("Modelica.Electrical.PowerConverters")
                && (name == "Interfaces" || name.starts_with("Interfaces."))
            {
                if name == "Interfaces" {
                    return Some("Modelica.Electrical.PowerConverters.Interfaces".to_string());
                }
                return Some(format!("Modelica.Electrical.PowerConverters.{}", name));
            }
            if current_qualified.starts_with("Modelica.Electrical.PowerConverters.Examples") {
                let parent = current_qualified
                    .rsplit_once('.')
                    .map(|(p, _)| p)
                    .unwrap_or(current_qualified);
                if name == "ExampleTemplates" {
                    return Some(format!("{}.ExampleTemplates", parent));
                }
                if let Some(rest) = name.strip_prefix("ExampleTemplates.") {
                    return Some(format!("{}.ExampleTemplates.{}", parent, rest));
                }
            }
            if current_qualified.starts_with("Modelica.Electrical.PowerConverters") {
                if name == "Icons" {
                    return Some("Modelica.Electrical.PowerConverters.Icons".to_string());
                }
                if name.starts_with("Icons.") {
                    return Some(format!("Modelica.Electrical.PowerConverters.{}", name));
                }
            }
            if name == "ComplexBlocks" {
                return Some("Modelica.ComplexBlocks".to_string());
            }
            if name.starts_with("ComplexBlocks.") {
                return Some(format!("Modelica.{}", name));
            }
            if name == "Mechanics" {
                return Some("Modelica.Mechanics".to_string());
            }
            if name.starts_with("Mechanics.") {
                return Some(format!("Modelica.{}", name));
            }
            if name == "Analog" {
                return Some("Modelica.Electrical.Analog".to_string());
            }
            if name.starts_with("Analog.") {
                return Some(format!("Modelica.Electrical.{}", name));
            }
            if name == "Polyphase" {
                return Some("Modelica.Electrical.Polyphase".to_string());
            }
            if name.starts_with("Polyphase.") {
                return Some(format!("Modelica.Electrical.{}", name));
            }
            if name == "QuasiStatic" {
                return Some("Modelica.Electrical.QuasiStatic".to_string());
            }
            if name.starts_with("QuasiStatic.") {
                return Some(format!("Modelica.Electrical.{}", name));
            }
            if name == "PowerConverters" {
                return Some("Modelica.Electrical.PowerConverters".to_string());
            }
            if name.starts_with("PowerConverters.") {
                return Some(format!("Modelica.Electrical.{}", name));
            }
            if name == "SinglePhase" {
                return Some("Modelica.Electrical.QuasiStatic.SinglePhase".to_string());
            }
            if name.starts_with("SinglePhase.") {
                return Some(format!("Modelica.Electrical.QuasiStatic.{}", name));
            }
            if name == "PositivePin" || name == "NegativePin" || name == "Pin" {
                return Some(format!("Modelica.Electrical.Analog.Interfaces.{}", name));
            }
        }
        if in_qs_polyphase_basic
            && (name == "PlugToPins_p"
                || name == "PlugToPins_n"
                || name == "PlugToPin_p"
                || name == "PlugToPin_n")
        {
            return Some(format!("Modelica.Electrical.QuasiStatic.Polyphase.Basic.{}", name));
        }
        if in_electrical && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return Some("Modelica.Electrical.Analog.Interfaces".to_string());
            }
            return Some(format!("Modelica.Electrical.Analog.{}", name));
        }
        if in_machines {
            if name == "ControlledDCDrives" {
                return Some("Modelica.Electrical.Machines.Examples.ControlledDCDrives".to_string());
            }
            if name.starts_with("ControlledDCDrives.") {
                return Some(format!("Modelica.Electrical.Machines.Examples.{}", name));
            }
            if name == "BasicMachines" {
                return Some("Modelica.Electrical.Machines.BasicMachines".to_string());
            }
            if name.starts_with("BasicMachines.") {
                return Some(format!("Modelica.Electrical.Machines.{}", name));
            }
            if name == "Utilities" {
                return Some("Modelica.Electrical.Machines.Utilities".to_string());
            }
            if name.starts_with("Utilities.") {
                return Some(format!("Modelica.Electrical.Machines.{}", name));
            }
            if name == "SpacePhasors" {
                return Some("Modelica.Electrical.Machines.SpacePhasors".to_string());
            }
            if name.starts_with("SpacePhasors.") {
                return Some(format!("Modelica.Electrical.Machines.{}", name));
            }
            if name == "Sensors" {
                return Some("Modelica.Electrical.Machines.Sensors".to_string());
            }
            if name.starts_with("Sensors.") {
                return Some(format!("Modelica.Electrical.Machines.{}", name));
            }
            if name == "Components" {
                return Some("Modelica.Electrical.Machines.BasicMachines.Components".to_string());
            }
            if name.starts_with("Components.") {
                let rest = name.trim_start_matches("Components.");
                return Some(format!(
                    "Modelica.Electrical.Machines.BasicMachines.Components.{}",
                    rest
                ));
            }
            if name == "Machines" {
                return Some("Modelica.Electrical.Machines".to_string());
            }
            if name.starts_with("Machines.") {
                let rest = name.trim_start_matches("Machines.");
                return Some(format!("Modelica.Electrical.Machines.{}", rest));
            }
        }
        None
    }