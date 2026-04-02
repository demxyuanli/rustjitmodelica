//! Last-row CSV comparison (aligned with `compare_omc.ps1`).

use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CsvCompareOutcome {
    pub max_abs_diff: f64,
    pub max_column_index: i32,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum CsvCompareError {
    #[error("IO: {0}")]
    Io(String),
    #[error("empty or missing CSV")]
    Empty,
}

/// Compare last data row of two CSVs: skip column 0 (time), max abs diff on remaining numeric columns.
/// Matches PowerShell loop `for ($j = 1; $j -lt $n; $j++)`.
pub fn compare_csv_last_row_max_abs_diff(
    rust_csv: &Path,
    reference_csv: &Path,
) -> Result<CsvCompareOutcome, CsvCompareError> {
    let rust_lines = read_lines(rust_csv)?;
    let ref_lines = read_lines(reference_csv)?;
    if rust_lines.len() < 2 || ref_lines.len() < 2 {
        return Ok(CsvCompareOutcome {
            max_abs_diff: 0.0,
            max_column_index: -1,
        });
    }
    let rust_last = split_csv_line(rust_lines.last().unwrap());
    let ref_last = split_csv_line(ref_lines.last().unwrap());
    let n = rust_last.len().min(ref_last.len());
    if n <= 1 {
        return Ok(CsvCompareOutcome {
            max_abs_diff: 0.0,
            max_column_index: -1,
        });
    }
    let mut max_diff = 0.0_f64;
    let mut max_idx: i32 = -1;
    for j in 1..n {
        let a = rust_last[j].trim().parse::<f64>().unwrap_or(0.0);
        let b = ref_last[j].trim().parse::<f64>().unwrap_or(0.0);
        let diff = (a - b).abs();
        if diff > max_diff {
            max_diff = diff;
            max_idx = j as i32;
        }
    }
    Ok(CsvCompareOutcome {
        max_abs_diff: max_diff,
        max_column_index: max_idx,
    })
}

fn read_lines(path: &Path) -> Result<Vec<String>, CsvCompareError> {
    let s = fs::read_to_string(path).map_err(|e| CsvCompareError::Io(e.to_string()))?;
    Ok(s.lines().map(|l| l.to_string()).collect())
}

fn split_csv_line(line: &str) -> Vec<String> {
    line.split(',').map(|s| s.trim().to_string()).collect()
}
