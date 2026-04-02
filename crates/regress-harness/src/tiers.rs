//! Tier and tag resolution.

use crate::config::{CaseDef, HarnessConfig, TierSpec};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Filter {
    pub tier: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Resolve which case IDs to run from config + CLI filter.
pub fn resolve_cases(cfg: &HarnessConfig, filter: &Filter) -> Result<Vec<CaseDef>, String> {
    let mut ids: HashSet<String> = HashSet::new();

    if let Some(tier_name) = &filter.tier {
        let tier_set = resolve_tier_case_ids(cfg, tier_name)?;
        ids = tier_set;
    } else {
        for c in &cfg.cases {
            ids.insert(c.id.clone());
        }
    }

    if let Some(tag_list) = &filter.tags {
        if tag_list.is_empty() {
            return Err("empty --tags list".to_string());
        }
        let want: HashSet<String> = tag_list.iter().cloned().collect();
        ids.retain(|id| {
            cfg.cases
                .iter()
                .find(|c| &c.id == id)
                .map(|c| c.tags.iter().any(|t| want.contains(t)))
                .unwrap_or(false)
        });
    }

    let mut out: Vec<CaseDef> = cfg
        .cases
        .iter()
        .filter(|c| ids.contains(&c.id))
        .cloned()
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

fn resolve_tier_case_ids(cfg: &HarnessConfig, tier_name: &str) -> Result<HashSet<String>, String> {
    let spec = cfg
        .tiers
        .get(tier_name)
        .ok_or_else(|| format!("unknown tier: {tier_name}"))?;
    let mut acc: HashSet<String> = HashSet::new();
    collect_tier(cfg, tier_name, spec, &mut acc, &mut HashSet::new())?;
    Ok(acc)
}

fn collect_tier(
    cfg: &HarnessConfig,
    tier_name: &str,
    spec: &TierSpec,
    acc: &mut HashSet<String>,
    visiting: &mut HashSet<String>,
) -> Result<(), String> {
    if visiting.contains(tier_name) {
        return Err(format!("tier cycle involving {tier_name}"));
    }
    visiting.insert(tier_name.to_string());

    if let Some(parent) = &spec.extends {
        let parent_spec = cfg
            .tiers
            .get(parent.as_str())
            .ok_or_else(|| format!("unknown tier extends: {parent}"))?;
        collect_tier(cfg, parent, parent_spec, acc, visiting)?;
    }

    for tid in &spec.case_ids {
        acc.insert(tid.clone());
    }

    for c in &cfg.cases {
        if spec.include_tags.iter().any(|t| c.tags.contains(t)) {
            acc.insert(c.id.clone());
        }
    }

    for pat in &spec.include_globs {
        let Ok(p) = glob::Pattern::new(pat) else {
            return Err(format!("invalid glob pattern: {pat}"));
        };
        for c in &cfg.cases {
            if p.matches_path(std::path::Path::new(&c.id)) {
                acc.insert(c.id.clone());
            }
        }
    }

    visiting.remove(tier_name);
    Ok(())
}
