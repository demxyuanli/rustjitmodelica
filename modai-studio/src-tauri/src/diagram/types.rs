use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioProjectManifest {
    pub schema_version: u32,
    pub product: String,
    pub model_name: String,
    #[serde(default)]
    pub library_paths: Vec<String>,
    pub simulation_defaults: SimulationDefaults,
    #[serde(default)]
    pub ui: StudioUiPrefs,
    /// Project-level parameter dictionary (F85). Values referenced as `$Name` on blocks.
    #[serde(default)]
    pub param_dictionary: Vec<ParamDictionaryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamDictionaryEntry {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationDefaults {
    pub stop_time: f64,
    pub number_of_intervals: u32,
    pub solver: String,
    pub tolerance: f64,
    #[serde(default)]
    pub max_step: Option<f64>,
    #[serde(default)]
    pub min_step: Option<f64>,
    #[serde(default)]
    pub logged_variables: Vec<String>,
}

impl Default for SimulationDefaults {
    fn default() -> Self {
        Self {
            stop_time: 1.0,
            number_of_intervals: 500,
            solver: "rk45".to_string(),
            tolerance: 1e-6,
            max_step: None,
            min_step: None,
            logged_variables: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioUiPrefs {
    #[serde(default = "default_locale")]
    pub locale: String,
}

fn default_locale() -> String {
    "zh".to_string()
}

impl Default for StudioUiPrefs {
    fn default() -> Self {
        Self {
            locale: default_locale(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramDocument {
    pub schema_version: u32,
    pub model_name: String,
    pub diagram_id: String,
    #[serde(default)]
    pub parent_diagram_id: Option<String>,
    #[serde(default)]
    pub nodes: Vec<DiagramNode>,
    #[serde(default)]
    pub edges: Vec<DiagramEdge>,
    #[serde(default)]
    pub annotations: Vec<serde_json::Value>,
    #[serde(default)]
    pub viewport: Option<Viewport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramNode {
    pub id: String,
    pub instance_name: String,
    pub type_name: String,
    pub domain: String,
    pub catalog_id: String,
    pub position: Position,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub params: Vec<ParamValue>,
    #[serde(default)]
    pub ports: Vec<PortDef>,
    #[serde(default = "default_node_kind")]
    pub node_kind: String,
    #[serde(default)]
    pub subsystem_ref: Option<String>,
    #[serde(default)]
    pub plot_hints: Vec<String>,
    #[serde(default)]
    pub ui: Option<NodeUi>,
}

fn default_node_kind() -> String {
    "component".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamValue {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortDef {
    pub id: String,
    pub kind: String,
    pub direction: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeUi {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
    /// Maps subsystem boundary port id -> inner node/port.
    #[serde(default)]
    pub port_bindings: Option<std::collections::HashMap<String, PortBinding>>,
    /// Relative or absolute path to extracted FMU directory (node_kind == "fmu").
    #[serde(default)]
    pub fmu_asset: Option<String>,
    #[serde(default)]
    pub fmu_guid: Option<String>,
    #[serde(default)]
    pub fmu_model_identifier: Option<String>,
    /// When true, instance name is drawn below the block body.
    #[serde(default)]
    pub label_below: Option<bool>,
    /// Modelica Icon annotation JSON (optional block face).
    #[serde(default)]
    pub icon: Option<serde_json::Value>,
    /// Relative .mo path for Model Reference / userlib (e.g. wrapped/Foo.mo).
    #[serde(default)]
    pub mo_asset: Option<String>,
    /// Variant Subsystem choices (F87); active choice is mirrored in subsystem_ref.
    #[serde(default)]
    pub variants: Option<Vec<VariantChoice>>,
    #[serde(default)]
    pub active_variant_id: Option<String>,
    /// Param names pinned to the pre-run dashboard.
    #[serde(default)]
    pub dashboard_pins: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantChoice {
    pub id: String,
    pub label: String,
    pub subsystem_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortBinding {
    pub node_id: String,
    pub port_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramEdge {
    pub id: String,
    pub source_node_id: String,
    pub source_port_id: String,
    pub target_node_id: String,
    pub target_port_id: String,
    #[serde(default)]
    pub signal_name: Option<String>,
    /// Unit annotation for signal / physical lines (F72 display).
    #[serde(default)]
    pub unit: Option<String>,
    /// Dimension hint, e.g. "1", "3", "[2]".
    #[serde(default)]
    pub dimension: Option<String>,
    #[serde(default)]
    pub vertices: Vec<Position>,
    #[serde(default)]
    pub routing: Option<String>,
    #[serde(default)]
    pub ui: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub id: String,
    pub type_name: String,
    pub domain: String,
    pub category: String,
    pub display_key: String,
    pub default_instance_prefix: String,
    pub requires_library: String,
    pub default_params: Vec<CatalogParam>,
    pub ports: Vec<PortDef>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub plot_hints: Vec<String>,
    /// Set when this catalog entry is an imported FMU.
    #[serde(default)]
    pub fmu_asset: Option<String>,
    #[serde(default)]
    pub fmu_guid: Option<String>,
    #[serde(default)]
    pub fmu_model_identifier: Option<String>,
    /// Modelica Icon annotation graphics for block face.
    #[serde(default)]
    pub icon: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogParam {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub value_type: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

impl CatalogParam {
    pub fn typed(name: impl Into<String>, value: impl Into<String>, value_type: &str) -> Self {
        let name = name.into();
        let (unit, description) = known_param_meta(name.as_str());
        Self {
            name,
            value: value.into(),
            value_type: Some(value_type.to_string()),
            unit,
            description,
        }
    }

    pub fn real(name: &str, value: &str) -> Self {
        Self::typed(name, value, "Real")
    }

    pub fn with_unit(mut self, unit: &str, description: &str) -> Self {
        self.unit = Some(unit.to_string());
        self.description = Some(description.to_string());
        self
    }
}

fn known_param_meta(name: &str) -> (Option<String>, Option<String>) {
    let pair = match name {
        "R" => Some(("Ohm", "Resistance")),
        "C" => Some(("F", "Capacitance")),
        "V" => Some(("V", "Voltage")),
        "I" => Some(("A", "Current")),
        "Ron" => Some(("Ohm", "On-state resistance")),
        "Goff" => Some(("S", "Off-state conductance")),
        "Vknee" => Some(("V", "Knee voltage")),
        "samplePeriod" => Some(("s", "Sample period")),
        "startTime" => Some(("s", "Start time")),
        "period" => Some(("s", "Period")),
        "duration" => Some(("s", "Duration")),
        "f" => Some(("Hz", "Frequency")),
        "phase" => Some(("rad", "Phase")),
        "amplitude" => Some(("1", "Amplitude")),
        "offset" => Some(("1", "Offset")),
        "Ti" => Some(("s", "Integral time")),
        "Td" => Some(("s", "Derivative time")),
        "J" => Some(("kg.m2", "Inertia")),
        "m" => Some(("kg", "Mass")),
        "density" => Some(("kg/m3", "Density")),
        "diameter" => Some(("m", "Diameter")),
        "phi0" => Some(("rad", "Initial angle")),
        "s0" => Some(("m", "Initial position")),
        "s_rel0" => Some(("m", "Unstretched length")),
        "ratio" => Some(("1", "Gear ratio")),
        "uMax" => Some(("1", "Upper limit")),
        "uMin" => Some(("1", "Lower limit")),
        "g" => Some(("m/s2", "Gravity acceleration")),
        "length" => Some(("m", "Length")),
        "width" => Some(("m", "Width")),
        "height" => Some(("m", "Height")),
        "k" => Some(("1", "Gain")),
        "k1" => Some(("1", "Gain u1")),
        "k2" => Some(("1", "Gain u2")),
        "T" => Some(("s", "Time constant")),
        _ => None,
    };
    match pair {
        Some((u, d)) => (Some(u.to_string()), Some(d.to_string())),
        None => (None, None),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioReport {
    pub success: bool,
    pub stage: String,
    pub errors: Vec<StudioDiagnostic>,
    #[serde(default)]
    pub warnings: Vec<StudioDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioDiagnostic {
    pub message: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub line: Option<u32>,
    #[serde(default)]
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioSimOptions {
    pub stop_time: f64,
    pub number_of_intervals: u32,
    pub solver: String,
    pub tolerance: f64,
}

/// Wall-clock phase timings for one simulate call (F89 Profiler).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SimProfile {
    /// Diagram -> Modelica emit.
    pub emit_ms: u64,
    /// Full compile / JIT; 0 when Fast Restart reuses Artifacts.
    pub compile_ms: u64,
    /// Parameter vector merge + Artifacts clone.
    pub apply_params_ms: u64,
    /// Integrator wall time.
    pub integrate_ms: u64,
    /// Sum of phases (may be slightly under client wallMs due to IPC).
    pub total_ms: u64,
    /// True when compile was skipped (Fast Restart hit).
    pub compile_skipped: bool,
    /// "fastRestart" | "fullCompile" | "fmu" | null
    #[serde(default)]
    pub restart_mode: Option<String>,
}

impl SimProfile {
    pub fn finalize(&mut self) {
        self.total_ms = self.emit_ms + self.compile_ms + self.apply_params_ms + self.integrate_ms;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenProjectResult {
    pub root_dir: String,
    pub manifest: StudioProjectManifest,
    pub diagram: DiagramDocument,
    pub model_source: String,
    #[serde(default)]
    pub subsystems: std::collections::HashMap<String, DiagramDocument>,
}
