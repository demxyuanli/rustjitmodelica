import path from "node:path";
import stringArgv from "string-argv";

export type OutputFormat = "human" | "json";

export interface SessionContext {
  cmdPrefix: string[];
  repoRoot: string | null;
  rustmodlicaExe: string | null;
  cargoTargetDir: string | null;
  config: string | null;
  tier: string | null;
  tags: string[] | null;
  workers: number | null;
  baseline: string | null;
  incremental: string | null;
  manifest: string | null;
  dataRoot: string;
  outDir: string | null;
  format: OutputFormat;
  ndjson: boolean;
  summaryCompat: boolean;
  progress: boolean;
}

export function defaultSessionContext(): SessionContext {
  return {
    cmdPrefix: [],
    repoRoot: null,
    rustmodlicaExe: null,
    cargoTargetDir: null,
    config: null,
    tier: null,
    tags: null,
    workers: null,
    baseline: null,
    incremental: null,
    manifest: null,
    dataRoot: "build/regression_data",
    outDir: null,
    format: "human",
    ndjson: false,
    summaryCompat: false,
    progress: false,
  };
}

const GLOBAL_NO_PREFIX = new Set([
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
  "tree",
]);

function normalizeLeadingSlash(s: string): string {
  const t = s.trim();
  if (t.startsWith("//")) {
    return t;
  }
  if (t.startsWith("/")) {
    return t.slice(1).trimStart();
  }
  return t;
}

function firstWord(line: string): string {
  const m = line.trim().match(/^[^\s]+/);
  return m ? m[0].toLowerCase() : "";
}

export function expandLineWithPrefix(prefix: string[], line: string): string {
  const t0 = line.trim();
  if (!t0 || t0.startsWith("//")) {
    return t0;
  }
  const body = normalizeLeadingSlash(t0);
  const fw = firstWord(body);
  if (prefix.length === 0) {
    return body;
  }
  if (GLOBAL_NO_PREFIX.has(fw)) {
    return body;
  }
  return `${prefix.join(" ")} ${body}`.trim();
}

