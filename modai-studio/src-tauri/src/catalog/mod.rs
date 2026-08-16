use crate::diagram::{CatalogEntry, CatalogParam, PortDef};

pub mod emit_golden;
pub mod gate;
mod builtin_rest;
mod mechanics_mb;
mod physical_blocks;
mod sensors;
mod signal_blocks;

pub(super) fn signal_port(id: &str, direction: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "signal".to_string(),
        direction: direction.to_string(),
        type_name: if direction == "in" {
            "RealInput".to_string()
        } else {
            "RealOutput".to_string()
        },
    }
}

pub(super) fn pin(id: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "connector".to_string(),
        direction: "bidirectional".to_string(),
        type_name: "Modelica.Electrical.Analog.Interfaces.Pin".to_string(),
    }
}

pub(super) fn flange(id: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "connector".to_string(),
        direction: "bidirectional".to_string(),
        type_name: "Modelica.Mechanics.Rotational.Interfaces.Flange_a".to_string(),
    }
}

pub(super) fn frame(id: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "connector".to_string(),
        direction: "bidirectional".to_string(),
        type_name: "Modelica.Mechanics.MultiBody.Interfaces.Frame_a".to_string(),
    }
}

pub(super) fn p(name: &str, value: &str) -> CatalogParam {
    CatalogParam::real(name, value)
}

pub(super) fn pu(name: &str, value: &str, unit: &str, desc: &str) -> CatalogParam {
    CatalogParam::real(name, value).with_unit(unit, desc)
}

pub(super) fn p_str(name: &str, value: &str) -> CatalogParam {
    CatalogParam::typed(name, value, "String")
}

pub(super) fn draw_entry(
    id: &str,
    type_suffix: &str,
    prefix: &str,
    params: Vec<CatalogParam>,
) -> CatalogEntry {
    CatalogEntry {
        id: id.to_string(),
        type_name: format!("Studio.Draw.{type_suffix}"),
        domain: "draw".into(),
        category: "Draw".into(),
        display_key: format!("catalog.{id}"),
        default_instance_prefix: prefix.into(),
        requires_library: "Studio".into(),
        default_params: params,
        ports: vec![],
        tags: vec!["draw".into(), "annotation".into()],
        plot_hints: vec![],
        fmu_asset: None,
        fmu_guid: None,
        fmu_model_identifier: None,
        icon: None,
    }
}

