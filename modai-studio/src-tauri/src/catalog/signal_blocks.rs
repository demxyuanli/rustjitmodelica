//! Extra signal-domain catalog entries (Math / Nonlinear / Sources / Continuous / Tables / Sinks).

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

fn p(name: &str, value: &str) -> CatalogParam {
    CatalogParam::real(name, value)
}

fn entry(
    id: &str,
    type_name: &str,
    category: &str,
    prefix: &str,
    params: Vec<CatalogParam>,
    ports: Vec<PortDef>,
    extra_tags: &[&str],
    plot: &[&str],
) -> CatalogEntry {
    let mut tags = vec!["signal".to_string()];
    for t in extra_tags {
        tags.push((*t).to_string());
    }
    CatalogEntry {
        id: id.to_string(),
        type_name: type_name.to_string(),
        domain: "signal".into(),
        category: category.into(),
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

/// P338–P352 catalog expansions.
pub fn extra_signal_blocks() -> Vec<CatalogEntry> {
    vec![
        // P338 Product
        entry(
            "blocks.math.product",
            "Modelica.Blocks.Math.Product",
            "Math",
            "product",
            vec![],
            vec![
                signal_port("u1", "in"),
                signal_port("u2", "in"),
                signal_port("y", "out"),
            ],
            &["math"],
            &["y"],
        ),
        // P339 Division
        entry(
            "blocks.math.division",
            "Modelica.Blocks.Math.Division",
            "Math",
            "div",
            vec![],
            vec![
                signal_port("u1", "in"),
                signal_port("u2", "in"),
                signal_port("y", "out"),
            ],
            &["math"],
            &["y"],
        ),
        // P340 Abs / Sqrt / Sign
        entry(
            "blocks.math.abs",
            "Modelica.Blocks.Math.Abs",
            "Math",
            "abs",
            vec![],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["math"],
            &["y"],
        ),
        entry(
            "blocks.math.sqrt",
            "Modelica.Blocks.Math.Sqrt",
            "Math",
            "sqrt",
            vec![],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["math"],
            &["y"],
        ),
        entry(
            "blocks.math.sign",
            "Modelica.Blocks.Math.Sign",
            "Math",
            "sign",
            vec![],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["math"],
            &["y"],
        ),
        // P341 Trig
        entry(
            "blocks.math.sin",
            "Modelica.Blocks.Math.Sin",
            "Math",
            "sin",
            vec![],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["math", "trig"],
            &["y"],
        ),
        entry(
            "blocks.math.cos",
            "Modelica.Blocks.Math.Cos",
            "Math",
            "cos",
            vec![],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["math", "trig"],
            &["y"],
        ),
        // P342 Inverse trig / Atan2
        entry(
            "blocks.math.asin",
            "Modelica.Blocks.Math.Asin",
            "Math",
            "asin",
            vec![],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["math", "trig"],
            &["y"],
        ),
        entry(
            "blocks.math.acos",
            "Modelica.Blocks.Math.Acos",
            "Math",
            "acos",
            vec![],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["math", "trig"],
            &["y"],
        ),
        entry(
            "blocks.math.atan",
            "Modelica.Blocks.Math.Atan",
            "Math",
            "atan",
            vec![],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["math", "trig"],
            &["y"],
        ),
        entry(
            "blocks.math.atan2",
            "Modelica.Blocks.Math.Atan2",
            "Math",
            "atan2",
            vec![],
            vec![
                signal_port("u1", "in"),
                signal_port("u2", "in"),
                signal_port("y", "out"),
            ],
            &["math", "trig"],
            &["y"],
        ),
        // P343 Limiter (Saturation)
        entry(
            "blocks.nonlinear.limiter",
            "Modelica.Blocks.Nonlinear.Limiter",
            "Nonlinear",
            "limiter",
            vec![p("uMax", "1"), p("uMin", "-1")],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["nonlinear"],
            &["y"],
        ),
        // P344 DeadZone
        entry(
            "blocks.nonlinear.deadZone",
            "Modelica.Blocks.Nonlinear.DeadZone",
            "Nonlinear",
            "deadZone",
            vec![p("uMax", "0.1"), p("uMin", "-0.1")],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["nonlinear"],
            &["y"],
        ),
        // P345 Ramp
        entry(
            "blocks.sources.ramp",
            "Modelica.Blocks.Sources.Ramp",
            "Sources",
            "ramp",
            vec![
                p("height", "1"),
                p("duration", "1"),
                p("offset", "0"),
                p("startTime", "0"),
            ],
            vec![signal_port("y", "out")],
            &["source"],
            &["y"],
        ),
        // P346 Pulse
        entry(
            "blocks.sources.pulse",
            "Modelica.Blocks.Sources.Pulse",
            "Sources",
            "pulse",
            vec![
                p("amplitude", "1"),
                p("width", "50"),
                p("period", "1"),
                p("nperiod", "-1"),
                p("offset", "0"),
                p("startTime", "0"),
            ],
            vec![signal_port("y", "out")],
            &["source"],
            &["y"],
        ),
        // P347 ExpSine
        entry(
            "blocks.sources.expSine",
            "Modelica.Blocks.Sources.ExpSine",
            "Sources",
            "expSine",
            vec![
                p("amplitude", "1"),
                p("f", "1"),
                p("phase", "0"),
                p("damping", "1"),
                p("offset", "0"),
                p("startTime", "0"),
            ],
            vec![signal_port("y", "out")],
            &["source"],
            &["y"],
        ),
        // P348 Lookup1D
        entry(
            "blocks.tables.lookup1d",
            "Modelica.Blocks.Tables.CombiTable1Ds",
            "Tables",
            "lookup",
            vec![
                CatalogParam::typed("table", "[0,0; 1,1]", "Real"),
                p("columns", "{2}"),
                CatalogParam::typed(
                    "smoothness",
                    "Modelica.Blocks.Types.Smoothness.LinearSegments",
                    "Enum",
                ),
            ],
            vec![
                signal_port("u", "in"),
                PortDef {
                    id: "y[1]".into(),
                    kind: "signal".into(),
                    direction: "out".into(),
                    type_name: "RealOutput".into(),
                },
            ],
            &["tables", "lookup"],
            &["y[1]"],
        ),
        // P349 TransferFunction
        entry(
            "blocks.continuous.transferFunction",
            "Modelica.Blocks.Continuous.TransferFunction",
            "Continuous",
            "tf",
            vec![
                CatalogParam::typed("b", "{1}", "Real").with_unit("1", "Numerator"),
                CatalogParam::typed("a", "{1,1}", "Real").with_unit("1", "Denominator"),
            ],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["continuous"],
            &["y"],
        ),
        // P350 Derivative
        entry(
            "blocks.continuous.derivative",
            "Modelica.Blocks.Continuous.Derivative",
            "Continuous",
            "der",
            vec![p("k", "1"), p("T", "0.01")],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["continuous"],
            &["y"],
        ),
        // P351 PI
        entry(
            "blocks.continuous.pi",
            "Modelica.Blocks.Continuous.PI",
            "Continuous",
            "pi",
            vec![p("k", "1"), p("T", "1")],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["continuous", "control"],
            &["y"],
        ),
        // P352 PID
        entry(
            "blocks.continuous.pid",
            "Modelica.Blocks.Continuous.PID",
            "Continuous",
            "pid",
            vec![p("k", "1"), p("Ti", "0.5"), p("Td", "0.1")],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["continuous", "control"],
            &["y"],
        ),
        // Display sink (visual + plotHints)
        entry(
            "blocks.sinks.display",
            "Modelica.Blocks.Routing.RealPassThrough",
            "Sinks",
            "display",
            vec![],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["sink", "display"],
            &["y"],
        ),
        // Scope sink (F50): bind Results series via plotHints
        entry(
            "blocks.sinks.scope",
            "Modelica.Blocks.Routing.RealPassThrough",
            "Sinks",
            "scope",
            vec![],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["sink", "scope"],
            &["y"],
        ),
        // P366 From File (CombiTimeTable, table filled from CSV)
        entry(
            "blocks.sources.fromFile",
            "Modelica.Blocks.Sources.CombiTimeTable",
            "Sources",
            "fromFile",
            vec![
                CatalogParam::typed("tableOnFile", "false", "Boolean"),
                CatalogParam::typed("table", "[0,0; 1,1; 2,0]", "Real"),
                CatalogParam::typed("columns", "{2}", "Integer"),
                CatalogParam::typed("fileName", "\"\"", "String"),
            ],
            vec![
                PortDef {
                    id: "y[1]".into(),
                    kind: "signal".into(),
                    direction: "out".into(),
                    type_name: "RealOutput".into(),
                },
            ],
            &["source", "fromFile"],
            &["y[1]"],
        ),
        // Clock
        entry(
            "blocks.sources.clock",
            "Modelica.Blocks.Sources.Clock",
            "Sources",
            "clock",
            vec![p("offset", "0"), p("startTime", "0")],
            vec![signal_port("y", "out")],
            &["source"],
            &["y"],
        ),
        // Boolean sources
        entry(
            "blocks.sources.booleanStep",
            "Modelica.Blocks.Sources.BooleanStep",
            "Sources",
            "boolStep",
            vec![
                CatalogParam::real("startTime", "0.5"),
                CatalogParam::typed("startValue", "false", "Boolean"),
            ],
            vec![
                PortDef {
                    id: "y".into(),
                    kind: "signal".into(),
                    direction: "out".into(),
                    type_name: "BooleanOutput".into(),
                },
            ],
            &["source", "boolean"],
            &["y"],
        ),
        entry(
            "blocks.sources.booleanPulse",
            "Modelica.Blocks.Sources.BooleanPulse",
            "Sources",
            "boolPulse",
            vec![
                p("width", "50"),
                p("period", "1"),
                p("startTime", "0"),
            ],
            vec![
                PortDef {
                    id: "y".into(),
                    kind: "signal".into(),
                    direction: "out".into(),
                    type_name: "BooleanOutput".into(),
                },
            ],
            &["source", "boolean"],
            &["y"],
        ),
        entry(
            "blocks.sources.booleanConstant",
            "Modelica.Blocks.Sources.BooleanConstant",
            "Sources",
            "boolConst",
            vec![CatalogParam::typed("k", "true", "Boolean")],
            vec![
                PortDef {
                    id: "y".into(),
                    kind: "signal".into(),
                    direction: "out".into(),
                    type_name: "BooleanOutput".into(),
                },
            ],
            &["source", "boolean"],
            &["y"],
        ),
        // P419–P423 Discrete (samplePeriod from DiscreteSISO)
        entry(
            "blocks.discrete.sampler",
            "Modelica.Blocks.Discrete.Sampler",
            "Discrete",
            "sampler",
            vec![p("samplePeriod", "0.1"), p("startTime", "0")],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["discrete", "sample"],
            &["y"],
        ),
        entry(
            "blocks.discrete.zeroOrderHold",
            "Modelica.Blocks.Discrete.ZeroOrderHold",
            "Discrete",
            "zoh",
            vec![p("samplePeriod", "0.1"), p("startTime", "0")],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["discrete", "sample"],
            &["y"],
        ),
        entry(
            "blocks.discrete.firstOrderHold",
            "Modelica.Blocks.Discrete.FirstOrderHold",
            "Discrete",
            "foh",
            vec![p("samplePeriod", "0.1"), p("startTime", "0")],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["discrete", "sample"],
            &["y"],
        ),
        entry(
            "blocks.discrete.unitDelay",
            "Modelica.Blocks.Discrete.UnitDelay",
            "Discrete",
            "unitDelay",
            vec![
                p("samplePeriod", "0.1"),
                p("startTime", "0"),
                p("y_start", "0"),
            ],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["discrete", "sample"],
            &["y"],
        ),
        entry(
            "blocks.discrete.transferFunction",
            "Modelica.Blocks.Discrete.TransferFunction",
            "Discrete",
            "dtf",
            vec![
                p("samplePeriod", "0.1"),
                p("startTime", "0"),
                CatalogParam::typed("b", "{1}", "Real").with_unit("1", "Numerator"),
                CatalogParam::typed("a", "{1}", "Real").with_unit("1", "Denominator"),
            ],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["discrete", "sample"],
            &["y"],
        ),
        // RATE / F89 follow-up: Simulink-like Rate Transition (MSL Discrete mapping)
        entry(
            "blocks.discrete.rateTransition",
            "Modelica.Blocks.Discrete.ZeroOrderHold",
            "Discrete",
            "rateTransition",
            vec![p("samplePeriod", "0.1"), p("startTime", "0")],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["discrete", "sample", "rateTransition", "multiRate"],
            &["y"],
        ),
        entry(
            "blocks.discrete.rateTransitionUd",
            "Modelica.Blocks.Discrete.UnitDelay",
            "Discrete",
            "rateTransitionUd",
            vec![
                p("samplePeriod", "0.1"),
                p("startTime", "0"),
                p("y_start", "0"),
            ],
            vec![signal_port("u", "in"), signal_port("y", "out")],
            &["discrete", "sample", "rateTransition", "multiRate", "integrity"],
            &["y"],
        ),
        // P424 Lookup2D
        entry(
            "blocks.tables.lookup2d",
            "Modelica.Blocks.Tables.CombiTable2Ds",
            "Tables",
            "lookup2d",
            vec![
                CatalogParam::typed("table", "[0,0,1; 0,0,1; 1,1,2]", "Real"),
                CatalogParam::typed(
                    "smoothness",
                    "Modelica.Blocks.Types.Smoothness.LinearSegments",
                    "Enum",
                ),
            ],
            vec![
                signal_port("u1", "in"),
                signal_port("u2", "in"),
                signal_port("y", "out"),
            ],
            &["tables", "lookup"],
            &["y"],
        ),
        // P451–P456 Logical / Switch
        entry(
            "blocks.logical.and",
            "Modelica.Blocks.Logical.And",
            "Logical",
            "and",
            vec![],
            vec![
                PortDef {
                    id: "u1".into(),
                    kind: "signal".into(),
                    direction: "in".into(),
                    type_name: "BooleanInput".into(),
                },
                PortDef {
                    id: "u2".into(),
                    kind: "signal".into(),
                    direction: "in".into(),
                    type_name: "BooleanInput".into(),
                },
                PortDef {
                    id: "y".into(),
                    kind: "signal".into(),
                    direction: "out".into(),
                    type_name: "BooleanOutput".into(),
                },
            ],
            &["logical"],
            &["y"],
        ),
        entry(
            "blocks.logical.or",
            "Modelica.Blocks.Logical.Or",
            "Logical",
            "or",
            vec![],
            vec![
                PortDef {
                    id: "u1".into(),
                    kind: "signal".into(),
                    direction: "in".into(),
                    type_name: "BooleanInput".into(),
                },
                PortDef {
                    id: "u2".into(),
                    kind: "signal".into(),
                    direction: "in".into(),
                    type_name: "BooleanInput".into(),
                },
                PortDef {
                    id: "y".into(),
                    kind: "signal".into(),
                    direction: "out".into(),
                    type_name: "BooleanOutput".into(),
                },
            ],
            &["logical"],
            &["y"],
        ),
        entry(
            "blocks.logical.not",
            "Modelica.Blocks.Logical.Not",
            "Logical",
            "not",
            vec![],
            vec![
                PortDef {
                    id: "u".into(),
                    kind: "signal".into(),
                    direction: "in".into(),
                    type_name: "BooleanInput".into(),
                },
                PortDef {
                    id: "y".into(),
                    kind: "signal".into(),
                    direction: "out".into(),
                    type_name: "BooleanOutput".into(),
                },
            ],
            &["logical"],
            &["y"],
        ),
        entry(
            "blocks.logical.switch",
            "Modelica.Blocks.Logical.Switch",
            "Logical",
            "switch",
            vec![],
            vec![
                signal_port("u1", "in"),
                PortDef {
                    id: "u2".into(),
                    kind: "signal".into(),
                    direction: "in".into(),
                    type_name: "BooleanInput".into(),
                },
                signal_port("u3", "in"),
                signal_port("y", "out"),
            ],
            &["logical", "switch"],
            &["y"],
        ),
        entry(
            "blocks.math.booleanToReal",
            "Modelica.Blocks.Math.BooleanToReal",
            "Math",
            "boolToReal",
            vec![p("realTrue", "1"), p("realFalse", "0")],
            vec![
                PortDef {
                    id: "u".into(),
                    kind: "signal".into(),
                    direction: "in".into(),
                    type_name: "BooleanInput".into(),
                },
                signal_port("y", "out"),
            ],
            &["math", "logical"],
            &["y"],
        ),
        entry(
            "blocks.math.realToBoolean",
            "Modelica.Blocks.Math.RealToBoolean",
            "Math",
            "realToBool",
            vec![p("threshold", "0.5")],
            vec![
                signal_port("u", "in"),
                PortDef {
                    id: "y".into(),
                    kind: "signal".into(),
                    direction: "out".into(),
                    type_name: "BooleanOutput".into(),
                },
            ],
            &["math", "logical"],
            &["y"],
        ),
    ]
}
