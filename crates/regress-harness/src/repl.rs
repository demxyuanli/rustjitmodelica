use anyhow::{bail, Result};
use inquire::ui::{Color, RenderConfig, StyleSheet, Styled};
use inquire::{CustomUserError, Select, Text};
use std::path::{Path, PathBuf};

use crate::commands::{
    cmd_agent_context, cmd_list_cases, cmd_monitor, cmd_plan, cmd_status, cmd_validate_config,
    parse_monitor_source, OutputFormat,
};
use crate::i18n::tr;

fn discover_repo_root() -> Result<PathBuf> {
    let mut dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|x| x.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    for _ in 0..12 {
        if dir.join(".git").is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    Ok(std::env::current_dir()?)
}

fn absolutize_under(root: &PathBuf, p: PathBuf) -> PathBuf {
    if p.is_absolute() {
        p
    } else {
        root.join(p)
    }
}

fn ctx_repo_root(ctx: &ReplContext) -> Result<PathBuf> {
    if let Some(r) = &ctx.repo_root {
        Ok(r.clone())
    } else {
        discover_repo_root()
    }
}

fn canonicalize_best_effort(p: PathBuf) -> PathBuf {
    std::fs::canonicalize(&p).unwrap_or(p)
}

fn split_path_input(input: &str) -> (String, String, char) {
    let s = input.trim();
    let last_slash = s.rfind('/');
    let last_backslash = s.rfind('\\');
    let (idx, sep) = match (last_slash, last_backslash) {
        (Some(a), Some(b)) => {
            if a > b {
                (Some(a), '/')
            } else {
                (Some(b), '\\')
            }
        }
        (Some(a), None) => (Some(a), '/'),
        (None, Some(b)) => (Some(b), '\\'),
        (None, None) => (None, std::path::MAIN_SEPARATOR),
    };
    if let Some(i) = idx {
        (s[..=i].to_string(), s[i + 1..].to_string(), sep)
    } else {
        ("".to_string(), s.to_string(), sep)
    }
}

fn list_windows_drives() -> Vec<String> {
    let mut out = Vec::new();
    for c in b'A'..=b'Z' {
        let root = format!("{}:\\", c as char);
        if Path::new(&root).is_dir() {
            out.push(root);
        }
    }
    out
}

fn path_suggestions_from_base(base_dir: &Path, input: &str) -> Result<Vec<String>, CustomUserError> {
    let (dir_prefix, name_prefix, sep) = split_path_input(input);
    let mut suggestions = Vec::new();

    if dir_prefix.is_empty() && name_prefix.len() <= 2 && name_prefix.ends_with(':') {
        for d in list_windows_drives() {
            if d.to_ascii_lowercase().starts_with(&name_prefix.to_ascii_lowercase()) {
                suggestions.push(d);
            }
        }
        return Ok(suggestions);
    }

    let dir = if dir_prefix.is_empty() {
        base_dir.to_path_buf()
    } else {
        let p = PathBuf::from(&dir_prefix);
        if p.is_absolute() {
            p
        } else {
            base_dir.join(p)
        }
    };

    let rd = match std::fs::read_dir(&dir) {
        Ok(v) => v,
        Err(_) => return Ok(vec![]),
    };
    let needle = name_prefix.to_ascii_lowercase();
    for e in rd.flatten() {
        let name_os = e.file_name();
        let name = name_os.to_string_lossy().to_string();
        if !needle.is_empty() && !name.to_ascii_lowercase().starts_with(&needle) {
            continue;
        }
        let mut s = format!("{dir_prefix}{name}");
        if e.path().is_dir() {
            s.push(sep);
        }
        suggestions.push(s);
    }
    suggestions.sort();
    Ok(suggestions)
}

fn prompt_path(label: &str, base_dir: PathBuf) -> Result<String> {
    Text::new(label)
        .with_autocomplete(move |input: &str| path_suggestions_from_base(&base_dir, input))
        .with_help_message("Type a path. Use Tab to autocomplete.")
        .prompt()
        .map_err(|e| anyhow::anyhow!(e))
}

#[derive(Debug, Clone)]
struct ReplContext {
    repo_root: Option<PathBuf>,
    cmd_prefix: Vec<String>,
    rustmodlica_exe: Option<PathBuf>,
    cargo_target_dir: Option<PathBuf>,
    config: Option<PathBuf>,
    tier: Option<String>,
    tags: Option<Vec<String>>,
    workers: Option<usize>,
    baseline: Option<PathBuf>,
    incremental: Option<String>,
    manifest: Option<PathBuf>,
    data_root: PathBuf,
    out_dir: Option<PathBuf>,
    format: OutputFormat,
    ndjson: bool,
    summary_compat: bool,
    progress: bool,
}

impl Default for ReplContext {
    fn default() -> Self {
        Self {
            repo_root: None,
            cmd_prefix: Vec::new(),
            rustmodlica_exe: None,
            cargo_target_dir: None,
            config: None,
            tier: None,
            tags: None,
            workers: None,
            baseline: None,
            incremental: None,
            manifest: None,
            data_root: PathBuf::from("build/regression_data"),
            out_dir: None,
            format: OutputFormat::Human,
            ndjson: false,
            summary_compat: false,
            progress: false,
        }
    }
}

fn command_suggestions(input: &str) -> Result<Vec<String>, CustomUserError> {
    let prefix = input.trim().to_ascii_lowercase();
    let cmds = [
        "help",
        "/help",
        "scope",
        "scope list",
        "scope use",
        "scope gen-config",
        "sync determinism",
        "sync trace-assert",
        "fmi emit-fmu",
        "fmi validate",
        "perf sparse-dense bench",
        "perf sparse-dense summarize",
        "stability event-scan-matrix",
        "coverage generate-status",
        "coverage gate",
        "profile",
        "profile list",
        "profile use",
        "ctx",
        "set",
        "unset",
        "flags",
        "validate",
        "list",
        "plan",
        "run",
        "status",
        "monitor",
        "agent-context",
        "ls",
        "cd",
        "tree",
        "quit",
        "/quit",
        "clear",
        "/clear",
    ];
    let mut out = Vec::new();
    if prefix.is_empty() {
        out.extend(cmds.iter().map(|s| s.to_string()));
        return Ok(out);
    }
    for c in cmds {
        if c.starts_with(prefix.as_str()) {
            out.push(c.to_string());
        }
    }
    Ok(out)
}

fn tokenize(line: &str) -> Vec<String> {
    line.split_whitespace().map(|s| s.to_string()).collect()
}

fn repl_render_config() -> RenderConfig<'static> {
    if !crate::ui::terminal_styles_enabled() {
        return RenderConfig::empty();
    }
    RenderConfig::default_colored()
        .with_prompt_prefix(Styled::new(""))
        .with_help_message(StyleSheet::empty().with_fg(Color::DarkGrey))
}

