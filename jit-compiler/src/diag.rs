// Rust-style compile error reporting: file:line:col with snippet and caret.

use std::fmt;

/// Location in a source file (1-based line and column for display). DBG-4: attach to flatten/analysis/JIT errors.
#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    /// Format as suffix for error message (e.g. "\n  --> path:1:2" or "\n  --> path" when line is 0).
    pub fn fmt_suffix(&self) -> String {
        if self.line > 0 && self.column > 0 {
            format!("\n  --> {}:{}:{}", self.file, self.line, self.column)
        } else {
            format!("\n  --> {}", self.file)
        }
    }
}

impl Default for SourceLocation {
    fn default() -> Self {
        Self { file: String::new(), line: 0, column: 0 }
    }
}

/// Structured parse error for pretty-printing.
#[derive(Debug, Clone)]
pub struct ParseErrorInfo {
    pub path: String,
    pub source: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl fmt::Display for ParseErrorInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lines: Vec<&str> = self.source.lines().collect();
        let line_index = self.line.saturating_sub(1).min(lines.len().saturating_sub(1));
        let line_content = lines.get(line_index).unwrap_or(&"");

        let line_num_width = self.line.to_string().len().max(2);
        let gutter = " ".repeat(line_num_width);

        writeln!(f, "error: {}", self.message)?;
        writeln!(f, "  --> {}:{}:{}", self.path, self.line, self.column)?;
        writeln!(f, "{} |", gutter)?;
        writeln!(f, "{:>width$} | {}", self.line, line_content, width = line_num_width)?;

        let col = self.column.saturating_sub(1).min(line_content.len());
        let caret_width = (line_content.len().saturating_sub(col)).max(1);
        let spaces = " ".repeat(col);
        let carets = "^".repeat(caret_width);
        writeln!(f, "{} | {}{}", gutter, spaces, carets)
    }
}

impl ParseErrorInfo {
    /// Print error to stderr in Rust compiler style.
    #[allow(dead_code)]
    pub fn print(&self) {
        eprint!("{}", self);
    }
}

/// Format a pest parse error with path and source into Rust-style output.
#[allow(dead_code)]
pub fn format_parse_error(
    path: &str,
    source: &str,
    message: &str,
    line: usize,
    column: usize,
) {
    let info = ParseErrorInfo {
        path: path.to_string(),
        source: source.to_string(),
        line,
        column,
        message: message.to_string(),
    };
    info.print();
}

/// Extract (line, column) from pest's LineColLocation (1-based).
pub fn line_col_from_pest(line_col: &pest::error::LineColLocation) -> (usize, usize) {
    use pest::error::LineColLocation;
    match line_col {
        LineColLocation::Pos((l, c)) => (*l, *c),
        LineColLocation::Span((l, c), _) => (*l, *c),
    }
}

impl std::error::Error for ParseErrorInfo {}

/// A single compile warning with optional source location and snippet.
#[derive(Debug, Clone)]
pub struct WarningInfo {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub source: Option<String>,
}

impl fmt::Display for WarningInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "warning: {}", self.message)?;
        if self.line > 0 && self.column > 0 {
            write!(f, "  --> {}:{}:{}", self.path, self.line, self.column)?;
        } else {
            write!(f, "  --> {}", self.path)?;
        }
        writeln!(f)?;
        if let Some(ref source) = self.source {
            if self.line > 0 && self.column > 0 {
            let lines: Vec<&str> = source.lines().collect();
            let line_index = self.line.saturating_sub(1).min(lines.len().saturating_sub(1));
            let line_content = lines.get(line_index).unwrap_or(&"");
            let line_num_width = self.line.to_string().len().max(2);
            let gutter = " ".repeat(line_num_width);
            let col = self.column.saturating_sub(1).min(line_content.len());
            let caret_width = (line_content.len().saturating_sub(col)).max(1);
            let spaces = " ".repeat(col);
            let carets = "^".repeat(caret_width);
            writeln!(f, "{} |", gutter)?;
            writeln!(f, "{:>width$} | {}", self.line, line_content, width = line_num_width)?;
            writeln!(f, "{} | {}{}", gutter, spaces, carets)?;
            }
        }
        Ok(())
    }
}

/// Build a warning without source snippet (path:line:col only).
#[allow(dead_code)]
pub fn warning_at(path: &str, line: usize, column: usize, message: &str) -> WarningInfo {
    WarningInfo {
        path: path.to_string(),
        line,
        column,
        message: message.to_string(),
        source: None,
    }
}

/// Build a warning with source for snippet display.
#[allow(dead_code)]
pub fn warning_at_with_source(
    path: &str,
    source: &str,
    line: usize,
    column: usize,
    message: &str,
) -> WarningInfo {
    WarningInfo {
        path: path.to_string(),
        line,
        column,
        message: message.to_string(),
        source: Some(source.to_string()),
    }
}

/// Extract a one-line hint from pest error string (the line after "=").
/// Tries to replace grammar rule names with user-friendly hints.
pub fn short_message_from_pest_string(s: &str) -> String {
    let raw = s
        .lines()
        .find(|l| l.trim_start().starts_with('='))
        .map(|l| l.trim_start().trim_start_matches('=').trim().to_string())
        .unwrap_or_else(|| s.lines().next().unwrap_or(s).to_string());
    humanize_expected_message(&raw)
}

fn humanize_expected_message(raw: &str) -> String {
    if !raw.starts_with("expected ") {
        return raw.to_string();
    }
    let rest = raw.trim_start_matches("expected ").trim();
    if rest.contains("modification_part")
        && (rest.contains("value_assignment") || rest.contains("array_subscript"))
    {
        return "expected `;` or `=` or `[` after variable name (e.g. `Real x;` or `Real x = 1;`)"
            .to_string();
    }
    if rest.contains("value_assignment") && rest.contains(";") {
        return "expected `=` or `;` (e.g. `Real x = 1;`)".to_string();
    }
    let replaced = rest
        .replace("value_assignment", "`= expression`")
        .replace("modification_part", "`( ... )`")
        .replace("array_subscript", "`[ ... ]`")
        .replace("string_comment", "string or comment")
        .replace("equation_section", "`equation`")
        .replace("algorithm_section", "`algorithm`");
    format!("expected {}", replaced)
}

