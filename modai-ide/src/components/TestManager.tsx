import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import Editor from "@monaco-editor/react";
import { t, tf } from "../i18n";
import { getCaseToFeatures, getCaseToSourceFiles } from "../data/jit_regression_metadata";

interface TestCaseInfo {
  name: string;
  path: string;
  sizeBytes: number;
  lastModified: string;
  category: string;
}

interface TestRunResult {
  name: string;
  passed: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  durationMs: number;
}

interface TestSuiteResult {
  total: number;
  passed: number;
  failed: number;
  results: TestRunResult[];
  durationMs: number;
}

const CATEGORIES = ["all", "basic", "initialization", "array", "connect", "discrete", "algebraic", "solver", "function", "structure", "msl", "tooling", "error"];

export function TestManager({ theme = "dark" }: { theme?: "dark" | "light" }) {
  const [testCases, setTestCases] = useState<TestCaseInfo[]>([]);
  const [categoryFilter, setCategoryFilter] = useState("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedTest, setSelectedTest] = useState<string | null>(null);
  const [content, setContent] = useState("");
  const [originalContent, setOriginalContent] = useState("");
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [runResult, setRunResult] = useState<TestRunResult | null>(null);
  const [running, setRunning] = useState(false);
  const [suiteResult, setSuiteResult] = useState<TestSuiteResult | null>(null);
  const [suiteRunning, setSuiteRunning] = useState(false);
  const [banner, setBanner] = useState<{ msg: string; type: "success" | "error" } | null>(null);
  const [outputTab, setOutputTab] = useState<"stdout" | "stderr">("stdout");

  useEffect(() => {
    invoke<TestCaseInfo[]>("list_test_library")
      .then(setTestCases)
      .catch((e) => setBanner({ msg: `${t("testManagerTitle")}: ${String(e)}`, type: "error" }));
  }, []);

  useEffect(() => {
    if (banner?.type === "success") {
      const tm = setTimeout(() => setBanner(null), 3000);
      return () => clearTimeout(tm);
    }
  }, [banner]);

  const filteredCases = testCases.filter((c) => {
    if (categoryFilter !== "all" && c.category !== categoryFilter) return false;
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      return c.name.toLowerCase().includes(q);
    }
    return true;
  });

  const loadTest = useCallback(async (name: string) => {
    try {
      const text = await invoke<string>("read_test_file", { name });
      setContent(text);
      setOriginalContent(text);
      setDirty(false);
      setSelectedTest(name);
      setRunResult(null);
    } catch (e) {
      setBanner({ msg: String(e), type: "error" });
    }
  }, []);

  const handleSave = useCallback(async () => {
    if (!selectedTest || !dirty) return;
    setSaving(true);
    try {
      await invoke("write_test_file", { name: selectedTest, content });
      setOriginalContent(content);
      setDirty(false);
      setBanner({ msg: t("testFileSaved"), type: "success" });
    } catch (e) {
      setBanner({ msg: String(e), type: "error" });
    } finally {
      setSaving(false);
    }
  }, [selectedTest, content, dirty]);

  const handleRunTest = useCallback(async () => {
    if (!selectedTest) return;
    setRunning(true);
    setRunResult(null);
    try {
      const result = await invoke<TestRunResult>("run_single_test", { name: selectedTest });
      setRunResult(result);
    } catch (e) {
      setRunResult({ name: selectedTest, passed: false, exitCode: -1, stdout: "", stderr: String(e), durationMs: 0 });
    } finally {
      setRunning(false);
    }
  }, [selectedTest]);

  const handleRunSuite = useCallback(async (suite: string) => {
    setSuiteRunning(true);
    setSuiteResult(null);
    try {
      let names: string[];
      if (suite === "smoke") {
        names = filteredCases.slice(0, 12).map((c) => c.name);
      } else if (suite === "standard") {
        names = filteredCases.slice(0, 50).map((c) => c.name);
      } else {
        names = filteredCases.map((c) => c.name);
      }
      const result = await invoke<TestSuiteResult>("run_test_suite", { names, suite });
      setSuiteResult(result);
    } catch (e) {
      setBanner({ msg: String(e), type: "error" });
    } finally {
      setSuiteRunning(false);
    }
  }, [filteredCases]);

  const handleDelete = useCallback(async () => {
    if (!selectedTest) return;
    if (!confirm(tf("deleteTestConfirm", { name: selectedTest }))) return;
    try {
      await invoke("delete_test_file", { name: selectedTest });
      setSelectedTest(null);
      setContent("");
      setBanner({ msg: t("testDeleted"), type: "success" });
      const list = await invoke<TestCaseInfo[]>("list_test_library");
      setTestCases(list);
    } catch (e) {
      setBanner({ msg: String(e), type: "error" });
    }
  }, [selectedTest]);

  const handleCreateTest = useCallback(async () => {
    const name = prompt(t("createTestPrompt"));
    if (!name) return;
    const fullName = `TestLib/${name}`;
    const template = `model ${name}\n  Real x(start=0);\nequation\n  der(x) = 1;\nend ${name};\n`;
    try {
      await invoke("write_test_file", { name: fullName, content: template });
      setBanner({ msg: tf("createdTest", { name: fullName }), type: "success" });
      const list = await invoke<TestCaseInfo[]>("list_test_library");
      setTestCases(list);
      loadTest(fullName);
    } catch (e) {
      setBanner({ msg: String(e), type: "error" });
    }
  }, [loadTest]);

  const linkedFeatures = selectedTest ? (getCaseToFeatures()[selectedTest] ?? []) : [];

  return (
    <div className="flex flex-col h-full min-h-0 overflow-hidden">
      {banner && (
        <div className={`px-4 py-2 text-xs shrink-0 border-b border-border ${banner.type === "error" ? "theme-banner-danger" : "theme-banner-success"}`}>
          {banner.msg}
        </div>
      )}
      <div className="flex flex-1 min-h-0 overflow-hidden">
        {/* Left: test list */}
        <div className="w-60 shrink-0 border-r border-border overflow-hidden flex flex-col bg-[var(--panel-bg)]">
          <div className="px-3 py-2 border-b border-border shrink-0">
            <div className="text-xs font-medium text-[var(--text-muted)] uppercase mb-1">{t("testManagerTitle")}</div>
            <input
              type="text"
              placeholder={t("search")}
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full theme-input border px-2 py-1 text-xs rounded mb-1"
            />
            <select
              value={categoryFilter}
              onChange={(e) => setCategoryFilter(e.target.value)}
              className="w-full theme-input border px-2 py-1 text-xs rounded text-[var(--text)]"
            >
              {CATEGORIES.map((c) => (
                <option key={c} value={c}>{c === "all" ? t("allCategories") : c}</option>
              ))}
            </select>
          </div>
          <div className="px-2 py-1 flex gap-1 shrink-0 border-b border-border">
            <button type="button" onClick={handleCreateTest} className="px-2 py-0.5 text-[10px] rounded border theme-banner-success">
              + {t("createTest")}
            </button>
            <button
              type="button"
              onClick={() => handleRunSuite("smoke")}
              disabled={suiteRunning}
              className="px-2 py-0.5 text-[10px] rounded border theme-button-secondary disabled:opacity-50"
            >
              {suiteRunning ? "..." : t("runSuite")}
            </button>
          </div>
          <div className="flex-1 min-h-0 overflow-auto">
            {filteredCases.map((c) => {
              const isSelected = c.name === selectedTest;
              return (
                <button
                  key={c.name}
                  type="button"
                  className={`w-full text-left px-3 py-1 text-xs truncate ${isSelected ? "bg-primary/20 text-primary" : "hover:bg-[var(--surface-hover)] text-[var(--text)]"}`}
                  onClick={() => loadTest(c.name)}
                  title={c.name}
                >
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

        {/* Center: editor + output */}
        <div className="flex-1 min-w-0 flex flex-col min-h-0">
          {selectedTest ? (
            <>
              <div className="flex items-center justify-between px-3 py-1.5 border-b border-border bg-[var(--surface-elevated)] shrink-0">
                <span className="text-xs text-[var(--text)] font-mono truncate">{selectedTest}</span>
                <div className="flex gap-2">
                  <button type="button" onClick={handleRunTest} disabled={running} className="px-3 py-1 text-xs rounded border theme-banner-success disabled:opacity-50">
                    {running ? t("running") : t("runTest")}
                  </button>
                  <button type="button" onClick={handleSave} disabled={!dirty || saving} className="px-3 py-1 text-xs rounded bg-primary hover:bg-blue-600 disabled:opacity-40">
                    {t("saveFile")}
                  </button>
                  <button type="button" onClick={handleDelete} className="px-3 py-1 text-xs rounded border theme-banner-danger">
                    {t("deleteTest")}
                  </button>
                </div>
              </div>
              <div className="flex-1 min-h-0">
                <Editor
                  height="100%"
                  language="modelica"
                  value={content}
                  onChange={(v) => {
                    setContent(v ?? "");
                    setDirty(v !== originalContent);
                  }}
                  theme={theme === "light" ? "vs-light" : "vs-dark"}
                  options={{ minimap: { enabled: false }, scrollBeyondLastLine: false, fontSize: 13 }}
                />
              </div>
              {(runResult || suiteResult) && (
                <div className="border-t border-border max-h-48 overflow-auto shrink-0 bg-[var(--surface)]">
                  {runResult && (
                    <div className="p-3">
                      <div className={`text-xs font-medium mb-1 ${runResult.passed ? "text-[var(--success-text)]" : "text-[var(--danger-text)]"}`}>
                        {runResult.passed ? t("testPassed") : t("testFailed")} (exit {runResult.exitCode}, {runResult.durationMs}ms)
                      </div>
                      <div className="flex gap-2 mb-1">
                        <button type="button" onClick={() => setOutputTab("stdout")} className={`text-[10px] px-2 py-0.5 rounded border ${outputTab === "stdout" ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary text-[var(--text-muted)]"}`}>{t("stdout")}</button>
                        <button type="button" onClick={() => setOutputTab("stderr")} className={`text-[10px] px-2 py-0.5 rounded border ${outputTab === "stderr" ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary text-[var(--text-muted)]"}`}>{t("stderr")}</button>
                      </div>
                      <pre className="text-[11px] text-[var(--text-muted)] font-mono whitespace-pre-wrap max-h-24 overflow-auto">
                        {outputTab === "stdout" ? runResult.stdout || t("empty") : runResult.stderr || t("empty")}
                      </pre>
                    </div>
                  )}
                  {suiteResult && !runResult && (
                    <div className="p-3">
                      <div className="text-xs font-medium mb-1 text-[var(--text)]">
                        {t("suiteLabel")}: {suiteResult.passed}/{suiteResult.total} {t("pass").toLowerCase()} ({suiteResult.durationMs}ms)
                      </div>
                      <div className="max-h-24 overflow-auto">
                        {suiteResult.results.filter((r) => !r.passed).map((r) => (
                          <div key={r.name} className="text-[11px] text-[var(--danger-text)]">{r.name.replace("TestLib/", "")} - exit {r.exitCode}</div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              )}
            </>
          ) : (
            <div className="flex-1 flex items-center justify-center text-sm text-[var(--text-muted)]">
              {t("noTestSelected")}
            </div>
          )}
        </div>

        {/* Right: metadata */}
        <div className="w-48 shrink-0 border-l border-border overflow-auto bg-[var(--panel-bg)]">
          {selectedTest && (
            <>
              <div className="px-3 py-2 border-b border-border">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("linkedFeatures")}</div>
                {linkedFeatures.length > 0 ? (
                  <div className="flex flex-wrap gap-1">
                    {linkedFeatures.map((fid) => (
                      <span key={fid} className="px-1.5 py-0.5 rounded theme-banner-info text-[10px]">{fid}</span>
                    ))}
                  </div>
                ) : (
                  <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                )}
              </div>
              <div className="px-3 py-2 border-b border-border">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("linkedSources")}</div>
                {(() => {
                  const sources = selectedTest ? (getCaseToSourceFiles()[selectedTest] ?? []) : [];
                  return sources.length > 0 ? (
                    <div className="flex flex-wrap gap-1">
                      {sources.map((s) => (
                        <span key={s} className="px-1.5 py-0.5 rounded theme-banner-warning text-[10px] truncate">{s.replace("src/", "")}</span>
                      ))}
                    </div>
                  ) : (
                    <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                  );
                })()}
              </div>
              {suiteResult && (
                <div className="px-3 py-2 border-b border-border">
                  <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("suiteResults")}</div>
                  <div className="text-xs text-[var(--success-text)]">{suiteResult.passed} {t("pass").toLowerCase()}</div>
                  <div className="text-xs text-[var(--danger-text)]">{suiteResult.failed} {t("fail").toLowerCase()}</div>
                  <div className="text-xs text-[var(--text-muted)]">{suiteResult.durationMs}ms</div>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