fn strip_leading_slash(line: &str) -> &str {
    match line.strip_prefix('/') {
        Some(rest) => rest.trim_start(),
        None => line,
    }
}

fn take_flag(args: &[String], name: &str) -> Option<String> {
    let mut i = 0usize;
    while i < args.len() {
        if args[i] == name {
            return args.get(i + 1).cloned();
        }
        i += 1;
    }
    None
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn parse_tags(args: &[String]) -> Option<Vec<String>> {
    let v = take_flag(args, "--tags")?;
    let out = v
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    if out.is_empty() { None } else { Some(out) }
}

fn parse_workers(args: &[String]) -> Result<Option<usize>> {
    let Some(v) = take_flag(args, "--workers") else { return Ok(None) };
    Ok(Some(v.parse::<usize>()?))
}

fn parse_format(args: &[String]) -> Result<OutputFormat> {
    let fmt = take_flag(args, "--format").unwrap_or_else(|| "human".to_string());
    OutputFormat::parse(&fmt)
}

fn parse_path(args: &[String], name: &str) -> Option<PathBuf> {
    take_flag(args, name).map(PathBuf::from)
}

fn print_help() {
    println!("Essentials");
    println!("  help | h | /help        Command reference");
    println!("  clear | cls | /clear    Clear the screen");
    println!("  quit | exit | q | /quit Exit the session");
    println!();
    println!("Navigation");
    println!("  ls                    List subcommands under the current prefix");
    println!("  cd <name|..|/>        Enter a group, go up, or reset to root");
    println!("  tree [path] [--depth N|--all|--select]   Command tree");
    println!();
    println!("Context");
    println!("  ctx                   Show active defaults");
    println!("  set <key> <value>     repo-root, rustmodlica-exe, cargo-target-dir, config, tier, tags, workers, baseline, incremental, manifest, data-root, out-dir, format");
    println!("  unset <key>           Clear a context key");
    println!("  flags <on|off> <ndjson|summary-compat|progress>");
    println!("  profile | profile list | profile use <name>");
    println!("  scope list | scope use <name> | scope gen-config <name>");
    println!();
    println!("Regression workflow");
    println!("  validate [--config <path>]");
    println!("  list [--format human|json] [--config <path>] [--tier <tier>] [--tags a,b]");
    println!("  plan [--format human|json] [--config <path>] [--tier <tier>] [--tags a,b] [--workers <n>]");
    println!("       [--baseline <path>] [--incremental <strategy>] [--manifest <path>] [--data-root <path>] [--out-dir <path>]");
    println!("  run [--config <path>] [--tier <tier>] [--tags a,b] [--workers <n>]");
    println!("      [--baseline <path>] [--incremental <strategy>] [--manifest <path>] [--data-root <path>] [--out-dir <path>]");
    println!("      [--ndjson] [--summary-compat] [--progress]");
    println!("  status [--data-root <path>] [--format human|json]");
    println!("  monitor [--data-root <path>] [--tail <n>] [--follow] [--source auto|cases|events|session]");
    println!("  agent-context [--data-root <path>] [--config <path>]");
    println!();
    println!("Script-migrated commands:");
    println!("  sync determinism --model <m> [--cargo-target-dir <dir>] [--output-interval <f>] [--artifacts-dir <dir>]");
    println!("  sync trace-assert --model <m> --expect-substr <s> --t-end <f> [--expect-times a,b] [--disallow-times a,b]");
    println!("                 [--cargo-target-dir <dir>] [--artifacts-dir <dir>]");
    println!("  fmi emit-fmu --model <m> [--cargo-target-dir <dir>] [--out-dir <dir>]");
    println!("  fmi validate --dir <path>");
    println!("  perf sparse-dense bench [--models a,b] [--t-end <f>] [--dt <f>] [--warnings <s>] [--out-dir <dir>]");
    println!("  perf sparse-dense summarize [--input-dir <dir>] [--output-dir <dir>] [--blt-guard-filter all|non_triggered|triggered]");
    println!("                            [--model-filter a,b]");
    println!("  stability event-scan-matrix --lib-path <p1,p2> [--out-dir <dir>] [--models a,b] [--count-values a,b] [--tail-velocity-values a,b]");
    println!("                            [--top-n <n>] [--allow-unsupported]");
    println!("  coverage generate-status");
    println!("  coverage gate [--status-json <path>]");
    println!();
    println!("(Leading '/' is optional, e.g. `/run` and `run` are equivalent.)");
}

#[derive(Clone, Copy)]
struct CmdNode {
    name: &'static str,
    children: &'static [CmdNode],
}

const N_SCOPE: &[CmdNode] = &[
    CmdNode { name: "list", children: &[] },
    CmdNode { name: "use", children: &[] },
    CmdNode {
        name: "gen-config",
        children: &[],
    },
];

const N_PROFILE: &[CmdNode] = &[
    CmdNode { name: "list", children: &[] },
    CmdNode { name: "use", children: &[] },
];

const N_SYNC: &[CmdNode] = &[
    CmdNode {
        name: "determinism",
        children: &[],
    },
    CmdNode {
        name: "trace-assert",
        children: &[],
    },
];

const N_FMI: &[CmdNode] = &[
    CmdNode {
        name: "emit-fmu",
        children: &[],
    },
    CmdNode {
        name: "validate",
        children: &[],
    },
];

const N_PERF_SPARSE_DENSE: &[CmdNode] = &[
    CmdNode { name: "bench", children: &[] },
    CmdNode {
        name: "summarize",
        children: &[],
    },
];

const N_PERF: &[CmdNode] = &[CmdNode {
    name: "sparse-dense",
    children: N_PERF_SPARSE_DENSE,
}];

const N_STABILITY: &[CmdNode] = &[CmdNode {
    name: "event-scan-matrix",
    children: &[],
}];

const N_COVERAGE: &[CmdNode] = &[
    CmdNode {
        name: "generate-status",
        children: &[],
    },
    CmdNode { name: "gate", children: &[] },
];

const COMMAND_TREE: &[CmdNode] = &[
    CmdNode {
        name: "scope",
        children: N_SCOPE,
    },
    CmdNode {
        name: "profile",
        children: N_PROFILE,
    },
    CmdNode {
        name: "sync",
        children: N_SYNC,
    },
    CmdNode { name: "fmi", children: N_FMI },
    CmdNode {
        name: "perf",
        children: N_PERF,
    },
    CmdNode {
        name: "stability",
        children: N_STABILITY,
    },
    CmdNode {
        name: "coverage",
        children: N_COVERAGE,
    },
    CmdNode {
        name: "validate",
        children: &[],
    },
    CmdNode { name: "list", children: &[] },
    CmdNode { name: "plan", children: &[] },
    CmdNode { name: "run", children: &[] },
    CmdNode {
        name: "status",
        children: &[],
    },
    CmdNode {
        name: "monitor",
        children: &[],
    },
    CmdNode {
        name: "agent-context",
        children: &[],
    },
    CmdNode { name: "ctx", children: &[] },
    CmdNode { name: "set", children: &[] },
    CmdNode {
        name: "unset",
        children: &[],
    },
    CmdNode {
        name: "flags",
        children: &[],
    },
    CmdNode { name: "ls", children: &[] },
    CmdNode { name: "cd", children: &[] },
    CmdNode { name: "tree", children: &[] },
    CmdNode { name: "quit", children: &[] },
];

fn tree_find_child<'a>(nodes: &'a [CmdNode], name: &str) -> Option<&'a CmdNode> {
    nodes.iter().find(|n| n.name == name)
}

