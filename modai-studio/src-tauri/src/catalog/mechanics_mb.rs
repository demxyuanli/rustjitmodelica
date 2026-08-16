//! Translational 1D + Multibody joint/part expansions (F70/F74 / P479–P498).

use crate::diagram::{CatalogEntry, CatalogParam, PortDef};

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

fn tflange(id: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "connector".to_string(),
        direction: "bidirectional".to_string(),
        type_name: "Modelica.Mechanics.Translational.Interfaces.Flange_a".to_string(),
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

fn frame(id: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "connector".to_string(),
        direction: "bidirectional".to_string(),
        type_name: "Modelica.Mechanics.MultiBody.Interfaces.Frame_a".to_string(),
    }
}

fn p(name: &str, value: &str) -> CatalogParam {
    CatalogParam::real(name, value)
}

fn mech_entry(
    id: &str,
    type_name: &str,
    domain: &str,
    category: &str,
    prefix: &str,
    params: Vec<CatalogParam>,
    ports: Vec<PortDef>,
    tags: &[&str],
    plot: &[&str],
) -> CatalogEntry {
    let mut t = vec![domain.to_string()];
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

pub fn extra_mechanics_multibody_blocks() -> Vec<CatalogEntry> {
    vec![
        // --- Translational 1D (P479–P488) ---
        mech_entry(
            "mechanics.translational.mass",
            "Modelica.Mechanics.Translational.Components.Mass",
            "mechanical",
            "Translational",
            "mass",
            vec![p("m", "1"), p("L", "0").with_unit("m", "Length")],
            vec![tflange("flange_a"), tflange("flange_b")],
            &["translational", "mass"],
            &["s", "v"],
        ),
        mech_entry(
            "mechanics.translational.spring",
            "Modelica.Mechanics.Translational.Components.Spring",
            "mechanical",
            "Translational",
            "tspring",
            vec![
                p("c", "1").with_unit("N/m", "Spring constant"),
                p("s_rel0", "0"),
            ],
            vec![tflange("flange_a"), tflange("flange_b")],
            &["translational"],
            &[],
        ),
        mech_entry(
            "mechanics.translational.damper",
            "Modelica.Mechanics.Translational.Components.Damper",
            "mechanical",
            "Translational",
            "tdamper",
            vec![p("d", "1").with_unit("N.s/m", "Damping")],
            vec![tflange("flange_a"), tflange("flange_b")],
            &["translational"],
            &[],
        ),
        mech_entry(
            "mechanics.translational.springDamper",
            "Modelica.Mechanics.Translational.Components.SpringDamper",
            "mechanical",
            "Translational",
            "tspringDamper",
            vec![
                p("c", "1").with_unit("N/m", "Spring constant"),
                p("d", "0.1").with_unit("N.s/m", "Damping"),
                p("s_rel0", "0"),
            ],
            vec![tflange("flange_a"), tflange("flange_b")],
            &["translational"],
            &[],
        ),
        mech_entry(
            "mechanics.translational.fixed",
            "Modelica.Mechanics.Translational.Components.Fixed",
            "mechanical",
            "Translational",
            "tfixed",
            vec![p("s0", "0")],
            vec![tflange("flange")],
            &["translational", "boundary"],
            &[],
        ),
        mech_entry(
            "mechanics.translational.force",
            "Modelica.Mechanics.Translational.Sources.Force",
            "mechanical",
            "Translational",
            "force",
            vec![],
            vec![signal_port("f", "in"), tflange("flange")],
            &["translational", "source"],
            &[],
        ),
        mech_entry(
            "mechanics.translational.speed",
            "Modelica.Mechanics.Translational.Sources.Speed",
            "mechanical",
            "Translational",
            "tspeed",
            vec![],
            vec![signal_port("v_ref", "in"), tflange("flange")],
            &["translational", "source"],
            &[],
        ),
        mech_entry(
            "mechanics.translational.position",
            "Modelica.Mechanics.Translational.Sources.Position",
            "mechanical",
            "Translational",
            "tposition",
            vec![],
            vec![signal_port("s_ref", "in"), tflange("flange")],
            &["translational", "source"],
            &[],
        ),
        mech_entry(
            "mechanics.translational.positionSensor",
            "Modelica.Mechanics.Translational.Sensors.PositionSensor",
            "mechanical",
            "Translational",
            "posSensor",
            vec![],
            vec![tflange("flange"), signal_port("s", "out")],
            &["translational", "sensor"],
            &["s"],
        ),
        mech_entry(
            "mechanics.translational.forceSensor",
            "Modelica.Mechanics.Translational.Sensors.ForceSensor",
            "mechanical",
            "Translational",
            "forceSensor",
            vec![],
            vec![
                tflange("flange_a"),
                tflange("flange_b"),
                signal_port("f", "out"),
            ],
            &["translational", "sensor"],
            &["f"],
        ),
        mech_entry(
            "mechanics.translational.idealGearR2T",
            "Modelica.Mechanics.Translational.Components.IdealGearR2T",
            "mechanical",
            "Translational",
            "gearR2T",
            vec![p("ratio", "1")],
            vec![rflange("flangeR"), tflange("flangeT")],
            &["translational", "gear", "coupling"],
            &[],
        ),
        // --- Multibody joints / parts (P489–P498) ---
        mech_entry(
            "multibody.prismatic",
            "Modelica.Mechanics.MultiBody.Joints.Prismatic",
            "multibody",
            "MultiBody",
            "prismatic",
            vec![p("n", "{1,0,0}")],
            vec![frame("frame_a"), frame("frame_b")],
            &["multibody", "joint", "3d"],
            &["s", "v"],
        ),
        mech_entry(
            "multibody.spherical",
            "Modelica.Mechanics.MultiBody.Joints.Spherical",
            "multibody",
            "MultiBody",
            "spherical",
            vec![],
            vec![frame("frame_a"), frame("frame_b")],
            &["multibody", "joint", "3d"],
            &[],
        ),
        mech_entry(
            "multibody.universal",
            "Modelica.Mechanics.MultiBody.Joints.Universal",
            "multibody",
            "MultiBody",
            "universal",
            vec![p("n_a", "{1,0,0}"), p("n_b", "{0,1,0}")],
            vec![frame("frame_a"), frame("frame_b")],
            &["multibody", "joint", "3d"],
            &["phi_a", "phi_b"],
        ),
        mech_entry(
            "multibody.freeMotion",
            "Modelica.Mechanics.MultiBody.Joints.FreeMotion",
            "multibody",
            "MultiBody",
            "freeMotion",
            vec![],
            vec![frame("frame_a"), frame("frame_b")],
            &["multibody", "joint", "3d"],
            &["r_rel_a[1]", "r_rel_a[2]", "r_rel_a[3]"],
        ),
        mech_entry(
            "multibody.fixedRotation",
            "Modelica.Mechanics.MultiBody.Parts.FixedRotation",
            "multibody",
            "MultiBody",
            "fixedRot",
            vec![p("n", "{0,0,1}"), p("angle", "0")],
            vec![frame("frame_a"), frame("frame_b")],
            &["multibody", "3d"],
            &[],
        ),
        mech_entry(
            "multibody.bodyCylinder",
            "Modelica.Mechanics.MultiBody.Parts.BodyCylinder",
            "multibody",
            "MultiBody",
            "cylinder",
            vec![
                p("r", "{0.5,0,0}"),
                p("diameter", "0.1"),
                p("density", "7700"),
            ],
            vec![frame("frame_a"), frame("frame_b")],
            &["multibody", "3d", "body"],
            &["r_0[1]", "r_0[2]", "r_0[3]"],
        ),
        mech_entry(
            "multibody.pointMass",
            "Modelica.Mechanics.MultiBody.Parts.PointMass",
            "multibody",
            "MultiBody",
            "pointMass",
            vec![p("m", "1")],
            vec![frame("frame_a")],
            &["multibody", "3d", "body"],
            &["r_0[1]", "r_0[2]", "r_0[3]"],
        ),
        mech_entry(
            "multibody.revoluteActuated",
            "Modelica.Mechanics.MultiBody.Joints.Revolute",
            "multibody",
            "MultiBody",
            "revoluteA",
            vec![
                p("n", "{0,0,1}"),
                CatalogParam::typed("useAxisFlange", "true", "Boolean"),
            ],
            vec![frame("frame_a"), frame("frame_b"), rflange("axis")],
            &["multibody", "joint", "3d", "actuated"],
            &["phi", "w"],
        ),
        mech_entry(
            "multibody.prismaticActuated",
            "Modelica.Mechanics.MultiBody.Joints.Prismatic",
            "multibody",
            "MultiBody",
            "prismaticA",
            vec![
                p("n", "{1,0,0}"),
                CatalogParam::typed("useAxisFlange", "true", "Boolean"),
            ],
            vec![frame("frame_a"), frame("frame_b"), tflange("axis")],
            &["multibody", "joint", "3d", "actuated"],
            &["s", "v"],
        ),
    ]
}
