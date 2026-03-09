import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import Editor from "@monaco-editor/react";
import { t } from "../i18n";
import { getSourceModules, getCaseToSourceFiles, type SourceModuleInfo } from "../data/jit_regression_metadata";
import { FileIcon } from "./FileIcon";

interface SymbolInfo {
  name: string;
  kind: string;
  lineStart: number;
  lineEnd: number;
  signature: string | null;
  docComment: string | null;
  filePath: string;
}

interface SourceTreeEntry {
  name: string;
  path?: string;
  children?: SourceTreeEntry[];
  isDir: boolean;
}

interface GitLogEntry {
  hash: string;
  short: string;
  author: string;
  date: string;
  message: string;
}

const SB_TREE_INDENT = 14;
const SB_TREE_BASE = 8;

function TreeNode({
  entry,
  depth,
  selectedPath,
  onSelect,
}: {
  entry: SourceTreeEntry;
  depth: number;
  selectedPath: string | null;
  onSelect: (path: string) => void;
}) {
  const [expanded, setExpanded] = useState(depth < 1);
  const paddingLeft = SB_TREE_BASE + depth * SB_TREE_INDENT;

  if (entry.isDir) {
    return (
      <div className="flex flex-col">
        <div className="tree-row group rounded" style={{ paddingLeft }}>
          <button
            type="button"
            className="tree-arrow text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10 rounded"
            onClick={() => setExpanded(!expanded)}
            aria-expanded={expanded}
          >
            {expanded ? "\u02C5" : "\u203A"}
          </button>
          <span
            className="tree-label font-medium text-[var(--text-muted)] cursor-pointer px-1"
            onClick={() => setExpanded(!expanded)}
          >
            {entry.name}
          </span>
        </div>
        {expanded &&
          entry.children?.map((c) => (
            <TreeNode key={c.path ?? c.name} entry={c} depth={depth + 1} selectedPath={selectedPath} onSelect={onSelect} />
          ))}
      </div>
    );
  }

  const isSelected = entry.path === selectedPath;
  return (
    <div
      className={`tree-row group rounded ${isSelected ? "bg-primary/20" : ""}`}
      style={{ paddingLeft }}
    >
      <span className="tree-icon-box shrink-0">
        <FileIcon name={entry.name} />
      </span>
      <button
        type="button"
        className={`tree-label text-left px-1 hover:bg-white/10 rounded ${isSelected ? "text-primary" : "text-[var(--text)]"}`}
        onClick={() => entry.path && onSelect(entry.path)}
        title={entry.path}
      >
        {entry.name}
      </button>
    </div>
  );
}

interface SourceBrowserProps {
  repoRoot?: string | null;
}

