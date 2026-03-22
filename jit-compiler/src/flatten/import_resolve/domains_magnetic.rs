use super::context::ResolveContext;
pub(super) fn resolve_magnetic_domain(
        name: &str,
        current_qualified: &str,
        ctx: &ResolveContext,
    ) -> Option<String> {
        let in_batteries = ctx.in_batteries;
        let in_magnetic = ctx.in_magnetic;
        let in_magnetic_fundamental_wave = ctx.in_magnetic_fundamental_wave;
        let in_magnetic_fw_components = ctx.in_magnetic_fw_components;
        let in_magnetic_fluxtubes = ctx.in_magnetic_fluxtubes;
        let in_magnetic_qs_fluxtubes = ctx.in_magnetic_qs_fluxtubes;
        let in_magnetic_qs_fundamental_wave = ctx.in_magnetic_qs_fundamental_wave;
        let in_modelicatest_magnetic_fluxtubes = ctx.in_modelicatest_magnetic_fluxtubes;
        if in_batteries {
            if name == "ParameterRecords" {
                return Some("Modelica.Electrical.Batteries.ParameterRecords".to_string());
            }
            if name.starts_with("ParameterRecords.") {
                return Some(format!("Modelica.Electrical.Batteries.{}", name));
            }
            if name == "Utilities" {
                return Some("Modelica.Electrical.Batteries.Utilities".to_string());
            }
            if name.starts_with("Utilities.") {
                return Some(format!("Modelica.Electrical.Batteries.{}", name));
            }
        }
        if in_magnetic_fundamental_wave {
            if name == "FundamentalWave" {
                return Some("Modelica.Magnetic.FundamentalWave".to_string());
            }
            if name.starts_with("FundamentalWave.") {
                return Some(format!("Modelica.Magnetic.{}", name));
            }
            if name == "Interfaces" {
                return Some("Modelica.Magnetic.FundamentalWave.Interfaces".to_string());
            }
            if name.starts_with("Interfaces.") {
                return Some(format!("Modelica.Magnetic.FundamentalWave.{}", name));
            }
            if name == "Utilities" {
                return Some("Modelica.Magnetic.FundamentalWave.Utilities".to_string());
            }
            if name.starts_with("Utilities.") {
                return Some(format!("Modelica.Magnetic.FundamentalWave.{}", name));
            }
            if current_qualified.contains(".BasicMachines.") {
                if name == "Components" {
                    return Some(
                        "Modelica.Magnetic.FundamentalWave.BasicMachines.Components".to_string(),
                    );
                }
                if name.starts_with("Components.") {
                    return Some(format!("Modelica.Magnetic.FundamentalWave.BasicMachines.{}", name));
                }
            }
            if name == "Components" {
                return Some("Modelica.Magnetic.FundamentalWave.Components".to_string());
            }
            if name.starts_with("Components.") {
                return Some(format!("Modelica.Magnetic.FundamentalWave.{}", name));
            }
            if name == "Machines" {
                return Some("Modelica.Magnetic.FundamentalWave.BasicMachines".to_string());
            }
            if name.starts_with("Machines.") {
                let rest = name.trim_start_matches("Machines.");
                return Some(format!(
                    "Modelica.Magnetic.FundamentalWave.BasicMachines.{}",
                    rest
                ));
            }
        }
        if in_magnetic_qs_fundamental_wave {
            if name == "ExampleUtilities" {
                return Some(
                    "Modelica.Magnetic.QuasiStatic.FundamentalWave.Examples.ExampleUtilities"
                        .to_string(),
                );
            }
            if name.starts_with("ExampleUtilities.") {
                return Some(format!(
                    "Modelica.Magnetic.QuasiStatic.FundamentalWave.Examples.{}",
                    name
                ));
            }
            if name == "Interfaces" {
                return Some("Modelica.Magnetic.QuasiStatic.FundamentalWave.Interfaces".to_string());
            }
            if name.starts_with("Interfaces.") {
                return Some(format!(
                    "Modelica.Magnetic.QuasiStatic.FundamentalWave.{}",
                    name
                ));
            }
            if current_qualified.contains(".BasicMachines.") {
                if name == "Components" {
                    return Some(
                        "Modelica.Magnetic.QuasiStatic.FundamentalWave.BasicMachines.Components"
                            .to_string(),
                    );
                }
                if name.starts_with("Components.") {
                    return Some(format!(
                        "Modelica.Magnetic.QuasiStatic.FundamentalWave.BasicMachines.{}",
                        name
                    ));
                }
            }
            if name == "Components" {
                return Some("Modelica.Magnetic.QuasiStatic.FundamentalWave.Components".to_string());
            }
            if name.starts_with("Components.") {
                return Some(format!(
                    "Modelica.Magnetic.QuasiStatic.FundamentalWave.{}",
                    name
                ));
            }
            if name == "BaseClasses" {
                return Some("Modelica.Magnetic.QuasiStatic.FundamentalWave.BaseClasses".to_string());
            }
            if name.starts_with("BaseClasses.") {
                return Some(format!(
                    "Modelica.Magnetic.QuasiStatic.FundamentalWave.{}",
                    name
                ));
            }
            if name == "Utilities" {
                return Some("Modelica.Magnetic.QuasiStatic.FundamentalWave.Utilities".to_string());
            }
            if name.starts_with("Utilities.") {
                return Some(format!(
                    "Modelica.Magnetic.QuasiStatic.FundamentalWave.{}",
                    name
                ));
            }
            if name == "Sensors" {
                return Some("Modelica.Magnetic.QuasiStatic.FundamentalWave.Sensors".to_string());
            }
            if name.starts_with("Sensors.") {
                return Some(format!(
                    "Modelica.Magnetic.QuasiStatic.FundamentalWave.{}",
                    name
                ));
            }
            if name == "Machines" {
                return Some(
                    "Modelica.Magnetic.QuasiStatic.FundamentalWave.BasicMachines".to_string(),
                );
            }
            if name.starts_with("Machines.") {
                let rest = name.trim_start_matches("Machines.");
                return Some(format!(
                    "Modelica.Magnetic.QuasiStatic.FundamentalWave.BasicMachines.{}",
                    rest
                ));
            }
        }
        if in_magnetic_fluxtubes || in_modelicatest_magnetic_fluxtubes {
            if name == "Material" {
                return Some("Modelica.Magnetic.FluxTubes.Material".to_string());
            }
            if name.starts_with("Material.") {
                return Some(format!("Modelica.Magnetic.FluxTubes.{}", name));
            }
            if name == "BaseClasses" {
                return Some("Modelica.Magnetic.FluxTubes.BaseClasses".to_string());
            }
            if name.starts_with("BaseClasses.") {
                return Some(format!("Modelica.Magnetic.FluxTubes.{}", name));
            }
            if name == "Interfaces" {
                return Some("Modelica.Magnetic.FluxTubes.Interfaces".to_string());
            }
            if name.starts_with("Interfaces.") {
                return Some(format!("Modelica.Magnetic.FluxTubes.{}", name));
            }
            if name == "Basic" {
                return Some("Modelica.Magnetic.FluxTubes.Basic".to_string());
            }
            if name.starts_with("Basic.") {
                return Some(format!("Modelica.Magnetic.FluxTubes.{}", name));
            }
            if name == "Sources" {
                return Some("Modelica.Magnetic.FluxTubes.Sources".to_string());
            }
            if name.starts_with("Sources.") {
                return Some(format!("Modelica.Magnetic.FluxTubes.{}", name));
            }
            if name == "Sensors" {
                return Some("Modelica.Magnetic.FluxTubes.Sensors".to_string());
            }
            if name.starts_with("Sensors.") {
                return Some(format!("Modelica.Magnetic.FluxTubes.{}", name));
            }
            if name == "Shapes" {
                return Some("Modelica.Magnetic.FluxTubes.Shapes".to_string());
            }
            if name.starts_with("Shapes.") {
                return Some(format!("Modelica.Magnetic.FluxTubes.{}", name));
            }
        }
        if in_magnetic_qs_fluxtubes {
            if name == "Interfaces" {
                return Some("Modelica.Magnetic.QuasiStatic.FluxTubes.Interfaces".to_string());
            }
            if name.starts_with("Interfaces.") {
                return Some(format!("Modelica.Magnetic.QuasiStatic.FluxTubes.{}", name));
            }
            if name == "Basic" {
                return Some("Modelica.Magnetic.QuasiStatic.FluxTubes.Basic".to_string());
            }
            if name.starts_with("Basic.") {
                return Some(format!("Modelica.Magnetic.QuasiStatic.FluxTubes.{}", name));
            }
            if name == "Sources" {
                return Some("Modelica.Magnetic.QuasiStatic.FluxTubes.Sources".to_string());
            }
            if name.starts_with("Sources.") {
                return Some(format!("Modelica.Magnetic.QuasiStatic.FluxTubes.{}", name));
            }
            if name == "Sensors" {
                return Some("Modelica.Magnetic.QuasiStatic.FluxTubes.Sensors".to_string());
            }
            if name.starts_with("Sensors.") {
                return Some(format!("Modelica.Magnetic.QuasiStatic.FluxTubes.{}", name));
            }
            if name == "Shapes" {
                return Some("Modelica.Magnetic.QuasiStatic.FluxTubes.Shapes".to_string());
            }
            if name.starts_with("Shapes.") {
                return Some(format!("Modelica.Magnetic.QuasiStatic.FluxTubes.{}", name));
            }
        }
        if in_magnetic_fw_components && (name == "Interfaces" || name.starts_with("Interfaces.")) {
            if name == "Interfaces" {
                return Some("Modelica.Magnetic.FundamentalWave.Interfaces".to_string());
            }
            return Some(format!("Modelica.Magnetic.FundamentalWave.{}", name));
        }
        if in_magnetic && current_qualified.contains(".Examples.") {
            let parent = current_qualified
                .rsplit_once('.')
                .map(|(p, _)| p)
                .unwrap_or(current_qualified);
            if name == "Components" || name == "BaseClasses" || name == "Utilities" {
                return Some(format!("{}.{}", parent, name));
            }
            for pkg in &["Components", "BaseClasses", "Utilities"] {
                if let Some(rest) = name.strip_prefix(&format!("{}.", pkg)) {
                    let base = if parent.ends_with(&format!(".{}", pkg)) {
                        parent.to_string()
                    } else {
                        format!("{}.{}", parent, pkg)
                    };
                    return Some(format!("{}.{}", base, rest));
                }
            }
        }
        None
    }
