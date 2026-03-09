// Semantic code chunker: splits source files into AI-friendly chunks at
// meaningful boundaries (model/function/class definitions).

use sha2::{Digest, Sha256};

const TARGET_MIN_LINES: usize = 15;
const TARGET_MAX_LINES: usize = 80;
const OVERLAP_LINES: usize = 3;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub line_start: usize,
    pub line_end: usize,
    pub content: String,
    pub context_label: Option<String>,
    pub content_hash: String,
}

pub fn chunk_modelica(source: &str) -> Vec<Chunk> {
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let boundaries = find_boundaries(&lines);

    if boundaries.is_empty() {
        return vec![make_chunk(&lines, 0, lines.len(), None)];
    }

    let mut chunks = Vec::new();
    let mut prev_end: usize = 0;

    for &(boundary_line, ref _label) in &boundaries {
        if boundary_line > prev_end && boundary_line - prev_end >= TARGET_MIN_LINES {
            let start = if prev_end > 0 {
                prev_end.saturating_sub(OVERLAP_LINES)
            } else {
                0
            };
            chunks.push(make_chunk(&lines, start, boundary_line, None));
            prev_end = boundary_line;
        }
    }

    let last_boundary = boundaries.last().map(|(l, _)| *l).unwrap_or(0);
    if last_boundary < lines.len() {
        let start = if last_boundary > 0 {
            last_boundary.saturating_sub(OVERLAP_LINES)
        } else {
            0
        };
        let label = boundaries.last().map(|(_, l)| l.clone());
        chunks.push(make_chunk(&lines, start, lines.len(), label));
    }

    if chunks.is_empty() {
        chunks.push(make_chunk(&lines, 0, lines.len(), None));
    }

    split_oversized(chunks, &lines)
}

pub fn chunk_generic(source: &str, file_ext: &str) -> Vec<Chunk> {
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let boundaries = find_generic_boundaries(&lines, file_ext);

    if boundaries.is_empty() || lines.len() <= TARGET_MAX_LINES {
        return vec![make_chunk(&lines, 0, lines.len(), None)];
    }

    let mut chunks = Vec::new();
    let mut prev_end: usize = 0;

    for &(boundary_line, ref _label) in &boundaries {
        if boundary_line > prev_end && boundary_line - prev_end >= TARGET_MIN_LINES {
            let start = if prev_end > 0 {
                prev_end.saturating_sub(OVERLAP_LINES)
            } else {
                0
            };
            chunks.push(make_chunk(&lines, start, boundary_line, None));
            prev_end = boundary_line;
        }
    }

    if prev_end < lines.len() {
        let start = if prev_end > 0 {
            prev_end.saturating_sub(OVERLAP_LINES)
        } else {
            0
        };
        chunks.push(make_chunk(&lines, start, lines.len(), None));
    }

    if chunks.is_empty() {
        chunks.push(make_chunk(&lines, 0, lines.len(), None));
    }

    split_oversized(chunks, &lines)
}

fn find_boundaries(lines: &[&str]) -> Vec<(usize, String)> {
    let mut boundaries = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("model ")
            || trimmed.starts_with("function ")
            || trimmed.starts_with("block ")
            || trimmed.starts_with("connector ")
            || trimmed.starts_with("record ")
            || trimmed.starts_with("package ")
            || trimmed.starts_with("class ")
        {
            let label = trimmed
                .split_whitespace()
                .take(2)
                .collect::<Vec<_>>()
                .join(" ");
            boundaries.push((i, label));
        } else if trimmed.starts_with("equation") || trimmed.starts_with("algorithm") {
            let ctx = find_enclosing_name(lines, i);
            let label = if let Some(ref name) = ctx {
                format!("{} > {}", name, trimmed.split_whitespace().next().unwrap_or(""))
            } else {
                trimmed.split_whitespace().next().unwrap_or("").to_string()
            };
            boundaries.push((i, label));
        } else if trimmed.starts_with("end ") && trimmed.ends_with(';') {
            if i + 1 < lines.len() {
                boundaries.push((i + 1, "end-block".to_string()));
            }
        }
    }
    boundaries
}

fn find_generic_boundaries(lines: &[&str], ext: &str) -> Vec<(usize, String)> {
    let mut boundaries = Vec::new();
    let patterns: &[&str] = match ext {
        "rs" => &["fn ", "pub fn ", "impl ", "struct ", "enum ", "trait ", "mod "],
        "ts" | "tsx" | "js" | "jsx" => &[
            "function ",
            "export function ",
            "export default ",
            "class ",
            "interface ",
            "const ",
            "export const ",
        ],
        "py" => &["def ", "class ", "async def "],
        _ => &[],
    };

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        for pat in patterns {
            if trimmed.starts_with(pat) {
                let label = trimmed.chars().take(60).collect::<String>();
                boundaries.push((i, label));
                break;
            }
        }
    }
    boundaries
}

fn find_enclosing_name(lines: &[&str], idx: usize) -> Option<String> {
    for i in (0..idx).rev() {
        let trimmed = lines[i].trim();
        for kw in &["model", "function", "block", "connector", "record", "class"] {
            if trimmed.starts_with(kw) {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    return Some(format!("{} {}", parts[0], parts[1]));
                }
            }
        }
    }
    None
}

fn make_chunk(lines: &[&str], start: usize, end: usize, label: Option<String>) -> Chunk {
    let end = end.min(lines.len());
    let content = lines[start..end].join("\n");
    let hash = hex_sha256(&content);
    Chunk {
        line_start: start + 1,
        line_end: end,
        content,
        context_label: label,
        content_hash: hash,
    }
}

fn split_oversized(chunks: Vec<Chunk>, lines: &[&str]) -> Vec<Chunk> {
    let mut result = Vec::new();
    for ch in chunks {
        let chunk_lines = ch.line_end - ch.line_start + 1;
        if chunk_lines <= TARGET_MAX_LINES {
            result.push(ch);
        } else {
            let start = ch.line_start - 1;
            let end = ch.line_end;
            let mut pos = start;
            while pos < end {
                let sub_end = (pos + TARGET_MAX_LINES).min(end);
                let sub_start = if pos > start {
                    pos.saturating_sub(OVERLAP_LINES)
                } else {
                    pos
                };
                result.push(make_chunk(lines, sub_start, sub_end, ch.context_label.clone()));
                pos = sub_end;
            }
        }
    }
    result
}

pub fn hex_sha256(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    format!("{:x}", hasher.finalize())
}
