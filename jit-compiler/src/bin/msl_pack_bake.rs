//! CLI: bake an MSL pre-pack directory (manifest + cache-std.sqlite).
//!
//! Usage:
//!   msl_pack_bake --msl <path-to-Modelica-root> --out <pack-dir> [--leaves leaves.toml] [--hot hotness.json]

use std::path::PathBuf;

fn usage() -> ! {
    eprintln!(
        "Usage: msl_pack_bake --msl <MSL_ROOT> --out <OUT_DIR> [--leaves leaves.toml] [--hot hotness.json]"
    );
    std::process::exit(2);
}

fn main() {
    let mut msl: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut leaves: Option<PathBuf> = None;
    let mut hot: Option<PathBuf> = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--msl" => msl = it.next().map(PathBuf::from),
            "--out" => out = it.next().map(PathBuf::from),
            "--leaves" => leaves = it.next().map(PathBuf::from),
            "--hot" => hot = it.next().map(PathBuf::from),
            "-h" | "--help" => usage(),
            _ => {
                eprintln!("unknown arg: {a}");
                usage();
            }
        }
    }
    let (Some(msl_root), Some(out_dir)) = (msl, out) else {
        usage();
    };
    let default_leaves = include_str!("../../msl-pack-bake/leaves-default.toml");
    let curated = if let Some(p) = leaves {
        rustmodlica::cache::msl_pack::leaves::load_leaves_path(&p).unwrap_or_else(|e| {
            eprintln!("leaves file: {e}");
            std::process::exit(1);
        })
    } else {
        rustmodlica::cache::msl_pack::leaves::parse_leaves_toml(default_leaves).unwrap_or_else(|e| {
            eprintln!("default leaves: {e}");
            std::process::exit(1);
        })
    };
    if let Err(e) = rustmodlica::cache::msl_pack::bake_msl_pack(
        &msl_root,
        &out_dir,
        &curated,
        hot.as_ref().map(|p| p.as_path()),
    ) {
        eprintln!("bake failed: {e}");
        std::process::exit(1);
    }
    eprintln!("OK -> {}", out_dir.display());
}
