//! MSL / version-difference name aliases. Centralized so flatten does not duplicate string rewrites.

/// Before cache lookup: try these targets in order. `Hard` propagates the last error if all fail;
/// `Soft` returns to normal resolution if no target loads.
pub(crate) enum EarlyCompat {
    None,
    Hard(Vec<String>),
    Soft(Vec<String>),
}

/// After file lookup and parent inner-class resolution fail.
pub(crate) enum LateCompat {
    None,
    Soft(Vec<String>),
}

pub(crate) fn early_compat(name: &str) -> EarlyCompat {
    match name {
        "Modelica.Electrical.Analog.Interfaces.Source" => {
            EarlyCompat::Hard(vec!["Modelica.Electrical.Analog.Interfaces.VoltageSource".to_string()])
        }
        "Modelica.Electrical.Analog.Interfaces.TwoPlug" => {
            EarlyCompat::Hard(vec!["Modelica.Electrical.Analog.Interfaces.TwoPin".to_string()])
        }
        "Modelica.Electrical.Analog.Interfaces.PositivePlug" => EarlyCompat::Hard(vec![
            "Modelica.Electrical.Analog.Interfaces.PositivePin".to_string(),
        ]),
        "Modelica.Electrical.Analog.Interfaces.NegativePlug" => EarlyCompat::Hard(vec![
            "Modelica.Electrical.Analog.Interfaces.NegativePin".to_string(),
        ]),
        "Modelica.Electrical.Analog.Interfaces.Plug" => {
            EarlyCompat::Hard(vec!["Modelica.Electrical.Analog.Interfaces.Pin".to_string()])
        }
        "Modelica.Electrical.Analog.Interfaces.MemoryBase" => EarlyCompat::Soft(vec![
            "Modelica.Electrical.Analog.Interfaces.TwoPin".to_string(),
        ]),
        "Modelica.Electrical.Analog.Interfaces.DigitalInput" => EarlyCompat::Soft(vec![
            "Modelica.Electrical.Analog.Interfaces.BooleanInput".to_string(),
        ]),
        _ if name == "Modelica.Magnetic.FluxTubes.Interfaces.Source" => EarlyCompat::Hard(vec![
            "Modelica.Magnetic.QuasiStatic.FluxTubes.Interfaces.Source".to_string(),
        ]),
        _ if name.starts_with("Modelica.Magnetic.FluxTubes.Interfaces.Source.") => {
            let rest = name.trim_start_matches("Modelica.Magnetic.FluxTubes.Interfaces.Source.");
            EarlyCompat::Hard(vec![format!(
                "Modelica.Magnetic.QuasiStatic.FluxTubes.Interfaces.Source.{rest}"
            )])
        }
        _ if name == "Modelica.Magnetic.FluxTubes.Interfaces.RelativeSensor"
            || name.starts_with("Modelica.Magnetic.FluxTubes.Interfaces.RelativeSensor.")
            || name == "Modelica.Magnetic.FluxTubes.Interfaces.AbsoluteSensor"
            || name.starts_with("Modelica.Magnetic.FluxTubes.Interfaces.AbsoluteSensor.") =>
        {
            let rest = name
                .strip_prefix("Modelica.Magnetic.FluxTubes.Interfaces.")
                .unwrap_or("");
            EarlyCompat::Hard(vec![format!(
                "Modelica.Magnetic.QuasiStatic.FluxTubes.Interfaces.{rest}"
            )])
        }
        _ if name == "Modelica.Mechanics.Translational.Components.PartialFrictionWithStop"
            || name.starts_with(
                "Modelica.Mechanics.Translational.Components.PartialFrictionWithStop.",
            ) =>
        {
            let rest = name.strip_prefix(
                "Modelica.Mechanics.Translational.Components.PartialFrictionWithStop",
            )
            .unwrap_or("");
            EarlyCompat::Hard(vec![format!(
                "Modelica.Mechanics.Translational.Components.MassWithStopAndFriction.PartialFrictionWithStop{rest}"
            )])
        }
        _ if name == "Modelica.Magnetic.FundamentalWave.Components.QuasiStaticAnalogElectroMagneticConverter"
            || name.starts_with(
                "Modelica.Magnetic.FundamentalWave.Components.QuasiStaticAnalogElectroMagneticConverter.",
            ) =>
        {
            let rest = name.trim_start_matches(
                "Modelica.Magnetic.FundamentalWave.Components.QuasiStaticAnalogElectroMagneticConverter",
            );
            EarlyCompat::Hard(vec![format!(
                "Modelica.Magnetic.QuasiStatic.FundamentalWave.Components.QuasiStaticAnalogElectroMagneticConverter{rest}"
            )])
        }
        _ if name == "Modelica.Fluid.Pipes.BaseClasses.QuadraticTurbulent"
            || name.starts_with("Modelica.Fluid.Pipes.BaseClasses.QuadraticTurbulent.") =>
        {
            let rest = name
                .strip_prefix("Modelica.Fluid.Pipes.BaseClasses.QuadraticTurbulent")
                .unwrap_or("");
            EarlyCompat::Hard(vec![format!(
                "Modelica.Fluid.Pipes.BaseClasses.WallFriction.QuadraticTurbulent{rest}"
            )])
        }
        _ if name == "Modelica.Fluid.Pipes.BaseClasses.WallFriction.QuadraticTurbulent.BaseModel"
            || name
                == "Modelica.Fluid.Pipes.BaseClasses.WallFriction.QuadraticTurbulent.BaseModelNonconstantCrossSectionArea" =>
        {
            EarlyCompat::Hard(vec![
                "Modelica.Fluid.Pipes.BaseClasses.WallFriction.PartialWallFriction".to_string(),
            ])
        }
        _ if name == "Modelica.Electrical.Machines.Utilities.PartialControlledDCPM" => {
            EarlyCompat::Hard(vec!["Modelica.Electrical.Machines.Examples.ControlledDCDrives.Utilities.PartialControlledDCPM".to_string()])
        }
        _ if name == "Modelica.Electrical.Machines.Utilities.LimitedPI" => EarlyCompat::Hard(vec![
            "Modelica.Electrical.Machines.Examples.ControlledDCDrives.Utilities.LimitedPI"
                .to_string(),
        ]),
        _ if name == "Magnetic"
            || (name.starts_with("Magnetic.") && !name.starts_with("Modelica.")) =>
        {
            let new_name = if name == "Magnetic" {
                "Modelica.Magnetic".to_string()
            } else {
                format!("Modelica.{name}")
            };
            EarlyCompat::Soft(vec![new_name])
        }
        _ if name == "FluidHeatFlow"
            || name.starts_with("FluidHeatFlow.")
            || name == "Interfaces.FluidHeatFlow" =>
        {
            let alt = if name == "FluidHeatFlow" || name == "Interfaces.FluidHeatFlow" {
                "Modelica.Thermal.FluidHeatFlow.Interfaces.FluidHeatFlow".to_string()
            } else {
                format!(
                    "Modelica.Thermal.FluidHeatFlow.{}",
                    name.trim_start_matches("FluidHeatFlow.")
                )
            };
            EarlyCompat::Soft(vec![alt])
        }
        _ if name == "semiLinear" => EarlyCompat::Soft(vec![
            "Modelica.Thermal.FluidHeatFlow.Utilities.semiLinear".to_string(),
            "Modelica.Utilities.Math.semiLinear".to_string(),
            "Modelica.Fluid.Utilities.semiLinear".to_string(),
        ]),
        _ if name == "Material" || name.starts_with("Material.") => {
            let alt = if name == "Material" {
                "Modelica.Magnetic.FluxTubes.Material".to_string()
            } else {
                format!(
                    "Modelica.Magnetic.FluxTubes.Material.{}",
                    name.trim_start_matches("Material.")
                )
            };
            EarlyCompat::Soft(vec![alt])
        }
        _ => {
            let clocked_short_prefixes = [
                "ClockSignals",
                "BooleanSignals",
                "RealSignals",
                "IntegerSignals",
            ];
            for short in clocked_short_prefixes {
                if name == short || name.starts_with(&format!("{short}.")) {
                    return EarlyCompat::Hard(vec![format!("Modelica.Clocked.{name}")]);
                }
            }
            EarlyCompat::None
        }
    }
}

