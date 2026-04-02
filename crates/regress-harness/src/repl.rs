use anyhow::{bail, Result};
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
    println!("Commands:");
    println!("  help | h");
    println!("  ls                    (list subcommands under current prefix)");
    println!("  cd <name|..|/>        (enter group, back, or reset)");
    println!("  tree [path] [--depth N|--all|--select]   (print command tree, collapsible)");
    println!("  scope list");
    println!("  scope use <name>    (sets ctx defaults)");
    println!("  scope gen-config <name>");
    println!("  profile list");
    println!("  profile use <name>  (sets ctx defaults)");
    println!("  profile             (alias for profile list)");
    println!("  ctx");
    println!("  set <key> <value>   (keys: repo-root rustmodlica-exe cargo-target-dir config tier tags workers baseline incremental manifest data-root out-dir format)");
    println!("  unset <key>         (keys: repo-root rustmodlica-exe cargo-target-dir tier tags workers baseline incremental manifest out-dir)");
    println!("  flags <on|off> <ndjson|summary-compat|progress>");
    println!("  quit | exit | q");
    println!();
    println!("Work commands (use ctx defaults; you can override with flags):");
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
        let picked = Select::new(&title, labels).prompt()?;
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
        "regress-harness>".to_string()
    } else {
        format!("regress-harness/{}>", ctx.cmd_prefix.join("/"))
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
        "help", "h", "ctx", "set", "unset", "flags", "quit", "exit", "q", "ls", "cd",
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

fn execute_line(ctx: &mut ReplContext, line: &str) -> Result<bool> {
    let raw = line.trim();
    if raw.is_empty() {
        return Ok(true);
    }
    let args = expand_with_prefix(ctx, tokenize(raw));
    let cmd = args[0].to_ascii_lowercase();
    let rest = args[1..].to_vec();

    match cmd.as_str() {
        "ls" => {
            let subs = list_subcommands(ctx);
            if subs.is_empty() {
                println!("<no subcommands>");
            } else {
                for s in subs {
                    println!("{s}");
                }
            }
            Ok(true)
        }
        "cd" => {
            let t = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            cd_prefix(ctx, t)?;
            println!("{}", prompt_label(ctx));
            Ok(true)
        }
        "tree" => {
            if rest.iter().any(|s| s == "--select") {
                match tree_select_interactive() {
                    Ok(p) => {
                        println!("selected: {}", p.join(" "));
                        Ok(true)
                    }
                    Err(e) => {
                        // Keep non-fatal; interactive cancel is common.
                        println!("[ERROR] {}", e);
                        Ok(true)
                    }
                }
            } else {
            let mut depth = 1usize;
            let mut path: Option<&str> = None;
            if rest.iter().any(|s| s == "--all") {
                depth = 64;
            } else if let Some(d) = take_flag(&rest, "--depth").and_then(|s| s.parse::<usize>().ok())
            {
                depth = d.max(1);
            }
            if let Some(p) = rest.get(0).map(|s| s.as_str()) {
                if !p.starts_with("--") {
                    path = Some(p);
                }
            }
            print_command_tree(path, depth)?;
            Ok(true)
            }
        }
        "help" | "h" => {
            print_help();
            Ok(true)
        }
        "scope" => {
            if rest.is_empty() {
                profile_list();
                return Ok(true);
            }
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            match sub {
                "list" => {
                    profile_list();
                    Ok(true)
                }
                "use" => {
                    let name = rest.get(1).map(|s| s.as_str()).unwrap_or("");
                    if name.is_empty() {
                        let scopes = crate::scope::list_scopes();
                        let labels = scopes
                            .iter()
                            .map(|s| format!("{}  -  {}", s.name, s.desc))
                            .collect::<Vec<_>>();
                        let picked = inquire::Select::new("Select scope", labels).prompt()?;
                        let picked_name = picked
                            .split("  -  ")
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if picked_name.is_empty() {
                            bail!("no scope selected");
                        }
                        profile_use(ctx, &picked_name)?;
                    } else {
                        profile_use(ctx, name)?;
                    }
                    Ok(true)
                }
                "gen-config" => {
                    let name = rest.get(1).map(|s| s.as_str()).unwrap_or("");
                    if name.is_empty() {
                        let scopes = crate::scope::list_scopes();
                        let labels = scopes
                            .iter()
                            .map(|s| format!("{}  -  {}", s.name, s.desc))
                            .collect::<Vec<_>>();
                        let picked = inquire::Select::new("Select scope", labels).prompt()?;
                        let picked_name = picked
                            .split("  -  ")
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if picked_name.is_empty() {
                            bail!("no scope selected");
                        }
                        let repo_root = ctx_repo_root(ctx)?;
                        let resolved = crate::scope::resolve_scope(&repo_root, &picked_name)?;
                        println!(
                            "config_path={}",
                            absolutize_under(&repo_root, resolved.config_path).display()
                        );
                        return Ok(true);
                    }
                    let repo_root = ctx_repo_root(ctx)?;
                    let resolved = crate::scope::resolve_scope(&repo_root, name)?;
                    println!(
                        "config_path={}",
                        absolutize_under(&repo_root, resolved.config_path).display()
                    );
                    Ok(true)
                }
                _ => bail!("usage: scope list | scope use <name> | scope gen-config <name>"),
            }
        }
        "sync" => {
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            match sub {
                "determinism" => {
                    let model = take_flag(&rest, "--model").unwrap_or_default();
                    if model.is_empty() {
                        bail!("missing --model");
                    }
                    let cargo_target_dir = take_flag(&rest, "--cargo-target-dir")
                        .map(PathBuf::from)
                        .or_else(|| ctx.cargo_target_dir.clone())
                        .unwrap_or_else(|| PathBuf::from("target_regression"));
                    let output_interval = take_flag(&rest, "--output-interval")
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.001);
                    let artifacts_dir = take_flag(&rest, "--artifacts-dir")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("build/regression_data_jit_phase1/artifacts"));
                    let repo_root = ctx_repo_root(ctx)?;
                    let r = crate::sync_tools::sync_determinism(
                        &repo_root,
                        &absolutize_under(&repo_root, cargo_target_dir),
                        &model,
                        output_interval,
                        &absolutize_under(&repo_root, artifacts_dir),
                    )?;
                    println!(
                        "[sync-det] model={} ok={} exit_a={} exit_b={} csv_a={} csv_b={} hash_a={} hash_b={} wall_ms_a={} wall_ms_b={}",
                        model,
                        r.ok,
                        r.exit_a,
                        r.exit_b,
                        r.csv_a.display(),
                        r.csv_b.display(),
                        r.hash_a.clone().unwrap_or_else(|| "-".to_string()),
                        r.hash_b.clone().unwrap_or_else(|| "-".to_string()),
                        r.wall_ms_a,
                        r.wall_ms_b
                    );
                    Ok(true)
                }
                "trace-assert" => {
                    let model = take_flag(&rest, "--model").unwrap_or_default();
                    let expect_substr = take_flag(&rest, "--expect-substr").unwrap_or_default();
                    let t_end = take_flag(&rest, "--t-end")
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(-1.0);
                    if model.is_empty() || expect_substr.is_empty() || t_end < 0.0 {
                        bail!("missing --model/--expect-substr/--t-end");
                    }
                    let cargo_target_dir = take_flag(&rest, "--cargo-target-dir")
                        .map(PathBuf::from)
                        .or_else(|| ctx.cargo_target_dir.clone())
                        .unwrap_or_else(|| PathBuf::from("target_regression"));
                    let artifacts_dir = take_flag(&rest, "--artifacts-dir")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("build/regression_data_jit_phase1/artifacts"));
                    let expect_times = take_flag(&rest, "--expect-times").unwrap_or_default();
                    let disallow_times = take_flag(&rest, "--disallow-times").unwrap_or_default();
                    let repo_root = ctx_repo_root(ctx)?;
                    let r = crate::sync_tools::sync_trace_assert(
                        &repo_root,
                        &absolutize_under(&repo_root, cargo_target_dir),
                        &model,
                        &expect_substr,
                        t_end,
                        &expect_times,
                        &disallow_times,
                        &absolutize_under(&repo_root, artifacts_dir),
                    )?;
                    println!(
                        "[sync-trace-assert] model={} ok={} exit={} trace={} csv={}",
                        model,
                        r.ok,
                        r.exit_code,
                        r.trace_path.display(),
                        r.csv_path.display()
                    );
                    Ok(true)
                }
                _ => bail!("usage: sync determinism|trace-assert ..."),
            }
        }
        "fmi" => {
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            match sub {
                "emit-fmu" => {
                    let model = take_flag(&rest, "--model").unwrap_or_default();
                    if model.is_empty() {
                        bail!("missing --model");
                    }
                    let cargo_target_dir = take_flag(&rest, "--cargo-target-dir")
                        .map(PathBuf::from)
                        .or_else(|| ctx.cargo_target_dir.clone())
                        .unwrap_or_else(|| PathBuf::from("target_regression"));
                    let out_dir = take_flag(&rest, "--out-dir")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("build_regress_fmu"));
                    let repo_root = ctx_repo_root(ctx)?;
                    let r = crate::fmi_tools::fmi_emit_fmu(
                        &repo_root,
                        &absolutize_under(&repo_root, cargo_target_dir),
                        &absolutize_under(&repo_root, out_dir),
                        &model,
                    )?;
                    println!(
                        "[fmi] ok={} exit={} out_dir={} md={} c={} {}",
                        r.ok,
                        r.exit_code,
                        r.out_dir.display(),
                        r.model_description.display(),
                        r.c_file.display(),
                        r.flags
                    );
                    Ok(true)
                }
                "validate" => {
                    let dir = take_flag(&rest, "--dir").unwrap_or_default();
                    if dir.is_empty() {
                        bail!("missing --dir");
                    }
                    crate::fmi_tools::fmi_validate_dir(PathBuf::from(dir).as_path())?;
                    println!("ok");
                    Ok(true)
                }
                _ => bail!("usage: fmi emit-fmu|validate ..."),
            }
        }
        "perf" => {
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            if sub != "sparse-dense" {
                bail!("usage: perf sparse-dense ...");
            }
            let action = rest.get(1).map(|s| s.as_str()).unwrap_or("");
            match action {
                "bench" => {
                    let models_arg = take_flag(&rest, "--models").unwrap_or_default();
                    let models = crate::sparse_dense::parse_models_arg(&models_arg);
                    let t_end = take_flag(&rest, "--t-end")
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(1.0);
                    let dt = take_flag(&rest, "--dt")
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.01);
                    let warnings = take_flag(&rest, "--warnings").unwrap_or_else(|| "none".to_string());
                    let out_dir = take_flag(&rest, "--out-dir")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("build_sparse_dense_bench"));
                    let repo_root = ctx_repo_root(ctx)?;
                    let out = crate::sparse_dense::bench_sparse_dense(
                        &repo_root,
                        &models,
                        t_end,
                        dt,
                        &warnings,
                        &absolutize_under(&repo_root, out_dir),
                        false,
                    )?;
                    println!("bench_csv={}", out.csv_path.display());
                    println!("bench_json={}", out.json_path.display());
                    Ok(true)
                }
                "summarize" => {
                    let input_dir = take_flag(&rest, "--input-dir")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("jit-compiler/build_sparse_dense_bench"));
                    let output_dir = take_flag(&rest, "--output-dir")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("build_sparse_dense_summary"));
                    let blt_guard_filter =
                        take_flag(&rest, "--blt-guard-filter").unwrap_or_else(|| "non_triggered".to_string());
                    let model_filter_arg = take_flag(&rest, "--model-filter").unwrap_or_default();
                    let model_filter = model_filter_arg
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>();
                    let out = crate::sparse_dense::summarize_sparse_dense(
                        &input_dir,
                        &output_dir,
                        &blt_guard_filter,
                        &model_filter,
                    )?;
                    println!("summary_csv={}", out.csv_path.display());
                    println!("summary_json={}", out.json_path.display());
                    Ok(true)
                }
                _ => bail!("usage: perf sparse-dense bench|summarize ..."),
            }
        }
        "stability" => {
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            if sub != "event-scan-matrix" {
                bail!("usage: stability event-scan-matrix ...");
            }
            let lib_paths = take_flag(&rest, "--lib-path").unwrap_or_default();
            let lib_paths = lib_paths
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            if lib_paths.is_empty() {
                bail!("missing --lib-path (comma-separated)");
            }
            let out_dir = take_flag(&rest, "--out-dir")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("build_stability/event_scan_matrix_ci"));
            let models = take_flag(&rest, "--models")
                .unwrap_or_else(|| "TestLib/BouncingBall,TestLib/Pendulum,ModelicaTest.JitStress.SyncOmCompare".to_string());
            let models = models
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            let count_values = take_flag(&rest, "--count-values")
                .unwrap_or_else(|| "0.0004,0.0005,0.0006,0.0008".to_string());
            let count_values = count_values
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            let tail_values = take_flag(&rest, "--tail-velocity-values")
                .unwrap_or_else(|| "0.02,0.03,0.04,0.05".to_string());
            let tail_velocity_values = tail_values
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            let top_n = take_flag(&rest, "--top-n")
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(3);
            let allow_unsupported = has_flag(&rest, "--allow-unsupported");
            let repo_root = ctx_repo_root(ctx)?;
            let ok = crate::event_scan_matrix::run_event_scan_matrix(
                &repo_root,
                &crate::event_scan_matrix::EventScanMatrixArgs {
                    out_dir: absolutize_under(&repo_root, out_dir),
                    models,
                    count_values,
                    tail_velocity_values,
                    lib_paths,
                    top_n,
                    allow_unsupported,
                },
            )?;
            println!("[event-scan] ok={ok}");
            Ok(true)
        }
        "coverage" => {
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            match sub {
                "generate-status" => {
                    let repo_root = ctx_repo_root(ctx)?;
                    let p = crate::coverage_status::generate_coverage_status(&repo_root)?;
                    println!("coverage_status_json={}", p.display());
                    Ok(true)
                }
                "gate" => {
                    let repo_root = ctx_repo_root(ctx)?;
                    let status_json = take_flag(&rest, "--status-json")
                        .map(PathBuf::from)
                        .map(|p| absolutize_under(&repo_root, p))
                        .unwrap_or_else(|| repo_root.join("jit-compiler/scripts/coverage_status.json"));
                    let ok = crate::coverage_status::coverage_gate(&status_json)?;
                    println!(
                        "[coverage-gate] ok={} status_json={}",
                        ok,
                        status_json.display()
                    );
                    Ok(true)
                }
                _ => bail!("usage: coverage generate-status|gate ..."),
            }
        }
        "profile" => {
            if rest.is_empty() {
                profile_list();
                return Ok(true);
            }
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            match sub {
                "list" => {
                    profile_list();
                    Ok(true)
                }
                "use" => {
                    let name = rest.get(1).map(|s| s.as_str()).unwrap_or("");
                    if name.is_empty() {
                        let scopes = crate::scope::list_scopes();
                        let labels = scopes
                            .iter()
                            .map(|s| format!("{}  -  {}", s.name, s.desc))
                            .collect::<Vec<_>>();
                        let picked = inquire::Select::new("Select scope", labels).prompt()?;
                        let picked_name = picked
                            .split("  -  ")
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if picked_name.is_empty() {
                            bail!("no scope selected");
                        }
                        profile_use(ctx, &picked_name)?;
                    } else {
                        profile_use(ctx, name)?;
                    }
                    Ok(true)
                }
                _ => bail!("usage: profile list | profile use <name>"),
            }
        }
        "ctx" => {
            print_ctx(ctx);
            Ok(true)
        }
        "set" => {
            let key = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            let value = rest.get(1).map(|s| s.as_str()).unwrap_or("");
            if key.is_empty() {
                bail!("usage: set <key> <value>");
            }
            let base_for_paths = ctx_repo_root(ctx)?;
            let value_owned: String;
            let value = if value.is_empty()
                && matches!(
                    key,
                    "repo-root"
                        | "rustmodlica-exe"
                        | "cargo-target-dir"
                        | "config"
                        | "baseline"
                        | "manifest"
                        | "data-root"
                        | "out-dir"
                )
            {
                value_owned = prompt_path("path", base_for_paths.clone())?;
                value_owned.as_str()
            } else {
                value
            };
            if value.is_empty() {
                bail!("usage: set <key> <value>");
            }
            match key {
                "repo-root" => {
                    let base = discover_repo_root()?;
                    let p = absolutize_under(&base, PathBuf::from(value));
                    ctx.repo_root = Some(canonicalize_best_effort(p));
                }
                "rustmodlica-exe" => {
                    let base = ctx_repo_root(ctx)?;
                    let p = absolutize_under(&base, PathBuf::from(value));
                    ctx.rustmodlica_exe = Some(canonicalize_best_effort(p));
                }
                "cargo-target-dir" => {
                    let base = ctx_repo_root(ctx)?;
                    let p = absolutize_under(&base, PathBuf::from(value));
                    ctx.cargo_target_dir = Some(canonicalize_best_effort(p));
                }
                "config" => ctx.config = Some(PathBuf::from(value)),
                "tier" => ctx.tier = Some(value.to_string()),
                "tags" => {
                    let v = value
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>();
                    ctx.tags = if v.is_empty() { None } else { Some(v) };
                }
                "workers" => ctx.workers = Some(value.parse::<usize>()?),
                "baseline" => ctx.baseline = Some(PathBuf::from(value)),
                "incremental" => ctx.incremental = Some(value.to_string()),
                "manifest" => ctx.manifest = Some(PathBuf::from(value)),
                "data-root" => ctx.data_root = PathBuf::from(value),
                "out-dir" => ctx.out_dir = Some(PathBuf::from(value)),
                "format" => ctx.format = OutputFormat::parse(value)?,
                _ => bail!("unknown key: {key}"),
            }
            Ok(true)
        }
        "unset" => {
            let key = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            if key.is_empty() {
                bail!("usage: unset <key>");
            }
            match key {
                "repo-root" => ctx.repo_root = None,
                "rustmodlica-exe" => ctx.rustmodlica_exe = None,
                "cargo-target-dir" => ctx.cargo_target_dir = None,
                "tier" => ctx.tier = None,
                "tags" => ctx.tags = None,
                "workers" => ctx.workers = None,
                "baseline" => ctx.baseline = None,
                "incremental" => ctx.incremental = None,
                "manifest" => ctx.manifest = None,
                "out-dir" => ctx.out_dir = None,
                _ => bail!("unknown/unsettable key: {key}"),
            }
            Ok(true)
        }
        "flags" => {
            let mode = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            let name = rest.get(1).map(|s| s.as_str()).unwrap_or("");
            let on = match mode {
                "on" => true,
                "off" => false,
                _ => bail!("usage: flags <on|off> <ndjson|summary-compat|progress>"),
            };
            match name {
                "ndjson" => ctx.ndjson = on,
                "summary-compat" => ctx.summary_compat = on,
                "progress" => ctx.progress = on,
                _ => bail!("unknown flag: {name}"),
            }
            Ok(true)
        }
        "quit" | "exit" | "q" => Ok(false),
        "validate" => {
            let config = parse_path(&rest, "--config")
                .or_else(|| ctx.config.clone())
                .ok_or_else(|| anyhow::anyhow!("missing --config (or set config)"))?;
            let root = ctx_repo_root(ctx)?;
            cmd_validate_config(&absolutize_under(&root, config))?;
            println!("{}", tr("ok"));
            Ok(true)
        }
        "list" => {
            let config = parse_path(&rest, "--config")
                .or_else(|| ctx.config.clone())
                .ok_or_else(|| anyhow::anyhow!("missing --config (or set config)"))?;
            let tier = take_flag(&rest, "--tier").or_else(|| ctx.tier.clone());
            let tags = parse_tags(&rest).or_else(|| ctx.tags.clone());
            let fmt = if take_flag(&rest, "--format").is_some() {
                parse_format(&rest)?
            } else {
                ctx.format
            };
            let root = ctx_repo_root(ctx)?;
            cmd_list_cases(absolutize_under(&root, config), tier, tags, fmt)?;
            Ok(true)
        }
        "plan" => {
            let config = parse_path(&rest, "--config")
                .or_else(|| ctx.config.clone())
                .ok_or_else(|| anyhow::anyhow!("missing --config (or set config)"))?;
            let tier = take_flag(&rest, "--tier").or_else(|| ctx.tier.clone());
            let tags = parse_tags(&rest).or_else(|| ctx.tags.clone());
            let workers = parse_workers(&rest)?.or(ctx.workers);
            let baseline = parse_path(&rest, "--baseline").or_else(|| ctx.baseline.clone());
            let incremental = take_flag(&rest, "--incremental").or_else(|| ctx.incremental.clone());
            let manifest = parse_path(&rest, "--manifest").or_else(|| ctx.manifest.clone());
            let data_root = parse_path(&rest, "--data-root").unwrap_or_else(|| ctx.data_root.clone());
            let out_dir = parse_path(&rest, "--out-dir").or_else(|| ctx.out_dir.clone());
            let fmt = if take_flag(&rest, "--format").is_some() {
                parse_format(&rest)?
            } else {
                ctx.format
            };
            let root = ctx_repo_root(ctx)?;
            cmd_plan(
                absolutize_under(&root, config),
                tier,
                tags,
                workers,
                baseline.map(|p| absolutize_under(&root, p)),
                incremental,
                manifest.map(|p| absolutize_under(&root, p)),
                absolutize_under(&root, data_root),
                out_dir.map(|p| absolutize_under(&root, p)),
                fmt,
            )?;
            Ok(true)
        }
        "run" => {
            let config = parse_path(&rest, "--config")
                .or_else(|| ctx.config.clone())
                .ok_or_else(|| anyhow::anyhow!("missing --config (or set config)"))?;
            let tier = take_flag(&rest, "--tier").or_else(|| ctx.tier.clone());
            let tags = parse_tags(&rest).or_else(|| ctx.tags.clone());
            let workers = parse_workers(&rest)?.or(ctx.workers);
            let baseline = parse_path(&rest, "--baseline").or_else(|| ctx.baseline.clone());
            let incremental = take_flag(&rest, "--incremental").or_else(|| ctx.incremental.clone());
            let manifest = parse_path(&rest, "--manifest").or_else(|| ctx.manifest.clone());
            let data_root = parse_path(&rest, "--data-root").unwrap_or_else(|| ctx.data_root.clone());
            let out_dir = parse_path(&rest, "--out-dir").or_else(|| ctx.out_dir.clone());
            let ndjson = has_flag(&rest, "--ndjson") || ctx.ndjson;
            let summary_compat = has_flag(&rest, "--summary-compat") || ctx.summary_compat;
            let progress = has_flag(&rest, "--progress") || ctx.progress;
            let root = ctx_repo_root(ctx)?;
            crate::run_cmd(
                absolutize_under(&root, config),
                tier,
                tags,
                workers,
                baseline.map(|p| absolutize_under(&root, p)),
                incremental,
                manifest.map(|p| absolutize_under(&root, p)),
                absolutize_under(&root, data_root),
                out_dir.map(|p| absolutize_under(&root, p)),
                ndjson,
                summary_compat,
                progress,
            )?;
            Ok(true)
        }
        "status" => {
            let data_root =
                parse_path(&rest, "--data-root").unwrap_or_else(|| ctx.data_root.clone());
            let fmt = if take_flag(&rest, "--format").is_some() {
                parse_format(&rest)?
            } else {
                ctx.format
            };
            let root = ctx_repo_root(ctx)?;
            cmd_status(absolutize_under(&root, data_root), fmt)?;
            Ok(true)
        }
        "monitor" => {
            let data_root =
                parse_path(&rest, "--data-root").unwrap_or_else(|| ctx.data_root.clone());
            let tail = take_flag(&rest, "--tail")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(20);
            let follow = has_flag(&rest, "--follow");
            let source = take_flag(&rest, "--source").unwrap_or_else(|| "auto".to_string());
            let root = ctx_repo_root(ctx)?;
            cmd_monitor(
                absolutize_under(&root, data_root),
                tail,
                follow,
                parse_monitor_source(Some(&source))?,
            )?;
            Ok(true)
        }
        "agent-context" => {
            let data_root =
                parse_path(&rest, "--data-root").unwrap_or_else(|| PathBuf::from("build/regression_data"));
            let config = parse_path(&rest, "--config");
            let root = ctx_repo_root(ctx)?;
            cmd_agent_context(
                absolutize_under(&root, data_root),
                config.map(|p| absolutize_under(&root, p)),
            )?;
            Ok(true)
        }
        _ => {
            bail!("unknown command: {cmd}");
        }
    }
}

pub fn run_repl() -> Result<()> {
    println!("regress-harness repl. Type 'help' for commands.");
    let mut ctx = ReplContext::default();
    loop {
        let line = Text::new(&prompt_label(&ctx))
            .with_autocomplete(command_suggestions)
            .with_help_message("Type a command. Use autocomplete.")
            .prompt();
        let line = match line {
            Ok(v) => v,
            Err(inquire::error::InquireError::OperationCanceled) => {
                continue;
            }
            Err(inquire::error::InquireError::OperationInterrupted) => {
                break;
            }
            Err(e) => return Err(anyhow::anyhow!(e)),
        };
        match execute_line(&mut ctx, &line) {
            Ok(keep) => {
                if !keep {
                    break;
                }
            }
            Err(e) => {
                eprintln!("[ERROR] {e}");
            }
        }
    }
    Ok(())
}

