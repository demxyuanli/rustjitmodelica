use std::collections::HashSet;

use crate::flatten::FlattenedModel;

use super::keys::{equation_hash, variable_key, VarKey};

#[derive(Debug, Clone, Default)]
pub struct FlatSnapshot {
    pub equation_hashes: Vec<u64>,
    pub declaration_names: HashSet<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DirtySet {
    pub changed_eqs: HashSet<usize>,
    pub changed_vars: HashSet<VarKey>,
}

pub fn snapshot_flat(flat: &FlattenedModel) -> FlatSnapshot {
    FlatSnapshot {
        equation_hashes: flat.equations.iter().map(equation_hash).collect(),
        declaration_names: flat.declarations.iter().map(|d| d.name.clone()).collect(),
    }
}

pub fn diff_flat_models(prev: &FlatSnapshot, next: &FlatSnapshot) -> DirtySet {
    let mut dirty = DirtySet::default();
    let max_len = prev.equation_hashes.len().max(next.equation_hashes.len());
    for i in 0..max_len {
        let a = prev.equation_hashes.get(i).copied();
        let b = next.equation_hashes.get(i).copied();
        if a != b {
            dirty.changed_eqs.insert(i);
        }
    }

    for name in prev
        .declaration_names
        .symmetric_difference(&next.declaration_names)
    {
        dirty.changed_vars.insert(variable_key(name));
    }
    dirty
}

