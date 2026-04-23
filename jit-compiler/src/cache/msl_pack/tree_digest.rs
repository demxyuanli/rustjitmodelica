//! Content-addressed digest of all `.mo` files under an MSL root (sorted, stable).

use std::fs;
use std::io::Read;
use std::path::Path;

use xxhash_rust::xxh3::Xxh3;

fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut f = fs::File::open(path)?;
    let mut buf = [0u8; 64 * 1024];
    let mut h = Xxh3::new();
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:032x}", h.digest128()))
}

/// Lexicographic walk of `root` for `**/*.mo`, format `relpath\thash_hex\n`, then hash that listing.
pub fn compute_msl_tree_digest(root: &Path) -> std::io::Result<String> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    collect_mo_files(root, root, &mut pairs)?;
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut listing = String::new();
    for (rel, h) in &pairs {
        listing.push_str(rel);
        listing.push('\t');
        listing.push_str(h);
        listing.push('\n');
    }
    let mut h = Xxh3::new();
    h.update(listing.as_bytes());
    Ok(format!("{:032x}", h.digest128()))
}

fn collect_mo_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, String)>,
) -> std::io::Result<()> {
    for ent in fs::read_dir(dir)? {
        let ent = ent?;
        let p = ent.path();
        let meta = ent.metadata()?;
        if meta.is_dir() {
            collect_mo_files(root, &p, out)?;
        } else if p.extension().and_then(|e| e.to_str()) == Some("mo") {
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            let fh = hash_file(&p)?;
            out.push((rel, fh));
        }
    }
    Ok(())
}

pub fn short_digest(full: &str) -> String {
    full.chars().take(12).collect()
}
