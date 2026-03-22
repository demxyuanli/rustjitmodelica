#[derive(Debug, Clone, Copy)]
pub(super) struct ResolveContext {
    pub(super) in_blocks: bool,
    pub(super) in_blocks_math: bool,
    pub(super) in_blocks_sources: bool,
    pub(super) in_blocks_tables: bool,
    pub(super) in_electrical_analog: bool,
    pub(super) in_electrical: bool,
    pub(super) in_polyphase: bool,
    pub(super) in_machines: bool,
    pub(super) in_qs_polyphase_basic: bool,
    pub(super) in_qs_single_phase: bool,
    pub(super) in_rotational: bool,
    pub(super) in_translational: bool,
    pub(super) in_mechanics: bool,
    pub(super) in_clocked_clocksignals: bool,
    pub(super) in_clocked_realsignals: bool,
    pub(super) in_clocked_booleansignals: bool,
    pub(super) in_clocked_integersignals: bool,
    pub(super) in_clocked_examples: bool,
    pub(super) in_powerconverters: bool,
    pub(super) in_batteries: bool,
    pub(super) in_magnetic: bool,
    pub(super) in_magnetic_fundamental_wave: bool,
    pub(super) in_magnetic_fw_components: bool,
    pub(super) in_magnetic_fluxtubes: bool,
    pub(super) in_magnetic_qs_fluxtubes: bool,
    pub(super) in_magnetic_qs_fundamental_wave: bool,
    pub(super) in_modelicatest_magnetic_fluxtubes: bool,
    pub(super) in_multibody: bool,
    pub(super) in_multibody_loops: bool,
    pub(super) in_heattransfer: bool,
    pub(super) in_thermal: bool,
    pub(super) in_fluid: bool,
    pub(super) in_utilities: bool,
}

impl ResolveContext {
    pub(super) fn from_current_qualified(current_qualified: &str) -> Self {
        Self {
            in_blocks: current_qualified.starts_with("Modelica.Blocks"),
            in_blocks_math: current_qualified.starts_with("Modelica.Blocks.Math"),
            in_blocks_sources: current_qualified.starts_with("Modelica.Blocks.Sources"),
            in_blocks_tables: current_qualified.starts_with("Modelica.Blocks.Tables"),
            in_electrical_analog: current_qualified.starts_with("Modelica.Electrical.Analog"),
            in_electrical: current_qualified.starts_with("Modelica.Electrical"),
            in_polyphase: current_qualified.starts_with("Modelica.Electrical.Polyphase"),
            in_machines: current_qualified.starts_with("Modelica.Electrical.Machines"),
            in_qs_polyphase_basic: current_qualified
                .starts_with("Modelica.Electrical.QuasiStatic.Polyphase.Basic"),
            in_qs_single_phase: current_qualified
                .starts_with("Modelica.Electrical.QuasiStatic.SinglePhase"),
            in_rotational: current_qualified.starts_with("Modelica.Mechanics.Rotational"),
            in_translational: current_qualified.starts_with("Modelica.Mechanics.Translational"),
            in_mechanics: current_qualified.starts_with("Modelica.Mechanics"),
            in_clocked_clocksignals: current_qualified
                .starts_with("Modelica.Clocked.ClockSignals"),
            in_clocked_realsignals: current_qualified.starts_with("Modelica.Clocked.RealSignals"),
            in_clocked_booleansignals: current_qualified
                .starts_with("Modelica.Clocked.BooleanSignals"),
            in_clocked_integersignals: current_qualified
                .starts_with("Modelica.Clocked.IntegerSignals"),
            in_clocked_examples: current_qualified.starts_with("Modelica.Clocked.Examples"),
            in_powerconverters: current_qualified
                .starts_with("Modelica.Electrical.PowerConverters"),
            in_batteries: current_qualified.starts_with("Modelica.Electrical.Batteries"),
            in_magnetic: current_qualified.starts_with("Modelica.Magnetic"),
            in_magnetic_fundamental_wave: current_qualified
                .starts_with("Modelica.Magnetic.FundamentalWave"),
            in_magnetic_fw_components: current_qualified
                .starts_with("Modelica.Magnetic.FundamentalWave.Components"),
            in_magnetic_fluxtubes: current_qualified.starts_with("Modelica.Magnetic.FluxTubes"),
            in_magnetic_qs_fluxtubes: current_qualified
                .starts_with("Modelica.Magnetic.QuasiStatic.FluxTubes"),
            in_magnetic_qs_fundamental_wave: current_qualified
                .starts_with("Modelica.Magnetic.QuasiStatic.FundamentalWave"),
            in_modelicatest_magnetic_fluxtubes: current_qualified
                .starts_with("ModelicaTest.Magnetic.FluxTubes"),
            in_multibody: current_qualified.starts_with("Modelica.Mechanics.MultiBody"),
            in_multibody_loops: current_qualified
                .starts_with("Modelica.Mechanics.MultiBody.Examples.Loops"),
            in_heattransfer: current_qualified.starts_with("Modelica.Thermal.HeatTransfer"),
            in_thermal: current_qualified.starts_with("Modelica.Thermal"),
            in_fluid: current_qualified.starts_with("Modelica.Fluid")
                || current_qualified.starts_with("ModelicaTest.Fluid"),
            in_utilities: current_qualified.starts_with("Modelica.Utilities"),
        }
    }
}

pub(super) fn resolve_named_subpackages(
    name: &str,
    namespace: &str,
    entries: &[&str],
) -> Option<String> {
    for entry in entries {
        if name == *entry {
            return Some(format!("{namespace}.{entry}"));
        }
        if name.starts_with(&format!("{entry}.")) {
            return Some(format!("{namespace}.{}", name));
        }
    }
    None
}
