//! Read `annotation(version="...")` from `Modelica/package.mo`.

use std::fs;
use std::path::Path;

pub fn read_msl_version_label(msl_root: &Path) -> Option<String> {
    let pkg = msl_root.join("Modelica").join("package.mo");
    let text = fs::read_to_string(&pkg).ok()?;
    extract_version_string(&text)
}

fn extract_version_string(text: &str) -> Option<String> {
    // Match: version="..." (possibly after spaces). Skip prose like "since version 3.0".
    let needle = "version";
    for line in text.lines() {
        let t = line.trim();
        if !t.contains(needle) {
            continue;
        }
        if let Some(pos) = t.find(needle) {
            let mut rest = t[pos + needle.len()..].trim_start();
            let Some(after_eq) = rest.strip_prefix('=') else {
                continue;
            };
            rest = after_eq.trim_start();
            let Some(after_q) = rest.strip_prefix('"') else {
                continue;
            };
            rest = after_q;
            let Some(end) = rest.find('"') else {
                continue;
            };
            let v = rest[..end].trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

pub fn sanitize_dir_component(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sample_annotation() {
        let mo = "  version=\"4.2.0 dev\",";
        assert_eq!(extract_version_string(mo).as_deref(), Some("4.2.0 dev"));
    }

    #[test]
    fn skips_prose_before_real_annotation() {
        let mo = "handled (since version 3.0).\nversion=\"4.0.0\",";
        assert_eq!(extract_version_string(mo).as_deref(), Some("4.0.0"));
    }
}