export function listSubcommands(prefix: string[]): string[] {
  const top = [
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
  if (prefix.length === 0) {
    return top;
  }
  const key = prefix.join(" ");
  switch (key) {
    case "scope":
      return ["list", "use", "gen-config"];
    case "profile":
      return ["list", "use"];
    case "sync":
      return ["determinism", "trace-assert"];
    case "fmi":
      return ["emit-fmu", "validate"];
    case "perf":
      return ["sparse-dense"];
    case "perf sparse-dense":
      return ["bench", "summarize"];
    case "stability":
      return ["event-scan-matrix"];
    case "coverage":
      return ["generate-status", "gate"];
    default:
      return [];
  }
}

export function cdPrefix(ctx: SessionContext, target: string): SessionContext {
  const t = target.trim();
  if (!t) {
    throw new Error("usage: cd <name|..|/>");
  }
  if (t === "/") {
    return { ...ctx, cmdPrefix: [] };
  }
  if (t === "..") {
    const p = [...ctx.cmdPrefix];
    p.pop();
    return { ...ctx, cmdPrefix: p };
  }
  const allowed = listSubcommands(ctx.cmdPrefix);
  if (!allowed.includes(t)) {
    throw new Error(`unknown command group under current prefix: ${t}`);
  }
  return { ...ctx, cmdPrefix: [...ctx.cmdPrefix, t] };
}

function parseRestFlags(rest: string[]): Map<string, string | true> {
  const m = new Map<string, string | true>();
  for (let i = 0; i < rest.length; i++) {
    const a = rest[i];
    if (!a.startsWith("--")) {
      continue;
    }
    const key = a.slice(2);
    const next = rest[i + 1];
    if (next === undefined || next.startsWith("--")) {
      m.set(key, true);
    } else {
      m.set(key, next);
      i++;
    }
  }
  return m;
}

function str(
  fm: Map<string, string | true>,
  k: string,
  fallback: string | null | undefined
): string | undefined {
  const v = fm.get(k);
  if (typeof v === "string" && v.length > 0) {
    return v;
  }
  if (fallback) {
    return fallback;
  }
  return undefined;
}

function num(fm: Map<string, string | true>, k: string, fb: number | null): number | undefined {
  const v = fm.get(k);
  if (typeof v === "string") {
    const n = parseInt(v, 10);
    return Number.isFinite(n) ? n : undefined;
  }
  if (fb !== null && fb !== undefined) {
    return fb;
  }
  return undefined;
}

function tagsStr(
  fm: Map<string, string | true>,
  ctx: SessionContext
): string | undefined {
  const v = fm.get("tags");
  if (typeof v === "string") {
    return v;
  }
  if (ctx.tags && ctx.tags.length > 0) {
    return ctx.tags.join(",");
  }
  return undefined;
}

function boolFlag(
  fm: Map<string, string | true>,
  k: string,
  ctxVal: boolean
): boolean {
  if (fm.has(k)) {
    return true;
  }
  return ctxVal;
}

function r(cwd: string, p: string): string {
  return path.resolve(cwd, p);
}

export function buildCliArgvFromWorkflow(
  cmd: string,
  rest: string[],
  ctx: SessionContext,
  cwd: string
): string[] {
  const fm = parseRestFlags(rest);

  if (cmd === "validate") {
    const cfg = str(fm, "config", ctx.config);
    if (!cfg) {
      throw new Error("missing --config (or set config)");
    }
    return ["validate-config", "--config", r(cwd, cfg)];
  }

  if (cmd === "list") {
    const cfg = str(fm, "config", ctx.config);
    if (!cfg) {
      throw new Error("missing --config (or set config)");
    }
    const out = ["list-cases", "--config", r(cwd, cfg)];
    const tier = str(fm, "tier", ctx.tier);
    if (tier) {
      out.push("--tier", tier);
    }
    const tg = tagsStr(fm, ctx);
    if (tg) {
      out.push("--tags", tg);
    }
    const fmt =
      typeof fm.get("format") === "string"
        ? (fm.get("format") as string)
        : ctx.format;
    out.push("--format", fmt);
    return out;
  }

  if (cmd === "plan" || cmd === "run") {
    const cfg = str(fm, "config", ctx.config);
    if (!cfg) {
      throw new Error("missing --config (or set config)");
    }
    const out = [cmd, "--config", r(cwd, cfg)];
    const tier = str(fm, "tier", ctx.tier);
    if (tier) {
      out.push("--tier", tier);
    }
    const tg = tagsStr(fm, ctx);
    if (tg) {
      out.push("--tags", tg);
    }
    const w = num(fm, "workers", ctx.workers ?? null);
    if (w !== undefined) {
      out.push("--workers", String(w));
    }
    const bl = str(fm, "baseline", ctx.baseline);
    if (bl) {
      out.push("--baseline", r(cwd, bl));
    }
    const inc = str(fm, "incremental", ctx.incremental);
    if (inc) {
      out.push("--incremental", inc);
    }
    const man = str(fm, "manifest", ctx.manifest);
    if (man) {
      out.push("--manifest", r(cwd, man));
    }
    const dr = str(fm, "data-root", ctx.dataRoot);
    if (dr) {
      out.push("--data-root", r(cwd, dr));
    }
    const od = str(fm, "out-dir", ctx.outDir ?? undefined);
    if (od) {
      out.push("--out-dir", r(cwd, od));
    }
    if (cmd === "plan") {
      const fmt =
        typeof fm.get("format") === "string"
          ? (fm.get("format") as string)
          : ctx.format;
      out.push("--format", fmt);
    }
    if (cmd === "run") {
      if (boolFlag(fm, "ndjson", ctx.ndjson)) {
        out.push("--ndjson");
      }
      if (boolFlag(fm, "summary-compat", ctx.summaryCompat)) {
        out.push("--summary-compat");
      }
      if (boolFlag(fm, "progress", ctx.progress)) {
        out.push("--progress");
      }
    }
    return out;
  }

  if (cmd === "status") {
    const dr = str(fm, "data-root", ctx.dataRoot);
    const fmt =
      typeof fm.get("format") === "string"
        ? (fm.get("format") as string)
        : ctx.format;
    return ["status", "--data-root", r(cwd, dr ?? ctx.dataRoot), "--format", fmt];
  }

  if (cmd === "monitor") {
    const dr = str(fm, "data-root", ctx.dataRoot);
    const tail = num(fm, "tail", 20) ?? 20;
    const out = [
      "monitor",
      "--data-root",
      r(cwd, dr ?? ctx.dataRoot),
      "--tail",
      String(tail),
    ];
    if (fm.has("follow")) {
      out.push("--follow");
    }
    const src =
      typeof fm.get("source") === "string"
        ? (fm.get("source") as string)
        : "auto";
    out.push("--source", src);
    return out;
  }

  if (cmd === "agent-context") {
    const dr = str(fm, "data-root", ctx.dataRoot);
    const out = ["agent-context", "--data-root", r(cwd, dr ?? ctx.dataRoot)];
    const cfg = str(fm, "config", ctx.config ?? undefined);
    if (cfg) {
      out.push("--config", r(cwd, cfg));
    }
    return out;
  }

  throw new Error(`internal: not a workflow command: ${cmd}`);
}

export type DispatchResult =
  | { kind: "exit" }
  | { kind: "noop" }
  | { kind: "clear" }
  | { kind: "log"; lines: string[]; nextCtx?: SessionContext }
  | { kind: "spawn-cli"; argv: string[] }
  | { kind: "spawn-repl-exec"; command: string };

const WORKFLOW_CLI = new Set([
  "validate",
  "list",
  "plan",
  "run",
  "status",
  "monitor",
  "agent-context",
]);

export function dispatchLine(
  rawLine: string,
  ctx: SessionContext,
  cwd: string
): DispatchResult {
  const trimmed = rawLine.trim();
  if (!trimmed || trimmed.startsWith("//")) {
    return { kind: "noop" };
  }

  const expanded = expandLineWithPrefix(ctx.cmdPrefix, rawLine);
  if (!expanded.trim() || expanded.trim().startsWith("//")) {
    return { kind: "noop" };
  }

  const tokens = stringArgv(expanded);
  if (tokens.length === 0) {
    return { kind: "noop" };
  }

  const cmd0 = tokens[0].toLowerCase();
  const rest = tokens.slice(1);

  if (cmd0 === "exit" || cmd0 === "quit" || cmd0 === "q") {
    return { kind: "exit" };
  }

  if (cmd0 === "help" || cmd0 === "h") {
    return { kind: "log", lines: [HELP_TEXT] };
  }

  if (cmd0 === "clear" || cmd0 === "cls") {
    return { kind: "clear" };
  }

  if (cmd0 === "ls") {
    const subs = listSubcommands(ctx.cmdPrefix);
    return {
      kind: "log",
      lines: subs.length ? subs : ["<no subcommands>"],
    };
  }

  if (cmd0 === "cd") {
    const target = rest.join(" ").trim();
    try {
      const next = cdPrefix(ctx, target);
      return {
        kind: "log",
        lines: [promptHint(next.cmdPrefix)],
        nextCtx: next,
      };
    } catch (e) {
      return {
        kind: "log",
        lines: [`[error] ${e instanceof Error ? e.message : String(e)}`],
      };
    }
  }

  if (cmd0 === "ctx") {
    return { kind: "log", lines: formatCtx(ctx) };
  }

  if (cmd0 === "set") {
    const key = rest[0]?.toLowerCase() ?? "";
    const value = rest.slice(1).join(" ").trim();
    if (!key || !value) {
      return { kind: "log", lines: ["usage: set <key> <value>"] };
    }
    try {
      const next = applySet(ctx, key, value, cwd);
      return {
        kind: "log",
        lines: [`set ${key}`, ...formatCtx(next)],
        nextCtx: next,
      };
    } catch (e) {
      return {
        kind: "log",
        lines: [`[error] ${e instanceof Error ? e.message : String(e)}`],
      };
    }
  }

  if (cmd0 === "unset") {
    const key = rest[0]?.toLowerCase() ?? "";
    if (!key) {
      return { kind: "log", lines: ["usage: unset <key>"] };
    }
    try {
      const next = applyUnset(ctx, key);
      return { kind: "log", lines: [`unset ${key}`], nextCtx: next };
    } catch (e) {
      return {
        kind: "log",
        lines: [`[error] ${e instanceof Error ? e.message : String(e)}`],
      };
    }
  }

  if (cmd0 === "flags") {
    const mode = rest[0]?.toLowerCase() ?? "";
    const name = rest[1]?.toLowerCase() ?? "";
    const on = mode === "on" ? true : mode === "off" ? false : null;
    if (on === null || !name) {
      return {
        kind: "log",
        lines: ["usage: flags <on|off> <ndjson|summary-compat|progress>"],
      };
    }
    try {
      const next = applyFlags(ctx, name, on);
      return { kind: "log", lines: [`flags ${name}=${on}`], nextCtx: next };
    } catch (e) {
      return {
        kind: "log",
        lines: [`[error] ${e instanceof Error ? e.message : String(e)}`],
      };
    }
  }

  if (cmd0 === "repl") {
    return {
      kind: "log",
      lines: [
        "The Rust interactive REPL (inquire) is not nested here. Run: regress-harness repl",
      ],
    };
  }

  if (cmd0 === "agent") {
    return {
      kind: "log",
      lines: [
        "agent repl uses stdin JSON; run in a plain terminal: regress-harness agent repl",
      ],
    };
  }

  if (cmd0 === "jit") {
    return { kind: "spawn-cli", argv: tokens };
  }

  if (WORKFLOW_CLI.has(cmd0)) {
    try {
      const argv = buildCliArgvFromWorkflow(cmd0, rest, ctx, cwd);
      return { kind: "spawn-cli", argv };
    } catch (e) {
      return {
        kind: "log",
        lines: [`[error] ${e instanceof Error ? e.message : String(e)}`],
      };
    }
  }

  return { kind: "spawn-repl-exec", command: expanded };
}

function promptHint(prefix: string[]): string {
  if (prefix.length === 0) {
    return "> ";
  }
  return `${prefix.join("/")}/> `;
}

function formatCtx(ctx: SessionContext): string[] {
  return [
    `ctx.repo_root=${ctx.repoRoot ?? "<auto>"}`,
    `ctx.rustmodlica_exe=${ctx.rustmodlicaExe ?? "<auto>"}`,
    `ctx.cargo_target_dir=${ctx.cargoTargetDir ?? "<auto>"}`,
    `ctx.data_root=${ctx.dataRoot}`,
    `ctx.config=${ctx.config ?? "<unset>"}`,
    `ctx.tier=${ctx.tier ?? "<none>"}`,
    `ctx.tags=${ctx.tags?.join(",") ?? "<none>"}`,
    `ctx.workers=${ctx.workers ?? "<none>"}`,
    `ctx.incremental=${ctx.incremental ?? "<none>"}`,
    `ctx.baseline=${ctx.baseline ?? "<none>"}`,
    `ctx.manifest=${ctx.manifest ?? "<none>"}`,
    `ctx.out_dir=${ctx.outDir ?? "<none>"}`,
    `ctx.format=${ctx.format} ndjson=${ctx.ndjson} summary_compat=${ctx.summaryCompat} progress=${ctx.progress}`,
    `cmd_prefix=${ctx.cmdPrefix.length ? ctx.cmdPrefix.join(" ") : "<root>"}`,
  ];
}

function applySet(
  ctx: SessionContext,
  key: string,
  value: string,
  cwd: string
): SessionContext {
  const next = { ...ctx };
  switch (key) {
    case "repo-root":
      next.repoRoot = r(cwd, value);
      break;
    case "rustmodlica-exe":
      next.rustmodlicaExe = r(cwd, value);
      break;
    case "cargo-target-dir":
      next.cargoTargetDir = r(cwd, value);
      break;
    case "config":
      next.config = value;
      break;
    case "tier":
      next.tier = value;
      break;
    case "tags":
      next.tags = value
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);
      break;
    case "workers": {
      const n = parseInt(value, 10);
      if (!Number.isFinite(n)) {
        throw new Error("workers must be a number");
      }
      next.workers = n;
      break;
    }
    case "baseline":
      next.baseline = value;
      break;
    case "incremental":
      next.incremental = value;
      break;
    case "manifest":
      next.manifest = value;
      break;
    case "data-root":
      next.dataRoot = value;
      break;
    case "out-dir":
      next.outDir = value;
      break;
    case "format":
      if (value !== "human" && value !== "json") {
        throw new Error("format must be human or json");
      }
      next.format = value;
      break;
    default:
      throw new Error(`unknown key: ${key}`);
  }
  return next;
}