pub(crate) fn late_compat(name: &str) -> LateCompat {
    if name == "Modelica.Blocks.SymmetricalComponents"
        || name.starts_with("Modelica.Blocks.SymmetricalComponents.")
    {
        let alt = name.replacen(
            "Modelica.Blocks.SymmetricalComponents",
            "Modelica.Electrical.QuasiStatic.Polyphase.Blocks.SymmetricalComponents",
            1,
        );
        if alt != name {
            return LateCompat::Soft(vec![alt]);
        }
    }
    if name == "Modelica.Electrical.Polyphase.Blocks.SymmetricalComponents"
        || name.starts_with("Modelica.Electrical.Polyphase.Blocks.SymmetricalComponents.")
    {
        let alt = name.replacen(
            "Modelica.Electrical.Polyphase.Blocks",
            "Modelica.Electrical.QuasiStatic.Polyphase.Blocks",
            1,
        );
        if alt != name {
            return LateCompat::Soft(vec![alt]);
        }
    }
    if name == "Modelica.Magnetic.FundamentalWave.Utilities"
        || name.starts_with("Modelica.Magnetic.FundamentalWave.Utilities.")
    {
        let alt = name.replacen(
            "Modelica.Magnetic.FundamentalWave.Utilities",
            "Modelica.Magnetic.QuasiStatic.FundamentalWave.Utilities",
            1,
        );
        if alt != name {
            return LateCompat::Soft(vec![alt]);
        }
    }
    if name == "Modelica.Magnetic.FundamentalWave.Losses"
        || name.starts_with("Modelica.Magnetic.FundamentalWave.Losses.")
    {
        let alt = name.replacen(
            "Modelica.Magnetic.FundamentalWave.Losses",
            "Modelica.Magnetic.QuasiStatic.FundamentalWave.Losses",
            1,
        );
        if alt != name {
            return LateCompat::Soft(vec![alt]);
        }
    }
    if name == "Modelica.Fluid.Pipes.BaseClasses.PartialValve" {
        return LateCompat::Soft(vec![
            "Modelica.Fluid.Valves.BaseClasses.PartialValve".to_string(),
        ]);
    }
    // Fallback when early_compat Soft chain did not run or missed (e.g. unusual entry paths).
    if name == "FluidHeatFlow"
        || name.starts_with("FluidHeatFlow.")
        || name == "Interfaces.FluidHeatFlow"
    {
        let alt = if name == "FluidHeatFlow" || name == "Interfaces.FluidHeatFlow" {
            "Modelica.Thermal.FluidHeatFlow.Interfaces.FluidHeatFlow".to_string()
        } else {
            format!(
                "Modelica.Thermal.FluidHeatFlow.{}",
                name.trim_start_matches("FluidHeatFlow.")
            )
        };
        if alt != name {
            return LateCompat::Soft(vec![alt]);
        }
    }
    if name == "semiLinear" {
        return LateCompat::Soft(vec![
            "Modelica.Thermal.FluidHeatFlow.Utilities.semiLinear".to_string(),
            "Modelica.Utilities.Math.semiLinear".to_string(),
            "Modelica.Fluid.Utilities.semiLinear".to_string(),
        ]);
    }
    if name == "Material" || name.starts_with("Material.") {
        let alt = if name == "Material" {
            "Modelica.Magnetic.FluxTubes.Material".to_string()
        } else {
            format!(
                "Modelica.Magnetic.FluxTubes.Material.{}",
                name.trim_start_matches("Material.")
            )
        };
        if alt != name {
            return LateCompat::Soft(vec![alt]);
        }
    }
    LateCompat::None
}
