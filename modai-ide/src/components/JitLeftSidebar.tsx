import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import { getSourceModules, getCaseToSourceFiles, getCaseToFeatures, type SourceModuleInfo } from "../data/jit_regression_metadata";
import { FileIcon } from "./FileIcon";
import type { JitLeftTab } from "../hooks/useJitLayout";
import { IconButton } from "./IconButton";
import { AppIcon } from "./Icon";
import { SourceControlView } from "./SourceControlView";

interface SourceTreeEntry {
  name: string;
  path?: string;
  children?: SourceTreeEntry[];
  isDir: boolean;
}

interface TestCaseInfo {
  name: string;
  path: string;
  sizeBytes: number;
  lastModified: string;
  category: string;
}

const SB_TREE_INDENT = 14;
const SB_TREE_BASE = 8;
const CATEGORIES = ["all", "basic", "initialization", "array", "connect", "discrete", "algebraic", "solver", "function", "structure", "msl", "tooling", "error"];

function TreeNode({
  entry, depth, selectedPath, onSelect,
}: {
  entry: SourceTreeEntry; depth: number; selectedPath: string | null; onSelect: (path: string) => void;
}) {
  const [expanded, setExpanded] = useState(depth < 1);
  const paddingLeft = SB_TREE_BASE + depth * SB_TREE_INDENT;

  if (entry.isDir) {
    return (
      <div className="flex flex-col">
        <div className="tree-row group rounded" style={{ paddingLeft }}>
          <button type="button" className="tree-arrow text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-[var(--surface-hover)] rounded"
            onClick={() => setExpanded(!expanded)} aria-expanded={expanded}>
            {expanded ? "\u02C5" : "\u203A"}
          </button>
          <span className="tree-label font-medium text-[var(--text-muted)] cursor-pointer px-1" onClick={() => setExpanded(!expanded)}>
            {entry.name}
          </span>
        </div>
        {expanded && entry.children?.map((c) => (
          <TreeNode key={c.path ?? c.name} entry={c} depth={depth + 1} selectedPath={selectedPath} onSelect={onSelect} />
        ))}
      </div>
    );
  }

  const isSelected = entry.path === selectedPath;
  return (
    <div className={`tree-row group rounded ${isSelected ? "bg-primary/20" : ""}`} style={{ paddingLeft }}>
      <span className="tree-icon-box shrink-0"><FileIcon name={entry.name} /></span>
      <button type="button"
        className={`tree-label text-left px-1 hover:bg-[var(--surface-hover)] rounded ${isSelected ? "text-primary" : "text-[var(--text)]"}`}
        onClick={() => entry.path && onSelect(entry.path)} title={entry.path}>
        {entry.name}
      </button>
    </div>
  );
}

interface JitLeftSidebarProps {
  activeTab: JitLeftTab;
  onTabChange: (tab: JitLeftTab) => void;
  selectedSourcePath: string | null;
  selectedTestName: string | null;
  onSelectSource: (path: string) => void;
  onSelectTest: (name: string) => void;
  onCreateTest: () => void;
  onRunSuite?: (names: string[]) => void;
  suiteRunning?: boolean;
  repoRoot?: string | null;
  onOpenDiff?: (relativePath: string, isStaged: boolean) => void;
  onOpenInEditor?: (relativePath: string) => void;
  onRefreshGitStatus?: () => void;
}

