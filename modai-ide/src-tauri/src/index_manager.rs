// Core indexing engine: scans project files, extracts symbols via AST parsing,
// builds chunks, and maintains the persistent SQLite index.

use crate::chunker;
use crate::index_db;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct CodeIndex {
    project_dir: String,
}

impl CodeIndex {
    pub fn new(project_dir: &str) -> Self {
        CodeIndex {
            project_dir: project_dir.to_string(),
        }
    }

    pub fn build_index(&self) -> Result<index_db::IndexStats, String> {
        self.build_index_inner(false, None::<&fn(usize, usize)>)
    }

    pub fn build_index_with_progress<F: Fn(usize, usize)>(
        &self,
        progress: F,
    ) -> Result<index_db::IndexStats, String> {
        self.build_index_inner(false, Some(&progress))
    }

    pub fn rebuild_index_with_progress<F: Fn(usize, usize)>(
        &self,
        progress: F,
    ) -> Result<index_db::IndexStats, String> {
        self.build_index_inner(true, Some(&progress))
    }

    fn build_index_inner<F: Fn(usize, usize)>(
        &self,
        force_rebuild: bool,
        progress: Option<&F>,
    ) -> Result<index_db::IndexStats, String> {
        let conn = index_db::open_connection(&self.project_dir)?;
        let base = Path::new(&self.project_dir);
        if !base.is_dir() {
            return Err("Project directory does not exist".to_string());
        }

        if force_rebuild {
            index_db::clear_all(&conn)?;
        }

        let existing = index_db::list_indexed_paths(&conn)?;
        let _existing_set: HashSet<String> = existing.iter().map(|(_, p, _)| p.clone()).collect();

        let ignore_patterns = load_ignore_patterns(base);
        let mut disk_files: Vec<PathBuf> = Vec::new();
        walk_source_files(base, base, &mut disk_files, &ignore_patterns);

        let disk_set: HashSet<String> = disk_files
            .iter()
            .filter_map(|p| p.strip_prefix(base).ok())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();

        for (id, path, _) in &existing {
            if !disk_set.contains(path.as_str()) {
                index_db::delete_file(&conn, *id)?;
            }
        }

        let total = disk_files.len();
        for (i, full_path) in disk_files.iter().enumerate() {
            let rel = full_path
                .strip_prefix(base)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            self.index_single_file(&conn, base, &rel, full_path)?;
            if let Some(cb) = progress {
                cb(i + 1, total);
            }
        }

        index_db::get_stats(&conn)
    }

    pub fn update_file(&self, file_path: &str) -> Result<(), String> {
        let conn = index_db::open_connection(&self.project_dir)?;
        let base = Path::new(&self.project_dir);
        let rel = file_path.replace('\\', "/");
        let full = base.join(&rel);

        if !full.exists() {
            index_db::delete_file_by_path(&conn, &rel)?;
            return Ok(());
        }

        self.index_single_file(&conn, base, &rel, &full)?;
        Ok(())
    }

    pub fn remove_file(&self, file_path: &str) -> Result<(), String> {
        let conn = index_db::open_connection(&self.project_dir)?;
        let rel = file_path.replace('\\', "/");
        index_db::delete_file_by_path(&conn, &rel)
    }

    pub fn search_symbols(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: i64,
    ) -> Result<Vec<index_db::SymbolInfo>, String> {
        let conn = index_db::open_connection(&self.project_dir)?;
        index_db::search_symbols(&conn, query, kind, limit)
    }

    pub fn file_symbols(&self, file_path: &str) -> Result<Vec<index_db::SymbolInfo>, String> {
        let conn = index_db::open_connection(&self.project_dir)?;
        let rel = file_path.replace('\\', "/");
        let file = index_db::get_file_by_path(&conn, &rel)?;
        match file {
            Some(f) => index_db::get_file_symbols(&conn, f.id),
            None => Ok(Vec::new()),
        }
    }

    pub fn find_references(&self, symbol_name: &str) -> Result<Vec<index_db::DependencyInfo>, String> {
        let conn = index_db::open_connection(&self.project_dir)?;
        index_db::find_references(&conn, symbol_name)
    }

    pub fn get_context(
        &self,
        query: &str,
        max_chunks: i64,
    ) -> Result<Vec<index_db::ChunkInfo>, String> {
        let conn = index_db::open_connection(&self.project_dir)?;
        index_db::search_chunks(&conn, query, max_chunks)
    }

    pub fn get_dependencies(
        &self,
        file_path: &str,
    ) -> Result<Vec<index_db::DependencyInfo>, String> {
        let conn = index_db::open_connection(&self.project_dir)?;
        let rel = file_path.replace('\\', "/");
        let file = index_db::get_file_by_path(&conn, &rel)?;
        match file {
            Some(f) => index_db::get_file_dependencies(&conn, f.id),
            None => Ok(Vec::new()),
        }
    }