fn tree_node_at_path<'a>(path: &[String]) -> Result<&'a [CmdNode]> {
    let mut nodes: &'a [CmdNode] = COMMAND_TREE;
    for seg in path {
        let Some(n) = tree_find_child(nodes, seg) else {
            bail!("unknown tree path segment: {}", seg);
        };
        nodes = n.children;
    }
    Ok(nodes)
}

fn tree_select_interactive() -> Result<Vec<String>> {
    let mut path: Vec<String> = Vec::new();
    loop {
        let nodes = tree_node_at_path(&path)?;
        let mut labels: Vec<String> = Vec::new();
        if !path.is_empty() {
            labels.push("..".to_string());
        }
        for n in nodes {
            if n.children.is_empty() {
                labels.push(n.name.to_string());
            } else {
                labels.push(format!("{} ({})", n.name, n.children.len()));
            }
        }
        labels.push("<exit>".to_string());

        let title = if path.is_empty() {
            "Command tree".to_string()
        } else {
            format!("Command tree: {}", path.join("/"))
        };
        let picked = Select::new(&title, labels)
            .with_render_config(repl_render_config())
            .prompt()?;
        if picked == "<exit>" {
            bail!("cancelled");
        }
        if picked == ".." {
            let _ = path.pop();
            continue;
        }
        let picked_name = picked
            .split(' ')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if picked_name.is_empty() {
            continue;
        }
        path.push(picked_name.clone());
        let nodes2 = tree_node_at_path(&path)?;
        if nodes2.is_empty() {
            return Ok(path);
        }
    }
}