function applyUnset(ctx: SessionContext, key: string): SessionContext {
  const next = { ...ctx };
  switch (key) {
    case "repo-root":
      next.repoRoot = null;
      break;
    case "rustmodlica-exe":
      next.rustmodlicaExe = null;
      break;
    case "cargo-target-dir":
      next.cargoTargetDir = null;
      break;
    case "tier":
      next.tier = null;
      break;
    case "tags":
      next.tags = null;
      break;
    case "workers":
      next.workers = null;
      break;
    case "baseline":
      next.baseline = null;
      break;
    case "incremental":
      next.incremental = null;
      break;
    case "manifest":
      next.manifest = null;
      break;
    case "out-dir":
      next.outDir = null;
      break;
    default:
      throw new Error(`unknown/unsettable key: ${key}`);
  }
  return next;
}

function applyFlags(
  ctx: SessionContext,
  name: string,
  on: boolean
): SessionContext {
  const next = { ...ctx };
  switch (name) {
    case "ndjson":
      next.ndjson = on;
      break;
    case "summary-compat":
      next.summaryCompat = on;
      break;
    case "progress":
      next.progress = on;
      break;
    default:
      throw new Error(`unknown flag: ${name}`);
  }
  return next;
}

const HELP_TEXT = `
Essentials: help | exit | clear | ctx
Navigation: ls | cd <name|..|/>
Context: set | unset | flags
Workflow (merged with ctx): validate | list | plan | run | status | monitor | agent-context
JIT: jit <subcommand> ... (passed to Rust CLI)
Other REPL commands (scope, sync, fmi, perf, ...): forwarded via repl-exec -c

Examples:
  set config baseline/regress.json
  plan
  run --progress
  repl-exec is used internally for scope list
`.trim();

export function formatPrompt(cmdPrefix: string[]): string {
  if (cmdPrefix.length === 0) {
    return "> ";
  }
  return `${cmdPrefix.join("/")}/> `;
}
