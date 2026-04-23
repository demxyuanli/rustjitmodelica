use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::Path;

use xxhash_rust::xxh3::Xxh3;

pub const PACK_FORMAT_V1: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MslPackManifestV1 {
    pub pack_format: u32,
    pub msl_version: String,
    pub tree_digest: String,
    pub created_unix_ms: i64,
    pub cache_std_sqlite: PackFileEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackFileEntry {
    pub relative_path: String,
    pub xxh128_hex: String,
    pub size_bytes: u64,
}

pub fn hash_file_xxh128(path: &Path) -> std::io::Result<(String, u64)> {
    let mut f = fs::File::open(path)?;
    let mut buf = [0u8; 64 * 1024];
    let mut h = Xxh3::new();
    let mut len = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        len += n as u64;
        h.update(&buf[..n]);
    }
    Ok((format!("{:032x}", h.digest128()), len))
}

pub fn write_manifest(path: &Path, m: &MslPackManifestV1) -> Result<(), String> {
    let json = serde_json::to_string_pretty(m).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

pub fn read_manifest(path: &Path) -> Result<MslPackManifestV1, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

pub fn verify_entry(path: &Path, entry: &PackFileEntry) -> Result<(), String> {
    let (h, sz) = hash_file_xxh128(path).map_err(|e| e.to_string())?;
    if h != entry.xxh128_hex {
        return Err(format!(
            "hash mismatch for {}: expected {} got {}",
            path.display(),
            entry.xxh128_hex,
            h
        ));
    }
    if sz != entry.size_bytes {
        return Err(format!(
            "size mismatch for {}: expected {} got {}",
            path.display(),
            entry.size_bytes,
            sz
        ));
    }
    Ok(())
}