fn print_command_tree(path: Option<&str>, depth: usize) -> Result<()> {
    // Keep it ASCII-only by project rule.
    println!("Commands (tree):");

    let mut nodes = COMMAND_TREE;
    if let Some(p) = path {
        let p = p.trim().trim_matches('/');
        if !p.is_empty() {
            for seg in p.split('/') {
                let Some(n) = tree_find_child(nodes, seg) else {
                    bail!("unknown tree path: {}", p);
                };
                nodes = n.children;
            }
        }
    }

    fn rec(nodes: &[CmdNode], indent: usize, depth: usize) {
        if depth == 0 {
            return;
        }
        for n in nodes {
            let has_children = !n.children.is_empty();
            let marker = if has_children { "+" } else { "-" };
            let suffix = if has_children {
                format!(" ({})", n.children.len())
            } else {
                String::new()
            };
            println!(
                "{:indent$}{} {}{}",
                "",
                marker,
                n.name,
                suffix,
                indent = indent
            );
            if has_children {
                rec(n.children, indent + 2, depth - 1);
            }
        }
    }

    rec(nodes, 2, depth);
    Ok(())
}
fn prompt_label(ctx: &ReplContext) -> String {
    if ctx.cmd_prefix.is_empty() {
        "> ".to_string()
    } else {
        format!("{}/> ", ctx.cmd_prefix.join("/"))
    }
}

fn list_subcommands(ctx: &ReplContext) -> Vec<&'static str> {
    let top = vec![
        "scope",
        "sync",
        "fmi",
        "perf",
        "stability",
        "coverage",
        "profile",
        "validate",
        "list",
        "plan",
        "run",
        "status",
        "monitor",
        "agent-context",
    ];
    if ctx.cmd_prefix.is_empty() {
        return top;
    }
    match ctx.cmd_prefix.join(" ").as_str() {
        "scope" => vec!["list", "use", "gen-config"],
        "profile" => vec!["list", "use"],
        "sync" => vec!["determinism", "trace-assert"],
        "fmi" => vec!["emit-fmu", "validate"],
        "perf" => vec!["sparse-dense"],
        "perf sparse-dense" => vec!["bench", "summarize"],
        "stability" => vec!["event-scan-matrix"],
        "coverage" => vec!["generate-status", "gate"],
        _ => vec![],
    }
}

