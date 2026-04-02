use crate::runtime::events::EventEnvelope;
use crate::runtime::state::{ExecutionSnapshot, ExecutionStateStore};
use anyhow::{bail, Context, Result};
use std::fs::File;
use std::collections::VecDeque;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub fn append_event_line(path: &Path, env: &EventEnvelope) -> Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;
    use std::io::Write;
    serde_json::to_writer(&mut f, env).context("serialize event")?;
    writeln!(&mut f).context("append newline")?;
    Ok(())
}

pub fn read_event_tail(path: &Path, n: usize) -> Result<Vec<EventEnvelope>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut out = Vec::new();
    for line in text.lines().filter(|x| !x.trim().is_empty()) {
        if let Ok(ev) = serde_json::from_str::<EventEnvelope>(line) {
            out.push(ev);
        }
    }
    let start = out.len().saturating_sub(n);
    Ok(out.into_iter().skip(start).collect())
}

pub fn follow_events(path: &Path, mut pos: u64) -> Result<()> {
    let poll = std::time::Duration::from_millis(250);
    loop {
        if !path.exists() {
            std::thread::sleep(poll);
            continue;
        }
        let mut f = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let len = f.metadata()?.len();
        if len < pos {
            pos = 0;
        }
        f.seek(SeekFrom::Start(pos))?;
        let mut chunk = String::new();
        f.read_to_string(&mut chunk)?;
        pos = f.stream_position()?;
        for line in chunk.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(ev) = serde_json::from_str::<EventEnvelope>(line) else {
                continue;
            };
            println!(
                "{} {} seq={} {}",
                ev.ts,
                ev.run_id,
                ev.seq,
                serde_json::to_string(&ev.payload).unwrap_or_else(|_| "{}".to_string())
            );
        }
        std::thread::sleep(poll);
    }
}

pub fn ensure_event_file(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, b"").with_context(|| format!("create {}", path.display()))?;
    if !path.exists() {
        bail!("failed to create {}", path.display());
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct RollingLogger {
    cap: usize,
    rows: VecDeque<String>,
}

impl RollingLogger {
    pub fn new(cap: usize) -> Self {
        Self {
            cap: cap.max(1),
            rows: VecDeque::new(),
        }
    }

    pub fn push(&mut self, line: String) {
        if self.rows.len() >= self.cap {
            self.rows.pop_front();
        }
        self.rows.push_back(line);
    }

    pub fn lines(&self) -> Vec<String> {
        self.rows.iter().cloned().collect()
    }
}

pub fn load_execution_snapshot_and_logs(
    path: &Path,
    tail: usize,
) -> Result<Option<(ExecutionSnapshot, Vec<String>)>> {
    if !path.exists() {
        return Ok(None);
    }
    let events = read_event_tail(path, tail)?;
    let Some(first) = events.first() else {
        return Ok(None);
    };
    let mut state = ExecutionStateStore::new(first.run_id.clone(), 0);
    let mut logs = RollingLogger::new(tail.max(8));
    for ev in events {
        state.apply_event(&ev.payload);
        logs.push(format!(
            "{} {} #{} {:?}",
            ev.ts, ev.run_id, ev.seq, ev.payload
        ));
    }
    Ok(Some((state.snapshot(), logs.lines())))
}

pub fn load_latest_run_id(data_root: &Path) -> Option<String> {
    let latest_path = data_root.join(".regress").join("runs").join("latest");
    if !latest_path.exists() {
        return None;
    }
    match std::fs::read_to_string(&latest_path) {
        Ok(content) => Some(content.trim().to_string()),
        Err(_) => None,
    }
}
