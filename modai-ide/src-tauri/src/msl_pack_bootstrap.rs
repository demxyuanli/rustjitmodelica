use std::fs;
use std::io;
use std::path::Path;

use tauri::Manager;

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for e in fs::read_dir(src)? {
        let e = e?;
        let p = e.path();
        let leaf = e.file_name();
        if p.is_dir() {
            copy_dir_recursive(&p, &dst.join(&leaf))?;
        } else {
            fs::copy(&p, dst.join(&leaf))?;
        }
    }
    Ok(())
}

/// Install bundled MSL cache packs into app data and set env vars for rustmodlica.
pub fn init(app: &tauri::AppHandle) -> Result<(), String> {
    let data = crate::app_data::app_data_root()?.join("msl-cache");
    fs::create_dir_all(&data).map_err(|e| e.to_string())?;

    if let Ok(res) = app.path().resource_dir() {
        let bundled_root = res.join("msl-cache");
        if bundled_root.is_dir() {
            for entry in fs::read_dir(&bundled_root).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let p = entry.path();
                if p.is_dir() && p.join("manifest.json").is_file() {
                    let name = entry.file_name();
                    let dest = data.join(&name);
                    if !dest.exists() {
                        copy_dir_recursive(&p, &dest).map_err(|e| e.to_string())?;
                    }
                }
            }
        }
    }

    let mut pack_dirs: Vec<String> = Vec::new();
    pack_dirs.push(data.to_string_lossy().into_owned());
    if let Ok(res) = app.path().resource_dir() {
        let bundled_root = res.join("msl-cache");
        if bundled_root.is_dir() {
            pack_dirs.push(bundled_root.to_string_lossy().into_owned());
        }
    }
    std::env::set_var("RUSTMODLICA_MSL_PACK_DIRS", pack_dirs.join(";"));
    let hot = data.join("msl-hotness.json");
    std::env::set_var(
        "RUSTMODLICA_MSL_HOTNESS_JSON",
        hot.to_string_lossy().as_ref(),
    );
    Ok(())
}