    pub fn stats(&self) -> Result<index_db::IndexStats, String> {
        let conn = index_db::open_connection(&self.project_dir)?;
        index_db::get_stats(&conn)
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    fn index_single_file(
        &self,
        conn: &Connection,
        _base: &Path,
        rel_path: &str,
        full_path: &Path,
    ) -> Result<(), String> {
        let content = std::fs::read_to_string(full_path).map_err(|e| e.to_string())?;
        let hash = chunker::hex_sha256(&content);

        if let Some(existing) = index_db::get_file_by_path(conn, rel_path)? {
            if existing.content_hash == hash {
                return Ok(());
            }
        }

        let mtime = file_mtime(full_path);
        let lang = detect_language(rel_path);
        let size = content.len() as i64;
        let file_id = index_db::upsert_file(conn, rel_path, &hash, mtime, &lang, size)?;

        index_db::clear_file_symbols(conn, file_id)?;
        index_db::clear_file_chunks(conn, file_id)?;
        index_db::clear_file_dependencies(conn, file_id)?;

        if lang == "modelica" {
            self.index_modelica(conn, file_id, &content)?;
        } else {
            self.index_generic(conn, file_id, &content, &lang)?;
        }

        let ext = Path::new(rel_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let chunks = if lang == "modelica" {
            chunker::chunk_modelica(&content)
        } else {
            chunker::chunk_generic(&content, ext)
        };

        for ch in &chunks {
            index_db::insert_chunk(
                conn,
                file_id,
                ch.line_start as i64,
                ch.line_end as i64,
                &ch.content,
                ch.context_label.as_deref(),
                &ch.content_hash,
            )?;
        }

        Ok(())
    }

    fn index_modelica(
        &self,
        conn: &Connection,
        file_id: i64,
        content: &str,
    ) -> Result<(), String> {
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len() as i64;

        match rustmodlica::parser::parse(content) {
            Ok(item) => {
                let model = match item {
                    rustmodlica::ast::ClassItem::Model(m) => m,
                    rustmodlica::ast::ClassItem::Function(f) => rustmodlica::ast::Model::from(f),
                };
                self.extract_model_symbols(conn, file_id, &model, None, &lines)?;
            }
            Err(_) => {
                self.index_modelica_regex(conn, file_id, &lines, total_lines)?;
            }
        }

        Ok(())
    }

    fn extract_model_symbols(
        &self,
        conn: &Connection,
        file_id: i64,
        model: &rustmodlica::ast::Model,
        parent_id: Option<i64>,
        lines: &[&str],
    ) -> Result<(), String> {
        let kind = if model.is_function {
            "function"
        } else if model.is_connector {
            "connector"
        } else if model.is_record {
            "record"
        } else if model.is_block {
            "block"
        } else {
            "model"
        };

        let line_start = find_name_line(lines, &model.name).unwrap_or(1) as i64;
        let line_end = find_end_line(lines, &model.name, line_start as usize).unwrap_or(lines.len()) as i64;

        let sig = build_signature(kind, &model.name, &model.extends);
        let model_sym_id = index_db::insert_symbol(
            conn,
            file_id,
            &model.name,
            kind,
            line_start,
            line_end,
            parent_id,
            Some(&sig),
            None,
        )?;

        for ext in &model.extends {
            index_db::insert_dependency(
                conn,
                file_id,
                &ext.model_name,
                "extends",
                Some(line_start),
            )?;
        }

        for decl in &model.declarations {
            let dk = if decl.is_parameter {
                "parameter"
            } else {
                "variable"
            };
            let dl = find_name_line(lines, &decl.name).unwrap_or(line_start as usize) as i64;
            let dsig = format!("{} {}", decl.type_name, decl.name);
            index_db::insert_symbol(
                conn,
                file_id,
                &decl.name,
                dk,
                dl,
                dl,
                Some(model_sym_id),
                Some(&dsig),
                None,
            )?;

            let base_types = ["Real", "Integer", "Boolean", "String"];
            if !base_types.contains(&decl.type_name.as_str()) {
                index_db::insert_dependency(
                    conn,
                    file_id,
                    &decl.type_name,
                    "uses",
                    Some(dl),
                )?;
            }
        }

        for inner in &model.inner_classes {
            self.extract_model_symbols(conn, file_id, inner, Some(model_sym_id), lines)?;
        }

        for (alias_name, base_type) in &model.type_aliases {
            let al = find_name_line(lines, alias_name).unwrap_or(line_start as usize) as i64;
            index_db::insert_symbol(
                conn,
                file_id,
                alias_name,
                "type_alias",
                al,
                al,
                Some(model_sym_id),
                Some(&format!("type {} = {}", alias_name, base_type)),
                None,
            )?;
        }

        Ok(())
    }

    fn index_modelica_regex(
        &self,
        conn: &Connection,
        file_id: i64,
        lines: &[&str],
        _total_lines: i64,
    ) -> Result<(), String> {
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            for kw in &[
                "model",
                "function",
                "block",
                "connector",
                "record",
                "package",
                "class",
            ] {
                let prefix = format!("{} ", kw);
                if trimmed.starts_with(&prefix) {
                    let rest = &trimmed[prefix.len()..];
                    let name = rest
                        .split(|c: char| !c.is_alphanumeric() && c != '_')
                        .next()
                        .unwrap_or("")
                        .to_string();
                    if !name.is_empty() {
                        let ln = (i + 1) as i64;
                        index_db::insert_symbol(
                            conn,
                            file_id,
                            &name,
                            kw,
                            ln,
                            ln,
                            None,
                            Some(&format!("{} {}", kw, name)),
                            None,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn index_generic(
        &self,
        conn: &Connection,
        file_id: i64,
        content: &str,
        lang: &str,
    ) -> Result<(), String> {
        let lines: Vec<&str> = content.lines().collect();
        let patterns: Vec<(&str, &str)> = match lang {
            "rust" => vec![
                ("fn ", "function"),
                ("pub fn ", "function"),
                ("struct ", "struct"),
                ("enum ", "enum"),
                ("trait ", "trait"),
                ("impl ", "impl"),
                ("mod ", "module"),
            ],
            "typescript" | "javascript" => vec![
                ("function ", "function"),
                ("export function ", "function"),
                ("class ", "class"),
                ("interface ", "interface"),
                ("export default function ", "function"),
            ],
            _ => vec![],
        };

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            for (pat, kind) in &patterns {
                if trimmed.starts_with(pat) {
                    let rest = &trimmed[pat.len()..];
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '<')
                        .take_while(|c| *c != '<')
                        .collect();
                    if !name.is_empty() {
                        let ln = (i + 1) as i64;
                        let sig = trimmed.chars().take(80).collect::<String>();
                        index_db::insert_symbol(
                            conn, file_id, &name, kind, ln, ln, None, Some(&sig), None,
                        )?;
                    }
                    break;
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// File system helpers
// ---------------------------------------------------------------------------

fn load_ignore_patterns(base: &Path) -> Vec<String> {
    let mut patterns = Vec::new();
    for name in [".modaiignore", ".cursorignore"] {
        let p = base.join(name);
        if let Ok(s) = std::fs::read_to_string(&p) {
            for line in s.lines() {
                let t = line.trim().replace('\\', "/");
                if t.is_empty() || t.starts_with('#') {
                    continue;
                }
                patterns.push(t);
            }
        }
    }
    patterns
}

fn path_matches_ignore(rel_path: &str, patterns: &[String]) -> bool {
    let normalized = rel_path.replace('\\', "/");
    for p in patterns {
        let pat = p.trim_end_matches('/');
        if pat.is_empty() {
            continue;
        }
        let segment_match = normalized == pat
            || normalized.starts_with(&format!("{}/", pat))
            || normalized.ends_with(&format!("/{}", pat))
            || normalized.contains(&format!("/{}/", pat));
        if segment_match {
            return true;
        }
    }
    false
}

fn walk_source_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<PathBuf>,
    ignore_patterns: &[String],
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" || name == "target" || name == "build" {
            continue;
        }
        if path.is_dir() {
            walk_source_files(root, &path, out, ignore_patterns);
        } else if is_indexable_file(&name) {
            if let Ok(rel) = path.strip_prefix(root) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if path_matches_ignore(&rel_str, ignore_patterns) {
                    continue;
                }
            }
            out.push(path);
        }
    }
}

fn is_indexable_file(name: &str) -> bool {
    let exts = [
        ".mo", ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".c", ".h", ".cpp", ".hpp", ".toml",
        ".json", ".css", ".html",
    ];
    let lower = name.to_lowercase();
    exts.iter().any(|ext| lower.ends_with(ext))
}

fn detect_language(rel_path: &str) -> String {
    let lower = rel_path.to_lowercase();
    if lower.ends_with(".mo") {
        "modelica".to_string()
    } else if lower.ends_with(".rs") {
        "rust".to_string()
    } else if lower.ends_with(".ts") || lower.ends_with(".tsx") {
        "typescript".to_string()
    } else if lower.ends_with(".js") || lower.ends_with(".jsx") {
        "javascript".to_string()
    } else if lower.ends_with(".py") {
        "python".to_string()
    } else if lower.ends_with(".c") || lower.ends_with(".h") {
        "c".to_string()
    } else if lower.ends_with(".cpp") || lower.ends_with(".hpp") {
        "cpp".to_string()
    } else {
        "text".to_string()
    }
}

fn file_mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn find_name_line(lines: &[&str], name: &str) -> Option<usize> {
    for (i, line) in lines.iter().enumerate() {
        if line.contains(name) {
            return Some(i + 1);
        }
    }
    None
}

fn find_end_line(lines: &[&str], name: &str, start: usize) -> Option<usize> {
    let end_marker = format!("end {};", name);
    let end_marker2 = format!("end {}", name);
    for i in start..lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with(&end_marker) || trimmed.starts_with(&end_marker2) {
            return Some(i + 1);
        }
    }
    None
}

fn build_signature(kind: &str, name: &str, extends: &[rustmodlica::ast::ExtendsClause]) -> String {
    let mut s = format!("{} {}", kind, name);
    if !extends.is_empty() {
        let ext_names: Vec<&str> = extends.iter().map(|e| e.model_name.as_str()).collect();
        s.push_str(&format!(" extends {}", ext_names.join(", ")));
    }
    s
}
