//! Curated + hot leaf lists for flatten pre-bake.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct LeavesFile {
    pub leaves: Vec<String>,
}

pub fn parse_leaves_toml(text: &str) -> Result<LeavesFile, String> {
    #[derive(serde::Deserialize)]
    struct TomlTop {
        leaves: Vec<String>,
    }
    let t: TomlTop = toml::from_str(text).map_err(|e| e.to_string())?;
    Ok(LeavesFile { leaves: t.leaves })
}

pub fn load_leaves_path(path: &Path) -> Result<LeavesFile, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_leaves_toml(&text)
}

pub fn write_leaves_toml(path: &std::path::Path, leaves: &[String]) -> Result<(), String> {
    let f = LeavesFile {
        leaves: leaves.to_vec(),
    };
    let s = toml::to_string_pretty(&f).map_err(|e| e.to_string())?;
    std::fs::write(path, s).map_err(|e| e.to_string())
}

pub fn merge_leaves(curated: LeavesFile, hot: &[String]) -> Vec<String> {
    let mut s: BTreeSet<String> = BTreeSet::new();
    for x in curated.leaves {
        let t = x.trim().to_string();
        if !t.is_empty() {
            s.insert(t);
        }
    }
    for x in hot {
        let t = x.trim().to_string();
        if !t.is_empty() && t.starts_with("Modelica.") {
            s.insert(t);
        }
    }
    s.into_iter().collect()
}