export function SourceBrowser({ repoRoot: _repoRoot }: SourceBrowserProps) {
  const [tree, setTree] = useState<SourceTreeEntry | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [content, setContent] = useState<string>("");
  const [originalContent, setOriginalContent] = useState<string>("");
  const [dirty, setDirty] = useState(false);
  const [gitLog, setGitLog] = useState<GitLogEntry[]>([]);
  const [diffText, setDiffText] = useState<string>("");
  const [branches, setBranches] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);
  const [banner, setBanner] = useState<{ msg: string; type: "success" | "error" } | null>(null);
  const [symbols, setSymbols] = useState<SymbolInfo[]>([]);
  const [showSymbols, setShowSymbols] = useState(true);

  useEffect(() => {
    invoke<SourceTreeEntry>("list_compiler_source_tree")
      .then(setTree)
      .catch((e) => setBanner({ msg: `Failed to load source tree: ${e}`, type: "error" }));
    invoke<string[]>("list_iteration_branches").then(setBranches).catch(() => {});
  }, []);

  const loadFile = useCallback(async (path: string) => {
    try {
      const text = await invoke<string>("read_compiler_file", { path });
      setContent(text);
      setOriginalContent(text);
      setDirty(false);
      setSelectedPath(path);
      setSymbols([]);
      const log = await invoke<GitLogEntry[]>("compiler_file_git_log", { path, limit: 10 });
      setGitLog(log);
      const diff = await invoke<string>("compiler_file_git_diff", { path });
      setDiffText(diff);
      invoke<SymbolInfo[]>("index_repo_file_symbols", { filePath: path })
        .then(setSymbols)
        .catch(() => setSymbols([]));
    } catch (e) {
      setBanner({ msg: String(e), type: "error" });
    }
  }, []);

  const handleSave = useCallback(async () => {
    if (!selectedPath || !dirty) return;
    setSaving(true);
    try {
      await invoke("write_compiler_file", { path: selectedPath, content });
      setOriginalContent(content);
      setDirty(false);
      setBanner({ msg: "File saved", type: "success" });
      const diff = await invoke<string>("compiler_file_git_diff", { path: selectedPath });
      setDiffText(diff);
    } catch (e) {
      setBanner({ msg: String(e), type: "error" });
    } finally {
      setSaving(false);
    }
  }, [selectedPath, content, dirty]);

  const handleRevert = useCallback(() => {
    setContent(originalContent);
    setDirty(false);
  }, [originalContent]);

  const handleCreateBranch = useCallback(async () => {
    const name = prompt("Branch name (will be prefixed with iter/):");
    if (!name) return;
    try {
      const branchName = await invoke<string>("create_iteration_branch", { name });
      setBanner({ msg: `Created branch: ${branchName}`, type: "success" });
      const b = await invoke<string[]>("list_iteration_branches");
      setBranches(b);
    } catch (e) {
      setBanner({ msg: String(e), type: "error" });
    }
  }, []);

  useEffect(() => {
    if (banner?.type === "success") {
      const tm = setTimeout(() => setBanner(null), 3000);
      return () => clearTimeout(tm);
    }
  }, [banner]);

  const moduleInfo: SourceModuleInfo | undefined = selectedPath ? getSourceModules()[selectedPath] : undefined;
  const linkedCases: string[] = [];
  if (selectedPath) {
    for (const [caseName, sources] of Object.entries(getCaseToSourceFiles())) {
      if (sources.includes(selectedPath)) {
        linkedCases.push(caseName);
      }
    }
  }

  return (
    <div className="flex flex-col h-full min-h-0 overflow-hidden">
      {banner && (
        <div className={`px-4 py-2 text-xs shrink-0 ${banner.type === "error" ? "bg-red-900/30 text-red-300" : "bg-green-900/30 text-green-300"}`}>
          {banner.msg}
        </div>
      )}
      <div className="flex flex-1 min-h-0 overflow-hidden">
        {/* Left: file tree */}
        <div className={`w-56 shrink-0 border-r border-gray-700 overflow-auto bg-[#252526]`}>
          <div className="px-3 py-2 text-xs font-medium text-[var(--text-muted)] uppercase border-b border-gray-700">
            {t("sourceBrowserTitle")}
          </div>
          {tree ? (
            tree.children?.map((c) => (
              <TreeNode key={c.path ?? c.name} entry={c} depth={0} selectedPath={selectedPath} onSelect={loadFile} />
            ))
          ) : (
            <div className="px-3 py-4 text-xs text-[var(--text-muted)]">{t("loading")}</div>
          )}
          {branches.length > 0 && (
            <div className="border-t border-gray-700 mt-2 pt-2 px-3">
              <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("iterationBranches")}</div>
              {branches.map((b) => (
                <div key={b} className="text-xs text-[var(--text)] truncate py-0.5">{b}</div>
              ))}
            </div>
          )}
        </div>

        {/* Center: editor */}
        <div className="flex-1 min-w-0 flex flex-col min-h-0">
          {selectedPath ? (
            <>
              <div className="flex items-center justify-between px-3 py-1.5 border-b border-gray-700 bg-[#2d2d2d] shrink-0">
                <span className="text-xs text-[var(--text)] font-mono truncate">{selectedPath}</span>
                <div className="flex gap-2">
                  <button
                    type="button"
                    onClick={handleSave}
                    disabled={!dirty || saving}
                    className="px-3 py-1 text-xs rounded bg-primary hover:bg-blue-600 disabled:opacity-40"
                  >
                    {t("saveFile")}
                  </button>
                  <button
                    type="button"
                    onClick={handleRevert}
                    disabled={!dirty}
                    className="px-3 py-1 text-xs rounded bg-[#3c3c3c] hover:bg-gray-600 disabled:opacity-40"
                  >
                    {t("revertFile")}
                  </button>
                  <button
                    type="button"
                    onClick={handleCreateBranch}
                    className="px-3 py-1 text-xs rounded bg-[#3c3c3c] hover:bg-gray-600"
                  >
                    {t("createBranch")}
                  </button>
                </div>
              </div>
              <div className="flex-1 min-h-0">
                <Editor
                  height="100%"
                  language="rust"
                  value={content}
                  onChange={(v) => {
                    setContent(v ?? "");
                    setDirty(v !== originalContent);
                  }}
                  theme="vs-dark"
                  options={{ minimap: { enabled: false }, scrollBeyondLastLine: false, fontSize: 13 }}
                />
              </div>
              {diffText && (
                <div className="border-t border-gray-700 max-h-32 overflow-auto shrink-0">
                  <div className="px-3 py-1 text-[10px] uppercase text-[var(--text-muted)] bg-[#2d2d2d]">Uncommitted diff</div>
                  <pre className="px-3 py-1 text-[11px] text-[var(--text-muted)] font-mono whitespace-pre-wrap">{diffText}</pre>
                </div>
              )}
            </>
          ) : (
            <div className="flex-1 flex items-center justify-center text-sm text-[var(--text-muted)]">
              {t("noFileSelected")}
            </div>
          )}
        </div>

        {/* Right: metadata */}
        <div className="w-56 shrink-0 border-l border-gray-700 overflow-auto bg-[#252526]">
          {selectedPath && (
            <>
              <div className="px-3 py-2 border-b border-gray-700">
                <button
                  type="button"
                  className="text-[10px] uppercase text-[var(--text-muted)] mb-1 hover:text-[var(--text)] w-full text-left flex items-center justify-between"
                  onClick={() => setShowSymbols(!showSymbols)}
                >
                  <span>Symbols ({symbols.length})</span>
                  <span>{showSymbols ? "\u25BC" : "\u25B6"}</span>
                </button>
                {showSymbols && (
                  symbols.length > 0 ? (
                    <div className="max-h-48 overflow-auto">
                      {symbols.map((s, i) => {
                        const kindColor: Record<string, string> = {
                          function: "text-yellow-300",
                          struct: "text-blue-300",
                          enum: "text-purple-300",
                          impl: "text-cyan-300",
                          trait: "text-green-300",
                          mod: "text-orange-300",
                          const: "text-pink-300",
                          type: "text-teal-300",
                        };
                        const color = kindColor[s.kind] || "text-[var(--text)]";
                        return (
                          <button
                            key={`${s.name}-${s.lineStart}-${i}`}
                            type="button"
                            className="flex items-center gap-1 text-[11px] py-0.5 w-full text-left hover:bg-white/10 rounded px-1"
                            title={s.signature || `${s.kind} ${s.name} (L${s.lineStart})`}
                          >
                            <span className={`text-[9px] font-mono ${color} w-4 shrink-0`}>
                              {s.kind.slice(0, 2).toUpperCase()}
                            </span>
                            <span className="text-[var(--text)] truncate">{s.name}</span>
                            <span className="text-[var(--text-muted)] text-[9px] ml-auto shrink-0">:{s.lineStart}</span>
                          </button>
                        );
                      })}
                    </div>
                  ) : (
                    <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                  )
                )}
              </div>
              <div className="px-3 py-2 border-b border-gray-700">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("gitHistory")}</div>
                {gitLog.length === 0 ? (
                  <div className="text-xs text-[var(--text-muted)]">{t("noCommitsYet")}</div>
                ) : (
                  gitLog.map((g) => (
                    <div key={g.hash} className="mb-1.5">
                      <div className="text-[11px] text-[var(--text)] truncate" title={g.message}>{g.message}</div>
                      <div className="text-[10px] text-[var(--text-muted)]">{g.short} - {g.date}</div>
                    </div>
                  ))
                )}
              </div>
              {moduleInfo && (
                <div className="px-3 py-2 border-b border-gray-700">
                  <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("linkedFeatures")}</div>
                  <div className="text-xs text-[var(--text-muted)] mb-1">{moduleInfo.description}</div>
                  {moduleInfo.features.length > 0 ? (
                    <div className="flex flex-wrap gap-1">
                      {moduleInfo.features.map((fid) => (
                        <span key={fid} className="px-1.5 py-0.5 rounded bg-blue-900/40 text-blue-300 text-[10px]">{fid}</span>
                      ))}
                    </div>
                  ) : (
                    <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                  )}
                </div>
              )}
              {linkedCases.length > 0 && (
                <div className="px-3 py-2 border-b border-gray-700">
                  <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("linkedTests")}</div>
                  <div className="flex flex-wrap gap-1">
                    {linkedCases.map((c) => (
                      <span key={c} className="px-1.5 py-0.5 rounded bg-green-900/40 text-green-300 text-[10px]">{c.replace("TestLib/", "")}</span>
                    ))}
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