pub fn builtin_catalog() -> Vec<CatalogEntry> {
    let mut out = vec![
        CatalogEntry {
            id: "blocks.sources.step".into(),
            type_name: "Modelica.Blocks.Sources.Step".into(),
            domain: "signal".into(),
            category: "Sources".into(),
            display_key: "catalog.blocks.sources.step".into(),
            default_instance_prefix: "step".into(),
            requires_library: "Modelica".into(),
            default_params: vec![pu("height", "1", "1", "Step height"), p("startTime", "0")],
            ports: vec![signal_port("y", "out")],
            tags: vec!["signal".into(), "source".into()],
            plot_hints: vec!["y".into()],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
        CatalogEntry {
            id: "blocks.sources.sine".into(),
            type_name: "Modelica.Blocks.Sources.Sine".into(),
            domain: "signal".into(),
            category: "Sources".into(),
            display_key: "catalog.blocks.sources.sine".into(),
            default_instance_prefix: "sine".into(),
            requires_library: "Modelica".into(),
            default_params: vec![p("amplitude", "1"), p("f", "1"), p("startTime", "0")],
            ports: vec![signal_port("y", "out")],
            tags: vec!["signal".into(), "source".into()],
            plot_hints: vec!["y".into()],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
        CatalogEntry {
            id: "blocks.sources.constant".into(),
            type_name: "Modelica.Blocks.Sources.Constant".into(),
            domain: "signal".into(),
            category: "Sources".into(),
            display_key: "catalog.blocks.sources.constant".into(),
            default_instance_prefix: "const".into(),
            requires_library: "Modelica".into(),
            default_params: vec![p("k", "1")],
            ports: vec![signal_port("y", "out")],
            tags: vec!["signal".into(), "source".into()],
            plot_hints: vec!["y".into()],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
        CatalogEntry {
            id: "blocks.math.gain".into(),
            type_name: "Modelica.Blocks.Math.Gain".into(),
            domain: "signal".into(),
            category: "Math".into(),
            display_key: "catalog.blocks.math.gain".into(),
            default_instance_prefix: "gain".into(),
            requires_library: "Modelica".into(),
            default_params: vec![p("k", "1")],
            ports: vec![signal_port("u", "in"), signal_port("y", "out")],
            tags: vec!["signal".into(), "math".into()],
            plot_hints: vec!["y".into()],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
        CatalogEntry {
            id: "blocks.math.add".into(),
            type_name: "Modelica.Blocks.Math.Add".into(),
            domain: "signal".into(),
            category: "Math".into(),
            display_key: "catalog.blocks.math.add".into(),
            default_instance_prefix: "add".into(),
            requires_library: "Modelica".into(),
            default_params: vec![p("k1", "1"), p("k2", "1")],
            ports: vec![
                signal_port("u1", "in"),
                signal_port("u2", "in"),
                signal_port("y", "out"),
            ],
            tags: vec!["signal".into(), "math".into()],
            plot_hints: vec!["y".into()],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
        CatalogEntry {
            id: "blocks.math.feedback".into(),
            type_name: "Modelica.Blocks.Math.Feedback".into(),
            domain: "signal".into(),
            category: "Math".into(),
            display_key: "catalog.blocks.math.feedback".into(),
            default_instance_prefix: "feedback".into(),
            requires_library: "Modelica".into(),
            default_params: vec![],
            ports: vec![
                signal_port("u1", "in"),
                signal_port("u2", "in"),
                signal_port("y", "out"),
            ],
            tags: vec!["signal".into(), "math".into()],
            plot_hints: vec!["y".into()],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
        CatalogEntry {
            id: "blocks.continuous.integrator".into(),
            type_name: "Modelica.Blocks.Continuous.Integrator".into(),
            domain: "signal".into(),
            category: "Continuous".into(),
            display_key: "catalog.blocks.continuous.integrator".into(),
            default_instance_prefix: "integrator".into(),
            requires_library: "Modelica".into(),
            default_params: vec![p("k", "1")],
            ports: vec![signal_port("u", "in"), signal_port("y", "out")],
            tags: vec!["signal".into(), "continuous".into()],
            plot_hints: vec!["y".into()],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
        CatalogEntry {
            id: "blocks.continuous.firstOrder".into(),
            type_name: "Modelica.Blocks.Continuous.FirstOrder".into(),
            domain: "signal".into(),
            category: "Continuous".into(),
            display_key: "catalog.blocks.continuous.firstOrder".into(),
            default_instance_prefix: "firstOrder".into(),
            requires_library: "Modelica".into(),
            default_params: vec![p("k", "1"), p("T", "1")],
            ports: vec![signal_port("u", "in"), signal_port("y", "out")],
            tags: vec!["signal".into(), "continuous".into()],
            plot_hints: vec!["y".into()],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
        CatalogEntry {
            id: "analog.basic.ground".into(),
            type_name: "Modelica.Electrical.Analog.Basic.Ground".into(),
            domain: "electrical".into(),
            category: "Analog".into(),
            display_key: "catalog.analog.basic.ground".into(),
            default_instance_prefix: "ground".into(),
            requires_library: "Modelica".into(),
            default_params: vec![],
            ports: vec![pin("p")],
            tags: vec!["electrical".into()],
            plot_hints: vec![],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
        CatalogEntry {
            id: "analog.basic.resistor".into(),
            type_name: "Modelica.Electrical.Analog.Basic.Resistor".into(),
            domain: "electrical".into(),
            category: "Analog".into(),
            display_key: "catalog.analog.basic.resistor".into(),
            default_instance_prefix: "resistor".into(),
            requires_library: "Modelica".into(),
            default_params: vec![p("R", "1")],
            ports: vec![pin("p"), pin("n")],
            tags: vec!["electrical".into()],
            plot_hints: vec![],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
        CatalogEntry {
            id: "analog.basic.capacitor".into(),
            type_name: "Modelica.Electrical.Analog.Basic.Capacitor".into(),
            domain: "electrical".into(),
            category: "Analog".into(),
            display_key: "catalog.analog.basic.capacitor".into(),
            default_instance_prefix: "capacitor".into(),
            requires_library: "Modelica".into(),
            default_params: vec![p("C", "1")],
            ports: vec![pin("p"), pin("n")],
            tags: vec!["electrical".into()],
            plot_hints: vec![],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
        CatalogEntry {
            id: "analog.sources.constantVoltage".into(),
            type_name: "Modelica.Electrical.Analog.Sources.ConstantVoltage".into(),
            domain: "electrical".into(),
            category: "Analog".into(),
            display_key: "catalog.analog.sources.constantVoltage".into(),
            default_instance_prefix: "vsource".into(),
            requires_library: "Modelica".into(),
            default_params: vec![p("V", "1")],
            ports: vec![pin("p"), pin("n")],
            tags: vec!["electrical".into(), "source".into()],
            plot_hints: vec![],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
        CatalogEntry {
            id: "analog.basic.inductor".into(),
            type_name: "Modelica.Electrical.Analog.Basic.Inductor".into(),
            domain: "electrical".into(),
            category: "Analog".into(),
            display_key: "catalog.analog.basic.inductor".into(),
            default_instance_prefix: "inductor".into(),
            requires_library: "Modelica".into(),
            default_params: vec![pu("L", "1", "H", "Inductance")],
            ports: vec![pin("p"), pin("n")],
            tags: vec!["electrical".into()],
            plot_hints: vec!["i".into()],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
        CatalogEntry {
            id: "analog.sensors.voltageSensor".into(),
            type_name: "Modelica.Electrical.Analog.Sensors.VoltageSensor".into(),
            domain: "electrical".into(),
            category: "Sensors".into(),
            display_key: "catalog.analog.sensors.voltageSensor".into(),
            default_instance_prefix: "voltageSensor".into(),
            requires_library: "Modelica".into(),
            default_params: vec![],
            ports: vec![
                pin("p"),
                pin("n"),
                signal_port("v", "out"),
            ],
            tags: vec!["electrical".into(), "sensor".into()],
            plot_hints: vec!["v".into()],
            fmu_asset: None,
            fmu_guid: None,
            fmu_model_identifier: None,
            icon: None,
        },
    ];
    out.extend(builtin_rest::rest_catalog_entries());
    out.extend(signal_blocks::extra_signal_blocks());
    out.extend(physical_blocks::extra_physical_blocks());
    out.extend(mechanics_mb::extra_mechanics_multibody_blocks());
    out.extend(sensors::extra_sensor_blocks());
    out
}