export function JitLeftSidebar({
  activeTab, onTabChange, selectedSourcePath, selectedTestName,
  onSelectSource, onSelectTest, onCreateTest, onRunSuite, suiteRunning,
  repoRoot, onOpenDiff, onOpenInEditor, onRefreshGitStatus,
}: JitLeftSidebarProps) {
  const [tree, setTree] = useState<SourceTreeEntry | null>(null);
  const [branches, setBranches] = useState<string[]>([]);
  const [testCases, setTestCases] = useState<TestCaseInfo[]>([]);
  const [categoryFilter, setCategoryFilter] = useState("all");
  const [searchQuery, setSearchQuery] = useState("");

  useEffect(() => {
    invoke<SourceTreeEntry>("list_compiler_source_tree").then(setTree).catch(() => {});
    invoke<string[]>("list_iteration_branches").then(setBranches).catch(() => {});
    invoke<TestCaseInfo[]>("list_test_library").then(setTestCases).catch(() => {});
  }, []);

  const filteredCases = testCases.filter((c) => {
    if (categoryFilter !== "all" && c.category !== categoryFilter) return false;
    if (searchQuery) return c.name.toLowerCase().includes(searchQuery.toLowerCase());
    return true;
  });

  const activeSourceModule: SourceModuleInfo | undefined = selectedSourcePath ? getSourceModules()[selectedSourcePath] : undefined;
  const linkedCasesForSource: string[] = [];
  if (selectedSourcePath) {
    for (const [caseName, sources] of Object.entries(getCaseToSourceFiles())) {
      if (sources.includes(selectedSourcePath)) linkedCasesForSource.push(caseName);
    }
  }
  const linkedFeaturesForTest = selectedTestName ? (getCaseToFeatures()[selectedTestName] ?? []) : [];
  const linkedSourcesForTest = selectedTestName ? (getCaseToSourceFiles()[selectedTestName] ?? []) : [];

  const TAB_ITEMS: { id: JitLeftTab; label: string }[] = [
    { id: "source", label: t("jitLeftSource" as Parameters<typeof t>[0]) },
    { id: "tests", label: t("jitLeftTests" as Parameters<typeof t>[0]) },
    { id: "links", label: t("jitLeftLinks" as Parameters<typeof t>[0]) },
    { id: "sourceControl", label: t("sourceControl") },
  ];

  return (
    <div className="flex flex-col h-full overflow-hidden bg-surface-alt">
      <div className="shrink-0 flex border-b border-border justify-around py-0.5">
        <IconButton
          icon={<AppIcon name="explorer" aria-hidden="true" />}
          variant="tab"
          size="xs"
          active={activeTab === "source"}
          onClick={() => onTabChange("source")}
          title={TAB_ITEMS[0]?.label ?? t("jitLeftSource" as Parameters<typeof t>[0])}
          aria-label={TAB_ITEMS[0]?.label ?? t("jitLeftSource" as Parameters<typeof t>[0])}
        />
        <IconButton
          icon={<AppIcon name="tests" aria-hidden="true" />}
          variant="tab"
          size="xs"
          active={activeTab === "tests"}
          onClick={() => onTabChange("tests")}
          title={TAB_ITEMS[1]?.label ?? t("jitLeftTests" as Parameters<typeof t>[0])}
          aria-label={TAB_ITEMS[1]?.label ?? t("jitLeftTests" as Parameters<typeof t>[0])}
        />
        <IconButton
          icon={<AppIcon name="link" aria-hidden="true" />}
          variant="tab"
          size="xs"
          active={activeTab === "links"}
          onClick={() => onTabChange("links")}
          title={TAB_ITEMS[2]?.label ?? t("jitLeftLinks" as Parameters<typeof t>[0])}
          aria-label={TAB_ITEMS[2]?.label ?? t("jitLeftLinks" as Parameters<typeof t>[0])}
        />
        <IconButton
          icon={<AppIcon name="sourceControl" aria-hidden="true" />}
          variant="tab"
          size="xs"
          active={activeTab === "sourceControl"}
          onClick={() => onTabChange("sourceControl")}
          title={t("sourceControl")}
          aria-label={t("sourceControl")}
        />
      </div>
      <div className="flex-1 min-h-0 overflow-auto scroll-vscode">
        {activeTab === "source" && (
          <div className="flex flex-col">
            {tree ? (
              tree.children?.map((c) => (
                <TreeNode key={c.path ?? c.name} entry={c} depth={0} selectedPath={selectedSourcePath} onSelect={onSelectSource} />
              ))
            ) : (
              <div className="px-3 py-4 text-xs text-[var(--text-muted)]">{t("loading")}</div>
            )}
            {branches.length > 0 && (
              <div className="border-t border-border mt-2 pt-2 px-3">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("iterationBranches")}</div>
                {branches.map((b) => (
                  <div key={b} className="text-xs text-[var(--text)] truncate py-0.5">{b}</div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === "tests" && (
          <div className="flex flex-col h-full">
            <div className="px-3 py-2 border-b border-border shrink-0">
              <input type="text" placeholder={t("search")} value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full theme-input border px-2 py-1 text-xs rounded mb-1" />
              <select value={categoryFilter} onChange={(e) => setCategoryFilter(e.target.value)}
                className="w-full theme-input border px-2 py-1 text-xs rounded text-[var(--text)]">
                {CATEGORIES.map((c) => (
                  <option key={c} value={c}>{c === "all" ? t("allCategories") : c}</option>
                ))}
              </select>
            </div>
            <div className="px-2 py-1 flex gap-1 shrink-0 border-b border-border">
              <button type="button" onClick={onCreateTest}
                className="px-2 py-0.5 text-[10px] rounded border theme-banner-success">
                + {t("createTest")}
              </button>
              {onRunSuite && (
                <button type="button"
                  onClick={() => onRunSuite(filteredCases.slice(0, 12).map((c) => c.name))}
                  disabled={suiteRunning}
                  className="px-2 py-0.5 text-[10px] rounded border theme-button-secondary disabled:opacity-50">
                  {suiteRunning ? "..." : t("runSuite")}
                </button>
              )}
            </div>
            <div className="flex-1 min-h-0 overflow-auto">
              {filteredCases.map((c) => {
                const isSelected = c.name === selectedTestName;
                return (
                  <button key={c.name} type="button"
                    className={`w-full text-left px-3 py-1 text-xs truncate ${isSelected ? "bg-primary/20 text-primary" : "hover:bg-[var(--surface-hover)] text-[var(--text)]"}`}
                    onClick={() => onSelectTest(c.name)} title={c.name}>
                    {c.name.replace("TestLib/", "")}
                    <span className="ml-1 text-[10px] text-[var(--text-muted)]">({c.category})</span>
                  </button>
                );
              })}
            </div>
            <div className="px-3 py-1 border-t border-border text-[10px] text-[var(--text-muted)] shrink-0">
              {filteredCases.length} / {testCases.length} {t("jitLeftTests" as Parameters<typeof t>[0]).toLowerCase()}
            </div>
          </div>
        )}

        {activeTab === "sourceControl" && (
          <div className="flex flex-col flex-1 min-h-0 overflow-hidden">
            <SourceControlView
              projectDir={repoRoot ?? null}
              onOpenDiff={onOpenDiff ?? (() => {})}
              onOpenInEditor={onOpenInEditor}
              onRefreshStatus={onRefreshGitStatus}
            />
          </div>
        )}

        {activeTab === "links" && (
          <div className="p-3">
            {selectedSourcePath && (
              <>
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">
                  {t("sourceBrowserTitle")}: <span className="text-[var(--text)] font-mono">{selectedSourcePath.replace("src/", "")}</span>
                </div>
                {activeSourceModule && (
                  <div className="mb-3">
                    <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("linkedFeatures")}</div>
                    {activeSourceModule.features.length > 0 ? (
                      <div className="flex flex-wrap gap-1">
                        {activeSourceModule.features.map((fid) => (
                          <span key={fid} className="px-1.5 py-0.5 rounded theme-banner-info text-[10px]">{fid}</span>
                        ))}
                      </div>
                    ) : <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>}
                  </div>
                )}
                {linkedCasesForSource.length > 0 && (
                  <div className="mb-3">
                    <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("linkedTests")}</div>
                    <div className="flex flex-wrap gap-1">
                      {linkedCasesForSource.map((c) => (
                        <span key={c} className="px-1.5 py-0.5 rounded theme-banner-success text-[10px]">{c.replace("TestLib/", "")}</span>
                      ))}
                    </div>
                  </div>
                )}
              </>
            )}
            {selectedTestName && (
              <>
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1 mt-2">
                  {t("testManagerTitle")}: <span className="text-[var(--text)]">{selectedTestName.replace("TestLib/", "")}</span>
                </div>
                <div className="mb-3">
                  <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("linkedFeatures")}</div>
                  {linkedFeaturesForTest.length > 0 ? (
                    <div className="flex flex-wrap gap-1">
                      {linkedFeaturesForTest.map((fid) => (
                        <span key={fid} className="px-1.5 py-0.5 rounded theme-banner-info text-[10px]">{fid}</span>
                      ))}
                    </div>
                  ) : <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>}
                </div>
                <div className="mb-3">
                  <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("linkedSources")}</div>
                  {linkedSourcesForTest.length > 0 ? (
                    <div className="flex flex-wrap gap-1">
                      {linkedSourcesForTest.map((s) => (
                        <span key={s} className="px-1.5 py-0.5 rounded theme-banner-warning text-[10px] truncate">{s.replace("src/", "")}</span>
                      ))}
                    </div>
                  ) : <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>}
                </div>
              </>
            )}
            {!selectedSourcePath && !selectedTestName && (
              <div className="text-xs text-[var(--text-muted)]">{t("noFileSelected")}</div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
