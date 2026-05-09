use crate::ast::Declaration;
use crate::flatten::flat_cache_v1::DepHashEntry;
use crate::flatten::structures::FlattenedModel;
use rkyv::{Archive, Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const FLAT_CACHE_SCHEMA_V2: &str = "rustmodlica_flat_cache_v2";

/// Zero-copy cache format using rkyv.
/// Complex AST types are serialized via serde/bincode and stored as bytes,
/// allowing rkyv to archive the top-level structure without
/// requiring Archive implementations for all AST types.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
#[archive(check_bytes)]
pub struct FlatCacheV2 {
    pub schema: String,
    pub key: String,
    pub model_name: String,
    /// Serde-serialized declarations
    pub declarations_bytes: Option<Vec<u8>>,
    pub equations_bytes: Option<Vec<u8>>,
    pub algorithms_bytes: Option<Vec<u8>>,
    pub initial_equations_bytes: Option<Vec<u8>>,
    pub initial_algorithms_bytes: Option<Vec<u8>>,
    pub instances_bytes: Option<Vec<u8>>,
    pub connections_bytes: Option<Vec<u8>>,
    pub conditional_connections_bytes: Option<Vec<u8>>,
    pub array_sizes_bytes: Option<Vec<u8>>,
    pub clocked_var_names_bytes: Option<Vec<u8>>,
    pub clock_partitions_bytes: Option<Vec<u8>>,
    pub clock_signal_connections_bytes: Option<Vec<u8>>,
    pub stream_peer_map_bytes: Option<Vec<u8>>,
    pub stream_connection_set_bytes: Option<Vec<u8>>,
    pub expandable_instances_bytes: Option<Vec<u8>>,
    pub deps: Vec<DepHashEntry>,
}

impl FlatCacheV2 {
    pub fn schema(&self) -> &str {
        FLAT_CACHE_SCHEMA_V2
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn from_flat_model(key: String, model_name: &str, flat: &FlattenedModel, deps: Vec<DepHashEntry>) -> Self {
        Self {
            schema: FLAT_CACHE_SCHEMA_V2.to_string(),
            key,
            model_name: model_name.to_string(),
            declarations_bytes: bincode::serialize(&flat.declarations).ok(),
            equations_bytes: bincode::serialize(&flat.equations).ok(),
            algorithms_bytes: bincode::serialize(&flat.algorithms).ok(),
            initial_equations_bytes: bincode::serialize(&flat.initial_equations).ok(),
            initial_algorithms_bytes: bincode::serialize(&flat.initial_algorithms).ok(),
            instances_bytes: bincode::serialize(&flat.instances).ok(),
            connections_bytes: bincode::serialize(&flat.connections).ok(),
            conditional_connections_bytes: bincode::serialize(&flat.conditional_connections).ok(),
            array_sizes_bytes: bincode::serialize(&flat.array_sizes).ok(),
            clocked_var_names_bytes: bincode::serialize(&flat.clocked_var_names).ok(),
            clock_partitions_bytes: bincode::serialize(&flat.clock_partitions).ok(),
            clock_signal_connections_bytes: bincode::serialize(&flat.clock_signal_connections).ok(),
            stream_peer_map_bytes: bincode::serialize(&flat.stream_peer_map).ok(),
            stream_connection_set_bytes: bincode::serialize(&flat.stream_connection_set).ok(),
            expandable_instances_bytes: bincode::serialize(&flat.expandable_instances).ok(),
            deps,
        }
    }

    pub fn into_flat_model(&self) -> Result<FlattenedModel, Box<dyn std::error::Error>> {
        let declarations: Vec<Declaration> = self.deserialize_field(&self.declarations_bytes)?;
        let instances: HashMap<String, String> = self.deserialize_field(&self.instances_bytes)?;
        let clocked_var_names: HashSet<String> = self.deserialize_field(&self.clocked_var_names_bytes)?;
        let stream_peer_map: HashMap<String, String> = self.deserialize_field(&self.stream_peer_map_bytes)?;
        let stream_connection_set: HashMap<String, Vec<String>> =
            self.deserialize_field(&self.stream_connection_set_bytes)?;
        let expandable_instances: HashSet<String> =
            self.deserialize_field(&self.expandable_instances_bytes)
                .unwrap_or_default();

        let mut interner = crate::string_intern::StringInterner::new();
        for d in &declarations {
            interner.intern(d.name.as_str());
        }
        for k in instances.keys() {
            interner.intern(k.as_str());
        }
        for v in instances.values() {
            interner.intern(v.as_str());
        }
        for n in &clocked_var_names {
            interner.intern(n.as_str());
        }
        for (k, v) in &stream_peer_map {
            interner.intern(k.as_str());
            interner.intern(v.as_str());
        }
        for (k, peers) in &stream_connection_set {
            interner.intern(k.as_str());
            for p in peers {
                interner.intern(p.as_str());
            }
        }

        let mut flat = FlattenedModel {
            declarations,
            equations: self.deserialize_field(&self.equations_bytes)?,
            algorithms: self.deserialize_field(&self.algorithms_bytes)?,
            initial_equations: self.deserialize_field(&self.initial_equations_bytes)?,
            initial_algorithms: self.deserialize_field(&self.initial_algorithms_bytes)?,
            connections: self.deserialize_field(&self.connections_bytes)?,
            conditional_connections: self.deserialize_field(&self.conditional_connections_bytes)?,
            instances,
            array_sizes: self.deserialize_field(&self.array_sizes_bytes)?,
            clocked_var_names,
            clock_partitions: self.deserialize_field(&self.clock_partitions_bytes)?,
            clock_signal_connections: self.deserialize_field(&self.clock_signal_connections_bytes)?,
            stream_peer_map,
            stream_connection_set,
            stream_flow_map: HashMap::new(),
            expandable_instances,
            interner,
            inst_records: Vec::new(),
            path_to_inst: HashMap::new(),
        };
        crate::flatten::connections::rebuild_stream_flow_map(&mut flat);
        for (k, v) in &flat.stream_flow_map {
            flat.interner.intern(k.as_str());
            flat.interner.intern(v.as_str());
        }
        Ok(flat)
    }

    fn deserialize_field<T: serde::de::DeserializeOwned>(&self, bytes: &Option<Vec<u8>>) -> Result<T, Box<dyn std::error::Error>> {
        match bytes {
            Some(b) => bincode::deserialize(b).map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            None => Err("missing bytes".into()),
        }
    }
}

/// Default scratch space size for rkyv serialization (256KB)
const RKYV_SCRATCH_SIZE: usize = 256 * 1024;

/// Serialize cache using rkyv for zero-copy deserialization
pub fn serialize_cache(cache: &FlatCacheV2) -> Result<Vec<u8>, String> {
    let bytes = rkyv::to_bytes::<_, RKYV_SCRATCH_SIZE>(cache)
        .map_err(|e| format!("rkyv serialize error: {:?}", e))?;
    Ok(bytes.to_vec())
}

/// Deserialize cache with validation
pub fn deserialize_cache(bytes: &[u8]) -> Result<FlatCacheV2, Box<dyn std::error::Error>> {
    let archived = rkyv::check_archived_root::<FlatCacheV2>(bytes)
        .map_err(|e| format!("rkyv access error: {:?}", e))?;
    archived.deserialize(&mut rkyv::Infallible).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}
