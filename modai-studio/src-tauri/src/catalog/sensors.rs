//! F71 physical sensors: electrical / rotational / translational probes (physical -> Real).

use crate::diagram::{CatalogEntry, CatalogParam, PortDef};

fn pin(id: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "connector".to_string(),
        direction: "bidirectional".to_string(),
        type_name: "Modelica.Electrical.Analog.Interfaces.Pin".to_string(),
    }
}

fn rflange(id: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "connector".to_string(),
        direction: "bidirectional".to_string(),
        type_name: "Modelica.Mechanics.Rotational.Interfaces.Flange_a".to_string(),
    }
}

fn tflange(id: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "connector".to_string(),
        direction: "bidirectional".to_string(),
        type_name: "Modelica.Mechanics.Translational.Interfaces.Flange_a".to_string(),
    }
}

fn signal_out(id: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "signal".to_string(),
        direction: "out".to_string(),
        type_name: "RealOutput".to_string(),
    }
}

fn entry(
    id: &str,
    type_name: &str,
    domain: &str,
    category: &str,
    prefix: &str,
    ports: Vec<PortDef>,
    tags: &[&str],
    plot: &[&str],
) -> CatalogEntry {
    let mut t = vec!["sensor".to_string(), domain.to_string()];
    for x in tags {
        t.push((*x).to_string());
    }
    CatalogEntry {
        id: id.to_string(),
        type_name: type_name.to_string(),
        domain: domain.into(),
        category: category.into(),
        display_key: format!("catalog.{id}"),
        default_instance_prefix: prefix.into(),
        requires_library: "Modelica".into(),
        default_params: Vec::<CatalogParam>::new(),
        ports,
        tags: t,
        plot_hints: plot.iter().map(|s| (*s).to_string()).collect(),
        fmu_asset: None,
        fmu_guid: None,
        fmu_model_identifier: None,
        icon: None,
    }
}

/// Additional sensor catalog entries (Q4-B / F71).
/// Convention: physical flanges/pins + RealOutput signal ports (no separate converter block).
pub fn extra_sensor_blocks() -> Vec<CatalogEntry> {
    vec![
        // --- Electrical ---
        entry(
            "analog.sensors.potentialSensor",
            "Modelica.Electrical.Analog.Sensors.PotentialSensor",
            "electrical",
            "Sensors",
            "potSensor",
            vec![pin("p"), signal_out("phi")],
            &[],
            &["phi"],
        ),
        entry(
            "analog.sensors.powerSensor",
            "Modelica.Electrical.Analog.Sensors.PowerSensor",
            "electrical",
            "Sensors",
            "pwrSensor",
            vec![
                pin("pc"),
                pin("nc"),
                pin("pv"),
                pin("nv"),
                signal_out("power"),
            ],
            &[],
            &["power"],
        ),
        // --- Rotational ---
        entry(
            "mechanics.rotational.speedSensor",
            "Modelica.Mechanics.Rotational.Sensors.SpeedSensor",
            "mechanical",
            "Sensors",
            "wSensor",
            vec![rflange("flange"), signal_out("w")],
            &["rotational"],
            &["w"],
        ),
        entry(
            "mechanics.rotational.accSensor",
            "Modelica.Mechanics.Rotational.Sensors.AccSensor",
            "mechanical",
            "Sensors",
            "accSensor",
            vec![rflange("flange"), signal_out("a")],
            &["rotational"],
            &["a"],
        ),
        entry(
            "mechanics.rotational.relAngleSensor",
            "Modelica.Mechanics.Rotational.Sensors.RelAngleSensor",
            "mechanical",
            "Sensors",
            "relAngle",
            vec![
                rflange("flange_a"),
                rflange("flange_b"),
                signal_out("phi_rel"),
            ],
            &["rotational"],
            &["phi_rel"],
        ),
        entry(
            "mechanics.rotational.relSpeedSensor",
            "Modelica.Mechanics.Rotational.Sensors.RelSpeedSensor",
            "mechanical",
            "Sensors",
            "relSpeed",
            vec![
                rflange("flange_a"),
                rflange("flange_b"),
                signal_out("w_rel"),
            ],
            &["rotational"],
            &["w_rel"],
        ),
        // --- Translational ---
        entry(
            "mechanics.translational.speedSensor",
            "Modelica.Mechanics.Translational.Sensors.SpeedSensor",
            "mechanical",
            "Sensors",
            "vSensor",
            vec![tflange("flange"), signal_out("v")],
            &["translational"],
            &["v"],
        ),
        entry(
            "mechanics.translational.accSensor",
            "Modelica.Mechanics.Translational.Sensors.AccSensor",
            "mechanical",
            "Sensors",
            "tAccSensor",
            vec![tflange("flange"), signal_out("a")],
            &["translational"],
            &["a"],
        ),
        entry(
            "mechanics.translational.relPositionSensor",
            "Modelica.Mechanics.Translational.Sensors.RelPositionSensor",
            "mechanical",
            "Sensors",
            "relPos",
            vec![
                tflange("flange_a"),
                tflange("flange_b"),
                signal_out("s_rel"),
            ],
            &["translational"],
            &["s_rel"],
        ),
        entry(
            "mechanics.translational.relSpeedSensor",
            "Modelica.Mechanics.Translational.Sensors.RelSpeedSensor",
            "mechanical",
            "Sensors",
            "relVel",
            vec![
                tflange("flange_a"),
                tflange("flange_b"),
                signal_out("v_rel"),
            ],
            &["translational"],
            &["v_rel"],
        ),
        entry(
            "mechanics.translational.relForceSensor",
            "Modelica.Mechanics.Translational.Sensors.RelForceSensor",
            "mechanical",
            "Sensors",
            "relForce",
            vec![
                tflange("flange_a"),
                tflange("flange_b"),
                signal_out("f_rel"),
            ],
            &["translational"],
            &["f_rel"],
        ),
    ]
}
