//! Modelica.StateGraph catalog (F95). Same diagram.json + connect; no JSON statechart.

use crate::diagram::{CatalogEntry, CatalogParam, PortDef};

fn conn(id: &str, direction: &str, type_name: &str) -> PortDef {
    PortDef {
        id: id.to_string(),
        kind: "connector".to_string(),
        direction: direction.to_string(),
        type_name: type_name.to_string(),
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

fn step_ports() -> Vec<PortDef> {
    vec![
        conn(
            "inPort[1]",
            "in",
            "Modelica.StateGraph.Interfaces.Step_in",
        ),
        conn(
            "outPort[1]",
            "out",
            "Modelica.StateGraph.Interfaces.Step_out",
        ),
    ]
}

fn trans_ports() -> Vec<PortDef> {
    vec![
        conn(
            "inPort",
            "in",
            "Modelica.StateGraph.Interfaces.Transition_in",
        ),
        conn(
            "outPort",
            "out",
            "Modelica.StateGraph.Interfaces.Transition_out",
        ),
    ]
}

fn n_io() -> Vec<CatalogParam> {
    vec![
        CatalogParam::typed("nIn", "1", "Integer"),
        CatalogParam::typed("nOut", "1", "Integer"),
    ]
}

fn trans_params() -> Vec<CatalogParam> {
    vec![
        CatalogParam::typed("condition", "true", "Boolean"),
        CatalogParam::typed("enableTimer", "true", "Boolean"),
        CatalogParam::typed("waitTime", "1", "Real"),
    ]
}

fn entry(
    id: &str,
    type_name: &str,
    prefix: &str,
    params: Vec<CatalogParam>,
    ports: Vec<PortDef>,
    extra_tags: &[&str],
    plot: &[&str],
) -> CatalogEntry {
    let mut tags = vec!["stategraph".to_string(), "signal".to_string()];
    for t in extra_tags {
        tags.push((*t).to_string());
    }
    CatalogEntry {
        id: id.to_string(),
        type_name: type_name.to_string(),
        domain: "signal".into(),
        category: "StateGraph".into(),
        display_key: format!("catalog.{id}"),
        default_instance_prefix: prefix.into(),
        requires_library: "Modelica".into(),
        default_params: params,
        ports,
        tags,
        plot_hints: plot.iter().map(|s| (*s).to_string()).collect(),
        fmu_asset: None,
        fmu_guid: None,
        fmu_model_identifier: None,
        icon: None,
    }
}

pub fn extra_stategraph_blocks() -> Vec<CatalogEntry> {
    vec![
        entry(
            "stategraph.initialStep",
            "Modelica.StateGraph.InitialStep",
            "initialStep",
            n_io(),
            step_ports(),
            &["sg-step"],
            &["active"],
        ),
        entry(
            "stategraph.step",
            "Modelica.StateGraph.Step",
            "step",
            n_io(),
            step_ports(),
            &["sg-step"],
            &["active"],
        ),
        entry(
            "stategraph.stepWithSignal",
            "Modelica.StateGraph.StepWithSignal",
            "stepSig",
            n_io(),
            {
                let mut p = step_ports();
                p.push(bool_port("active", "out"));
                p
            },
            &["sg-step"],
            &["active"],
        ),
        entry(
            "stategraph.transition",
            "Modelica.StateGraph.Transition",
            "transition",
            trans_params(),
            trans_ports(),
            &["sg-transition"],
            &[],
        ),
        entry(
            "stategraph.transitionWithSignal",
            "Modelica.StateGraph.TransitionWithSignal",
            "transitionSig",
            vec![
                CatalogParam::typed("enableTimer", "false", "Boolean"),
                CatalogParam::typed("waitTime", "0", "Real"),
            ],
            {
                let mut p = trans_ports();
                p.push(bool_port("condition", "in"));
                p
            },
            &["sg-transition"],
            &[],
        ),
    ]
}