fn cd_prefix(ctx: &mut ReplContext, target: &str) -> Result<()> {
    let t = target.trim();
    if t.is_empty() {
        bail!("usage: cd <name|..|/>");
    }
    match t {
        "/" => {
            ctx.cmd_prefix.clear();
            return Ok(());
        }
        ".." => {
            let _ = ctx.cmd_prefix.pop();
            return Ok(());
        }
        _ => {}
    }
    let allowed = list_subcommands(ctx);
    if !allowed.iter().any(|x| *x == t) {
        bail!("unknown command group under current prefix: {t}");
    }
    ctx.cmd_prefix.push(t.to_string());
    Ok(())
}

fn expand_with_prefix(ctx: &ReplContext, mut tokens: Vec<String>) -> Vec<String> {
    if ctx.cmd_prefix.is_empty() {
        return tokens;
    }
    let global = [
        "help",
        "h",
        "ctx",
        "set",
        "unset",
        "flags",
        "quit",
        "exit",
        "q",
        "ls",
        "cd",
        "clear",
        "cls",
    ];
    if tokens
        .get(0)
        .map(|s| global.iter().any(|g| g == s))
        .unwrap_or(false)
    {
        return tokens;
    }
    let mut out = Vec::new();
    out.extend(ctx.cmd_prefix.iter().cloned());
    out.append(&mut tokens);
    out
}

fn profile_list() {
    println!("Scopes:");
    for s in crate::scope::list_scopes() {
        println!("  {}\t{}\t{:?}", s.name, s.desc, s.source);
    }
}

fn profile_use(ctx: &mut ReplContext, name: &str) -> Result<()> {
    let repo_root = ctx_repo_root(ctx)?;
    let resolved = crate::scope::resolve_scope(&repo_root, name)?;
    ctx.config = Some(absolutize_under(&repo_root, resolved.config_path.clone()));
    if let Some(dr) = resolved.data_root {
        ctx.data_root = absolutize_under(&repo_root, dr);
    }
    ctx.progress = resolved.progress;
    ctx.format = resolved.format;
    ctx.tier = None;
    ctx.tags = None;
    ctx.baseline = None;
    ctx.incremental = Some("none".to_string());
    ctx.manifest = None;
    ctx.out_dir = None;
    ctx.ndjson = false;
    ctx.summary_compat = false;
    println!("scope set: {}", name.trim());
    print_ctx(ctx);
    Ok(())
}

fn print_ctx(ctx: &ReplContext) {
    println!(
        "ctx.repo_root={}",
        ctx.repo_root
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<auto>".to_string())
    );
    println!(
        "ctx.rustmodlica_exe={}",
        ctx.rustmodlica_exe
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<auto>".to_string())
    );
    println!(
        "ctx.cargo_target_dir={}",
        ctx.cargo_target_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<auto>".to_string())
    );
    println!("ctx.data_root={}", ctx.data_root.display());
    println!(
        "ctx.config={}",
        ctx.config
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<unset>".to_string())
    );
    println!("ctx.tier={}", ctx.tier.clone().unwrap_or_else(|| "<none>".to_string()));
    println!(
        "ctx.tags={}",
        ctx.tags
            .as_ref()
            .map(|v| v.join(","))
            .unwrap_or_else(|| "<none>".to_string())
    );
    println!(
        "ctx.workers={}",
        ctx.workers.map(|v| v.to_string()).unwrap_or_else(|| "<none>".to_string())
    );
    println!(
        "ctx.incremental={}",
        ctx.incremental.clone().unwrap_or_else(|| "<none>".to_string())
    );
    println!(
        "ctx.baseline={}",
        ctx.baseline
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".to_string())
    );
    println!(
        "ctx.manifest={}",
        ctx.manifest
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".to_string())
    );
    println!(
        "ctx.out_dir={}",
        ctx.out_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".to_string())
    );
    println!(
        "ctx.format={:?} ndjson={} summary_compat={} progress={}",
        ctx.format, ctx.ndjson, ctx.summary_compat, ctx.progress
    );
}


#[path = "repl_dispatch.rs"]
mod repl_dispatch;

pub use repl_dispatch::run_single_command;
pub use repl_dispatch::run_repl;
