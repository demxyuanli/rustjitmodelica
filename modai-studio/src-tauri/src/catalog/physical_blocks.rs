//! Extra Analog / Rotational catalog entries (F70 / P439–P450).

use crate::diagram::{CatalogEntry, CatalogParam, PortDef};

fn pin(id: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "connector".to_string(),
        direction: "bidirectional".to_string(),
        type_name: "Modelica.Electrical.Analog.Interfaces.Pin".to_string(),
    }
}

fn flange(id: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "connector".to_string(),
        direction: "bidirectional".to_string(),
        type_name: "Modelica.Mechanics.Rotational.Interfaces.Flange_a".to_string(),
    }
}

fn signal_port(id: &str, direction: &str) -> PortDef {
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

fn bool_port(id: &str, direction: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "signal".to_string(),
        direction: direction.to_string(),
        type_name: if direction == "in" {
            "BooleanInput".to_string()
        } else {
            "BooleanOutput".to_string()
        },
    }
}

fn p(name: &str, value: &str) -> CatalogParam {
    CatalogParam::real(name, value)
}

fn elec(
    id: &str,
    type_name: &str,
    category: &str,
    prefix: &str,
    params: Vec<CatalogParam>,
    ports: Vec<PortDef>,
    tags: &[&str],
    plot: &[&str],
) -> CatalogEntry {
    let mut t = vec!["electrical".to_string()];
    for x in tags {
        t.push((*x).to_string());
    }
    CatalogEntry {
        id: id.to_string(),
        type_name: type_name.to_string(),
        domain: "electrical".into(),
        category: category.into(),
        display_key: format!("catalog.{id}"),
        default_instance_prefix: prefix.into(),
        requires_library: "Modelica".into(),
        default_params: params,
        ports,
        tags: t,
        plot_hints: plot.iter().map(|s| (*s).to_string()).collect(),
        fmu_asset: None,
        fmu_guid: None,
        fmu_model_identifier: None,
        icon: None,
    }
}

fn mech(
    id: &str,
    type_name: &str,
    prefix: &str,
    params: Vec<CatalogParam>,
    ports: Vec<PortDef>,
    tags: &[&str],
    plot: &[&str],
) -> CatalogEntry {
    let mut t = vec!["mechanical".to_string()];
    for x in tags {
        t.push((*x).to_string());
    }
    CatalogEntry {
        id: id.to_string(),
        type_name: type_name.to_string(),
        domain: "mechanical".into(),
        category: "Rotational".into(),
        display_key: format!("catalog.{id}"),
        default_instance_prefix: prefix.into(),
        requires_library: "Modelica".into(),
        default_params: params,
        ports,
        tags: t,
        plot_hints: plot.iter().map(|s| (*s).to_string()).collect(),
        fmu_asset: None,
        fmu_guid: None,
        fmu_model_identifier: None,
        icon: None,
    }
}

pub fn extra_physical_blocks() -> Vec<CatalogEntry> {
    vec![
        // Analog sources / ideal switches / diode
        elec(
            "analog.sources.sineVoltage",
            "Modelica.Electrical.Analog.Sources.SineVoltage",
            "Analog",
            "vsine",
            vec![p("V", "1"), p("f", "50"), p("phase", "0"), p("offset", "0"), p("startTime", "0")],
            vec![pin("p"), pin("n")],
            &["source"],
            &[],
        ),
        elec(
            "analog.sources.pulseVoltage",
            "Modelica.Electrical.Analog.Sources.PulseVoltage",
            "Analog",
            "vpulse",
            vec![
                p("V", "1"),
                p("width", "50"),
                p("period", "1"),
                p("nperiod", "-1"),
                p("offset", "0"),
                p("startTime", "0"),
            ],
            vec![pin("p"), pin("n")],
            &["source"],
            &[],
        ),
        elec(
            "analog.sources.constantCurrent",
            "Modelica.Electrical.Analog.Sources.ConstantCurrent",
            "Analog",
            "isource",
            vec![p("I", "1")],
            vec![pin("p"), pin("n")],
            &["source"],
            &[],
        ),
        elec(
            "analog.ideal.idealDiode",
            "Modelica.Electrical.Analog.Ideal.IdealDiode",
            "Analog",
            "diode",
            vec![p("Ron", "1e-5"), p("Goff", "1e-5"), p("Vknee", "0")],
            vec![pin("p"), pin("n")],
            &["ideal", "semiconductor"],
            &[],
        ),
        elec(
            "analog.ideal.idealClosingSwitch",
            "Modelica.Electrical.Analog.Ideal.IdealClosingSwitch",
            "Analog",
            "cswitch",
            vec![p("Ron", "1e-5"), p("Goff", "1e-5")],
            vec![pin("p"), pin("n"), bool_port("control", "in")],
            &["ideal", "switch"],
            &[],
        ),
        elec(
            "analog.ideal.idealOpeningSwitch",
            "Modelica.Electrical.Analog.Ideal.IdealOpeningSwitch",
            "Analog",
            "oswitch",
            vec![p("Ron", "1e-5"), p("Goff", "1e-5")],
            vec![pin("p"), pin("n"), bool_port("control", "in")],
            &["ideal", "switch"],
            &[],
        ),
        // Rotational
        mech(
            "mechanics.rotational.fixed",
            "Modelica.Mechanics.Rotational.Components.Fixed",
            "fixed",
            vec![p("phi0", "0")],
            vec![flange("flange")],
            &["boundary"],
            &[],
        ),
        mech(
            "mechanics.rotational.springDamper",
            "Modelica.Mechanics.Rotational.Components.SpringDamper",
            "springDamper",
            vec![
                p("c", "1").with_unit("N.m/rad", "Spring constant"),
                p("d", "0.1").with_unit("N.m.s/rad", "Damping"),
            ],
            vec![flange("flange_a"), flange("flange_b")],
            &[],
            &[],
        ),
        mech(
            "mechanics.rotational.idealGear",
            "Modelica.Mechanics.Rotational.Components.IdealGear",
            "gear",
            vec![p("ratio", "1")],
            vec![flange("flange_a"), flange("flange_b")],
            &["gear"],
            &[],
        ),
        mech(
            "mechanics.rotational.speed",
            "Modelica.Mechanics.Rotational.Sources.Speed",
            "speed",
            vec![],
            vec![signal_port("w_ref", "in"), flange("flange")],
            &["source"],
            &[],
        ),
        mech(
            "mechanics.rotational.angleSensor",
            "Modelica.Mechanics.Rotational.Sensors.AngleSensor",
            "angleSensor",
            vec![],
            vec![flange("flange"), signal_port("phi", "out")],
            &["sensor"],
            &["phi"],
        ),
        mech(
            "mechanics.rotational.torqueSensor",
            "Modelica.Mechanics.Rotational.Sensors.TorqueSensor",
            "torqueSensor",
            vec![],
            vec![
                flange("flange_a"),
                flange("flange_b"),
                signal_port("tau", "out"),
            ],
            &["sensor"],
            &["tau"],
        ),
    ]
}
