//! Emit `default_function_builtin_rules.json` into `OUT_DIR` (same data as former `scripts/gen_function_builtin_rules.py`).

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let dest = Path::new(&out_dir).join("default_function_builtin_rules.json");

    type Pair = (&'static str, &'static str);
    type Group = (&'static str, &'static [Pair]);

    const GROUPS: &[Group] = &[
        (
            "sample_interval",
            &[
                ("equals", "sample"),
                ("ends_with", ".sample"),
                ("equals", "interval"),
                ("ends_with", ".interval"),
            ],
        ),
        (
            "passthrough_first_empty0",
            &[
                ("equals", "Utilities.regRoot2"),
                ("ends_with", ".Utilities.regRoot2"),
                ("equals", "Utilities.regRoot"),
                ("ends_with", ".Utilities.regRoot"),
                ("equals", "Utilities.regSquare2"),
                ("ends_with", ".Utilities.regSquare2"),
                ("ends_with", "massFlowRate_dp_and_Re"),
                ("contains", ".massFlowRate_dp_and_Re"),
                ("starts_with", "WallFriction."),
                ("contains", ".WallFriction."),
                ("equals", "Modelica.Fluid.Utilities.regFun3"),
                ("ends_with", ".regFun3"),
                ("equals", "Utilities.regFun3"),
                ("ends_with", ".Utilities.regFun3"),
                ("equals", "flowCharacteristic"),
                ("ends_with", ".flowCharacteristic"),
                ("equals", "efficiencyCharacteristic"),
                ("ends_with", ".efficiencyCharacteristic"),
                ("equals", "distribution"),
                ("ends_with", ".distribution"),
                ("equals", "realFFT"),
                ("ends_with", ".realFFT"),
                ("equals", "realFFTsamplePoints"),
                ("ends_with", ".realFFTsamplePoints"),
                ("contains", "pressureLoss"),
                ("starts_with", "FCN"),
                ("starts_with", "Modelica.Math."),
                ("starts_with", "Modelica.Electrical.Polyphase."),
                ("starts_with", "Polyphase."),
                ("contains", ".Electrical.Polyphase."),
                ("contains", ".Polyphase."),
                ("equals", "positiveMax"),
                ("equals", "xtCharacteristic"),
                ("equals", "FlCharacteristic"),
                ("equals", "cross"),
                ("equals", "Complex"),
                ("equals", "real"),
                ("ends_with", ".real"),
                ("equals", "conj"),
                ("ends_with", ".conj"),
                ("equals", "linearTemperatureDependency"),
                ("equals", "transpose"),
                ("equals", "vector"),
                ("equals", "fill"),
                ("equals", "Clock"),
                ("ends_with", ".Clock"),
                ("equals", "noClock"),
                ("ends_with", ".noClock"),
                ("equals", "hold"),
                ("ends_with", ".hold"),
                ("equals", "previous"),
                ("ends_with", ".previous"),
                ("equals", "Integer"),
                ("equals", "Real"),
                ("equals", "position"),
                ("ends_with", ".position"),
                ("equals", "oneTrue"),
                ("ends_with", ".oneTrue"),
                ("equals", "delay"),
                ("ends_with", ".delay"),
                ("equals", "exlin"),
                ("equals", "exlin2"),
                ("ends_with", ".exlin"),
                ("ends_with", ".exlin2"),
                ("contains", "ExternalCombiTable1D"),
                ("ends_with", "getTable1DValue"),
                ("ends_with", "getTable1DValueNoDer"),
                ("ends_with", "getTable1DValueNoDer2"),
            ],
        ),
        (
            "passthrough_first_empty1",
            &[("ends_with", "powerOfJ"), ("contains", ".powerOfJ")],
        ),
        (
            "const0_warn_gravity",
            &[
                ("ends_with", "gravityAcceleration"),
                ("contains", ".gravityAcceleration"),
            ],
        ),
        ("const0_warn_medium", &[("starts_with", "Medium.")]),
        (
            "const0_warn_internal",
            &[("starts_with", "Internal."), ("contains", ".Internal.")],
        ),
        (
            "reg_step_blend",
            &[
                ("equals", "Modelica.Fluid.Utilities.regStep"),
                ("ends_with", ".regStep"),
                ("equals", "Utilities.regStep"),
                ("ends_with", ".Utilities.regStep"),
            ],
        ),
        (
            "splice_blend",
            &[
                ("equals", "Modelica.Fluid.Utilities.spliceFunction"),
                ("ends_with", ".spliceFunction"),
                ("equals", "Utilities.spliceFunction"),
                ("ends_with", ".Utilities.spliceFunction"),
            ],
        ),
        ("const0_warn_connections", &[("starts_with", "Connections.")]),
        (
            "const0_warn_noise",
            &[("equals", "generateNoise"), ("ends_with", ".generateNoise")],
        ),
        (
            "interp_coef",
            &[
                ("ends_with", "getInterpolationCoefficients"),
                ("contains", ".getInterpolationCoefficients"),
            ],
        ),
        (
            "semi_linear",
            &[("equals", "semiLinear"), ("ends_with", ".semiLinear")],
        ),
        (
            "outer_product",
            &[("equals", "outerProduct"), ("ends_with", ".outerProduct")],
        ),
        (
            "identity_jit",
            &[("equals", "identity"), ("ends_with", ".identity")],
        ),
        ("skew_jit", &[("equals", "skew"), ("ends_with", ".skew")]),
        (
            "const0_warn_baseclasses",
            &[
                ("starts_with", "BaseClasses."),
                ("contains", ".BaseClasses."),
            ],
        ),
        (
            "const0_warn_frames",
            &[("starts_with", "Frames."), ("contains", ".Frames.")],
        ),
        ("noevent_1", &[("equals", "noEvent")]),
        (
            "instream",
            &[("equals", "inStream"), ("ends_with", ".inStream")],
        ),
        (
            "actualstream",
            &[("equals", "actualStream"), ("ends_with", ".actualStream")],
        ),
        ("valve_char_1", &[("equals", "valveCharacteristic")]),
        ("imag_zero", &[("equals", "imag"), ("ends_with", ".imag")]),
        ("cardinality_zero", &[("equals", "cardinality")]),
        ("initial_fn", &[("equals", "initial")]),
        ("terminal_fn", &[("equals", "terminal")]),
        ("boolean_1", &[("equals", "Boolean")]),
        ("abs_1", &[("equals", "abs")]),
        ("max_2", &[("equals", "max")]),
        ("min_2", &[("equals", "min")]),
        ("integer_1", &[("equals", "integer")]),
        ("homotopy_var", &[("equals", "homotopy")]),
        ("size_jit", &[("equals", "size")]),
        (
            "first_tick",
            &[("equals", "firstTick"), ("ends_with", ".firstTick")],
        ),
        (
            "first_true_index",
            &[
                (
                    "equals",
                    "Modelica.Math.BooleanVectors.firstTrueIndex",
                ),
                ("ends_with", ".firstTrueIndex"),
            ],
        ),
        (
            "interpolate",
            &[
                ("equals", "Modelica.Math.Vectors.interpolate"),
                ("ends_with", ".interpolate"),
            ],
        ),
        ("get_next_time_event", &[("ends_with", "getNextTimeEvent")]),
        (
            "is_empty_one",
            &[
                ("equals", "Modelica.Utilities.Strings.isEmpty"),
                ("ends_with", ".isEmpty"),
            ],
        ),
        ("named_last", &[("equals", "named")]),
        ("cat", &[("equals", "cat"), ("ends_with", ".cat")]),
        (
            "modelicatest_one",
            &[
                ("starts_with", "ModelicaTest.Math."),
                ("starts_with", "ModelicaTest.ComplexMath."),
            ],
        ),
        ("not_1", &[("equals", "not")]),
        (
            "clock_derived",
            &[
                ("equals", "subSample"),
                ("equals", "backSample"),
                ("equals", "superSample"),
                ("equals", "shiftSample"),
                ("ends_with", ".backSample"),
                ("ends_with", ".subSample"),
                ("ends_with", ".superSample"),
                ("ends_with", ".shiftSample"),
            ],
        ),
        (
            "number_symmetric",
            &[
                ("equals", "numberOfSymmetricBaseSystems"),
                ("ends_with", ".numberOfSymmetricBaseSystems"),
            ],
        ),
        (
            "combitable_err0",
            &[
                ("contains", "CombiTimeTable"),
                ("contains", "getTimeTableValue"),
            ],
        ),
        (
            "ext_object_err0",
            &[
                ("contains", "ExternalObject"),
                ("ends_with", ".ExternalObject"),
            ],
        ),
        (
            "ext_combitimetable_warn0",
            &[("ends_with", "ExternalCombiTimeTable")],
        ),
        (
            "loadresource_warn0",
            &[("equals", "loadResource"), ("ends_with", ".loadResource")],
        ),
        ("zeros_fn", &[("equals", "zeros")]),
        ("ones_fn", &[("equals", "ones")]),
        (
            "type_conv_pf0",
            &[
                ("equals", "Integer"),
                ("ends_with", ".Integer"),
                ("equals", "Real"),
                ("ends_with", ".Real"),
                ("ends_with", ".Boolean"),
            ],
        ),
        ("product_fn", &[("equals", "product")]),
        ("sum_fn", &[("equals", "sum")]),
    ];

    let mut rules = Vec::new();
    for (handler_id, pairs) in GROUPS {
        for (op, pattern) in *pairs {
            rules.push(serde_json::json!({
                "handler_id": handler_id,
                "op": op,
                "pattern": pattern,
            }));
        }
    }

    let doc = serde_json::json!({
        "schema_version": 1,
        "rules": rules,
    });

    fs::write(
        &dest,
        serde_json::to_string_pretty(&doc).expect("serialize function builtin rules"),
    )
    .expect("write default_function_builtin_rules.json");

    println!("cargo:rerun-if-changed=build.rs");
}
