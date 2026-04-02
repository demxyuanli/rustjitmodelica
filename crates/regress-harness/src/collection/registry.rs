use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use crate::runtime::scanner::ScanSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CollectionSource {
    AutoDirectory,
    AutoType,
    ManualSelection,
    FromFailures {
        source_run_id: String,
        include: Vec<String>,
    },
    FromConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCollection {
    pub collection_id: String,
    pub name: String,
    pub source: CollectionSource,
    pub source_run_id: Option<String>,
    pub scan_id: Option<String>,
    pub filter_expr: Option<String>,
    pub case_ids: Vec<String>,
    pub frozen: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionRegistry {
    pub schema_version: u32,
    pub collections: BTreeMap<String, TestCollection>,
}

impl CollectionRegistry {
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                schema_version: 1,
                collections: BTreeMap::new(),
            });
        }
        let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let mut reg: Self = serde_json::from_str(&text).context("parse collection registry")?;
        if reg.schema_version == 0 {
            reg.schema_version = 1;
        }
        Ok(reg)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn create_from_failures(
        &mut self,
        source_run_id: String,
        filter_expr: String,
        case_ids: Vec<String>,
        frozen: bool,
    ) -> String {
        let collection_id = format!(
            "col_fail_{}_{}",
            chrono::Utc::now().format("%Y%m%d%H%M%S"),
            source_run_id
        );
        self.collections.insert(
            collection_id.clone(),
            TestCollection {
                collection_id: collection_id.clone(),
                name: format!("failures_{source_run_id}"),
                source: CollectionSource::FromFailures {
                    source_run_id: source_run_id.clone(),
                    include: case_ids.clone(),
                },
                source_run_id: Some(source_run_id),
                scan_id: None,
                filter_expr: Some(filter_expr),
                case_ids,
                frozen,
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        );
        collection_id
    }

    pub fn create_from_config(
        &mut self,
        config_path: &Path,
        case_ids: Vec<String>,
    ) -> String {
        let collection_id = format!("col_cfg_{}", chrono::Utc::now().format("%Y%m%d%H%M%S"));
        self.collections.insert(
            collection_id.clone(),
            TestCollection {
                collection_id: collection_id.clone(),
                name: format!(
                    "from_config_{}",
                    config_path.file_stem().and_then(|x| x.to_str()).unwrap_or("config")
                ),
                source: CollectionSource::FromConfig,
                source_run_id: None,
                scan_id: None,
                filter_expr: Some(format!("config={}", config_path.display())),
                case_ids,
                frozen: true,
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        );
        collection_id
    }

    pub fn create_manual_selection(
        &mut self,
        name: String,
        case_ids: Vec<String>,
        scan_id: Option<String>,
    ) -> String {
        let collection_id = format!("col_manual_{}", chrono::Utc::now().format("%Y%m%d%H%M%S"));
        self.collections.insert(
            collection_id.clone(),
            TestCollection {
                collection_id: collection_id.clone(),
                name,
                source: CollectionSource::ManualSelection,
                source_run_id: None,
                scan_id,
                filter_expr: None,
                case_ids,
                frozen: false,
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        );
        collection_id
    }

    pub fn create_auto_directory_collections(&mut self, snapshot: &ScanSnapshot) -> Vec<String> {
        let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for m in &snapshot.models {
            groups
                .entry(m.dir_group.clone())
                .or_default()
                .push(m.case_id.clone());
        }
        let mut ids = Vec::new();
        for (group, mut case_ids) in groups {
            case_ids.sort();
            case_ids.dedup();
            let collection_id =
                format!("col_dir_{}_{}", sanitize_id(&group), chrono::Utc::now().format("%H%M%S"));
            self.collections.insert(
                collection_id.clone(),
                TestCollection {
                    collection_id: collection_id.clone(),
                    name: format!("dir::{group}"),
                    source: CollectionSource::AutoDirectory,
                    source_run_id: None,
                    scan_id: Some(snapshot.scan_id.clone()),
                    filter_expr: Some(format!("dir_group={group}")),
                    case_ids,
                    frozen: false,
                    created_at: chrono::Utc::now().to_rfc3339(),
                },
            );
            ids.push(collection_id);
        }
        ids
    }

    pub fn create_auto_type_collections(&mut self, snapshot: &ScanSnapshot) -> Vec<String> {
        let mut groups: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for m in &snapshot.models {
            groups
                .entry(m.model_type.clone())
                .or_default()
                .insert(m.case_id.clone());
        }
        let mut ids = Vec::new();
        for (kind, set) in groups {
            let collection_id =
                format!("col_type_{}_{}", sanitize_id(&kind), chrono::Utc::now().format("%H%M%S"));
            self.collections.insert(
                collection_id.clone(),
                TestCollection {
                    collection_id: collection_id.clone(),
                    name: format!("type::{kind}"),
                    source: CollectionSource::AutoType,
                    source_run_id: None,
                    scan_id: Some(snapshot.scan_id.clone()),
                    filter_expr: Some(format!("model_type={kind}")),
                    case_ids: set.into_iter().collect(),
                    frozen: false,
                    created_at: chrono::Utc::now().to_rfc3339(),
                },
            );
            ids.push(collection_id);
        }
        ids
    }
}

fn sanitize_id(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}
