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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogParam {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub value_type: Option<String>,
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
