use std::io::Write;

pub fn write_csv_line(w: &mut dyn Write, line: &str) -> Result<(), String> {
    w.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    w.write_all(b"\n").map_err(|e| e.to_string())?;
    Ok(())
}

pub fn flush_writer(w: &mut dyn Write) -> Result<(), String> {
    w.flush().map_err(|e| e.to_string())
}
