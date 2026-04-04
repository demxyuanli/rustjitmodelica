#![allow(dead_code)]

//! Lightweight terminal UI helpers inspired by framed CLI tools.

use crate::i18n::tr;
use std::io::IsTerminal;
use unicode_width::UnicodeWidthStr;

fn use_color() -> bool {
    std::io::stdout().is_terminal() && std::env::var("NO_COLOR").is_err()
}

/// Whether styled terminal output (prompts, banners) should use ANSI colors.
pub fn terminal_styles_enabled() -> bool {
    use_color()
}

fn style(s: &str, code: &str) -> String {
    if use_color() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn color_block(code: &str) -> String {
    if use_color() {
        format!("\x1b[{code}m  \x1b[0m")
    } else {
        "[]".to_string()
    }
}

fn term_width() -> usize {
    if let Ok(v) = std::env::var("COLUMNS") {
        if let Ok(n) = v.trim().parse::<usize>() {
            return n.clamp(72, 140);
        }
    }
    96
}

fn trunc(s: &str, max: usize) -> String {
    truncate_display(s, max)
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(ch) = it.next() {
        if ch == '\u{1b}' && it.peek().copied() == Some('[') {
            let _ = it.next();
            for c in it.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}

pub fn visible_width(s: &str) -> usize {
    UnicodeWidthStr::width(strip_ansi(s).as_str())
}

pub fn truncate_display(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if visible_width(s) <= max_width {
        return s.to_string();
    }
    let mut out = String::new();
    let mut width = 0usize;
    for ch in s.chars() {
        let ch_s = ch.to_string();
        let w = UnicodeWidthStr::width(ch_s.as_str());
        if width + w + 1 > max_width {
            break;
        }
        out.push(ch);
        width += w;
    }
    out.push('…');
    out
}

pub fn pad_display(s: &str, target_width: usize) -> String {
    let w = visible_width(s);
    if w >= target_width {
        return s.to_string();
    }
    format!("{s}{}", " ".repeat(target_width - w))
}

pub fn clear_screen() {
    print!("\x1b[2J\x1b[H");
}

pub fn print_session_intro(version: &str) {
    if !terminal_styles_enabled() {
        println!("regress-harness {version}");
        println!("Interactive mode. Type a command, or /help for topics. Ctrl+C exits.");
        return;
    }
    println!(
        "{}",
        style(
            &format!("regress-harness {version} · regression runner"),
            "1;37"
        )
    );
    println!(
        "{}",
        style(
            "Interactive session · type freely, or /help for commands · Tab completes",
            "90"
        )
    );
}

pub fn print_banner(title: &str) {
    let width = term_width();
    let line = "═".repeat(width);
    println!("{}", style(&line, "36"));
    let logo_lines = [
        "     _ ___ _____   _   _   _   ____  _   _ _____ ____ ____  ",
        "    | |_ _|_   _| | | | | / \\ |  _ \\| \\ | | ____/ ___/ ___| ",
        " _  | || |  | |   | |_| |/ _ \\| |_) |  \\| |  _| \\___ \\___ \\ ",
        "| |_| || |  | |   |  _  / ___ \\  _ <| |\\  | |___ ___) |__) |",
        " \\___/|___| |_|   |_| |_/_/   \\_\\_| \\_\\_| \\_|_____|____/____/ ",
        "                         J I T   H A R N E S S                 ",
    ];
    for l in logo_lines {
        println!("  {}", style(&trunc(l, width.saturating_sub(4)), "1;35"));
    }
    println!("  {}", style(&trunc(title, width.saturating_sub(4)), "1;37"));
    println!("{}", style(&line, "36"));
}

pub fn print_section(title: &str) {
    println!();
    println!("{} {}", style("▶", "33"), style(title, "1;37"));
}

pub fn print_ok(msg: &str) {
    println!("{} {msg}", style("[OK]", "32"));
}

pub fn print_warn(msg: &str) {
    println!("{} {msg}", style("[WARN]", "33"));
}

#[allow(dead_code)]
pub fn print_kv_block(title: &str, rows: &[(&str, String)]) {
    let width = term_width().min(96);
    let inner = width.saturating_sub(2);
    let line = "─".repeat(inner);
    println!("{}", style(&format!("┌{line}"), "90"));
    println!("{}", style(&format!("│ {}", trunc(title, inner.saturating_sub(1))), "1;36"));
    println!("{}", style(&format!("├{line}"), "90"));
    let key_w = (inner / 3).max(12).min(28);
    let val_w = inner.saturating_sub(key_w + 4);
    for (k, v) in rows {
        let k2 = trunc(k, key_w);
        let v2 = trunc(v, val_w);
        println!(
            "{} {} │ {}",
            style("│", "90"),
            pad_display(&style(&k2, "37"), key_w),
            pad_display(&v2, val_w)
        );
    }
    println!("{}", style(&format!("└{line}"), "90"));
}

fn style_tip_item(item: &str) -> String {
    let mut out = String::new();
    let mut in_hotkey = false;
    let mut hot = String::new();
    for ch in item.chars() {
        if ch == '[' {
            if !hot.is_empty() {
                out.push_str(&style(&hot, "90"));
                hot.clear();
            }
            in_hotkey = true;
            hot.push(ch);
            continue;
        }
        if in_hotkey {
            hot.push(ch);
            if ch == ']' {
                out.push_str(&style(&hot, "2;36"));
                hot.clear();
                in_hotkey = false;
            }
            continue;
        }
        hot.push(ch);
    }
    if !hot.is_empty() {
        let code = if in_hotkey { "2;36" } else { "90" };
        out.push_str(&style(&hot, code));
    }
    out
}

pub fn format_tips_items(items: &[&str], wrap_width: usize) -> Vec<String> {
    if items.is_empty() {
        return vec![];
    }
    let mut lines = Vec::new();
    let mut line = String::new();
    for raw in items {
        let item = raw.trim();
        if item.is_empty() {
            continue;
        }
        let formatted = style_tip_item(item);
        let seg = format!("{} {}", style("•", "90"), formatted);
        let next = if line.is_empty() {
            seg.clone()
        } else {
            format!("{line}  {seg}")
        };
        if visible_width(&next) > wrap_width.max(24) {
            if !line.is_empty() {
                lines.push(line.clone());
            }
            line = seg;
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

pub fn print_divider(title: Option<&str>) {
    let width = term_width();
    let line = "─".repeat(width);
    match title {
        Some(t) if !t.is_empty() => {
            println!("{}", style(&format!("{} {} {}", "─".repeat(4), trunc(t, width.saturating_sub(10)), "─".repeat(4)), "90"));
            println!("{}", style(&line, "90"));
        }
        _ => println!("{}", style(&line, "90")),
    }
}

pub fn print_status_table(rows: &[(&str, &str, &str)]) {
    let width = term_width();
    let inner = width.saturating_sub(2);
    let line = "─".repeat(inner);
    let item_w = (inner * 32 / 100).clamp(18, 36);
    let status_w = (inner * 18 / 100).clamp(10, 18);
    let note_w = inner.saturating_sub(item_w + status_w + 6);
    println!("{}", style(&format!("┌{line}"), "90"));
    println!(
        "{} {} │ {} │ {}",
        style("│", "90"),
        pad_display(&style("item", "1;36"), item_w),
        pad_display(&style("status", "1;36"), status_w),
        pad_display(&style("note", "1;36"), note_w)
    );
    println!("{}", style(&format!("├{line}"), "90"));
    for (item, status, note) in rows {
        let i2 = trunc(item, item_w);
        let s2 = trunc(status, status_w);
        let mut note_lines: Vec<String> = note
            .lines()
            .filter(|x| !x.trim().is_empty())
            .map(|x| trunc(x, note_w))
            .collect();
        if note_lines.is_empty() {
            note_lines.push("-".to_string());
        }
        for (idx, n2) in note_lines.iter().enumerate() {
            let item_cell = if idx == 0 { i2.as_str() } else { "" };
            let status_cell = if idx == 0 { s2.as_str() } else { "" };
            println!(
                "{} {} │ {} │ {}",
                style("│", "90"),
                pad_display(item_cell, item_w),
                pad_display(status_cell, status_w),
                pad_display(n2, note_w)
            );
        }
    }
    println!("{}", style(&format!("└{line}"), "90"));
}

pub fn print_tree(title: &str, rows: &[(&str, String)]) {
    println!("{}", style(title, "1;36"));
    for (i, (k, v)) in rows.iter().enumerate() {
        let branch = if i + 1 == rows.len() { "└─" } else { "├─" };
        println!("{} {}: {}", style(branch, "90"), style(k, "37"), v);
    }
}

pub fn print_color_legend() {
    println!(
        "{} {}  {} {}  {} {}  {} {}",
        color_block("42"),
        style("pass", "32"),
        color_block("41"),
        style("fail", "31"),
        color_block("43"),
        style("warn", "33"),
        color_block("44"),
        style("info", "34")
    );
}

pub fn make_action_tree(
    title: &str,
    actions: &[String],
    focused_idx: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(style(title, "1;36"));
    for (i, act) in actions.iter().enumerate() {
        let branch = if i + 1 == actions.len() { "└─" } else { "├─" };
        if i == focused_idx {
            lines.push(format!(
                "{} {} {}",
                style(branch, "33"),
                style("▶", "1;33"),
                style(act, "1;37")
            ));
        } else {
            lines.push(format!("{}   {}", style(branch, "90"), act));
        }
    }
    lines
}

pub fn make_tree_lines(title: &str, rows: &[(&str, String)]) -> Vec<String> {
    let mut out = Vec::new();
    out.push(style(title, "1;36"));
    for (i, (k, v)) in rows.iter().enumerate() {
        let branch = if i + 1 == rows.len() { "└─" } else { "├─" };
        out.push(format!("{} {}: {}", style(branch, "90"), style(k, "37"), v));
    }
    out
}

pub fn print_two_column(left: &[String], right: &[String]) {
    let width = term_width();
    let sep = " │ ";
    let left_w = (width * 36 / 100).clamp(22, 48);
    let right_w = width.saturating_sub(left_w + sep.len());
    let rows = left.len().max(right.len());
    for i in 0..rows {
        let l = left.get(i).cloned().unwrap_or_default();
        let r = right.get(i).cloned().unwrap_or_default();
        println!(
            "{}{}{}",
            pad_display(&trunc(&l, left_w), left_w),
            style(sep, "90"),
            pad_display(&trunc(&r, right_w), right_w)
        );
    }
}

pub fn run_done_line(passed: usize, failed: usize, skipped: usize) -> String {
    format!(
        "{}: passed={} failed={} skipped={}",
        tr("run_done"),
        passed,
        failed,
        skipped
    )
}
