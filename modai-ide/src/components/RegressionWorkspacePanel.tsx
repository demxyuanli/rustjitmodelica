import { useCallback, useEffect, useMemo, useState } from "react";
import {
  regressionCancelWorkspace,
  regressionCreateWorkspace,
  regressionGetWorkspaceState,
  regressionListWorkspaces,
  regressionRunWorkspace,
} from "../api/tauri";
import type {
  RegressionPlanRequest,
  RegressionPlanStrategy,
  RegressionWorkspaceInfo,
  RegressionWorkspaceState,
} from "../types";
import { t, tf } from "../i18n";
import { PREFS_KEYS, readPref } from "../utils/prefsConstants";

const CATEGORY_OPTIONS = [
  "basic",
  "initialization",
  "array",
  "connect",
  "discrete",
  "algebraic",
  "solver",
  "function",
  "structure",
  "msl",
  "tooling",
  "error",
];

function pct(a: number, b: number): string {
  if (!b) return "0.0%";
  return `${((a / b) * 100).toFixed(1)}%`;
}

function statusTone(status: string): string {
  if (status === "completed") return "theme-banner-success";
  if (status === "failed" || status === "cancelled") return "theme-banner-danger";
  if (status === "running") return "theme-banner-warning";
  return "theme-button-secondary";
}

function parseCategoryFromDetail(detail: string): string {
  const marker = "category=";
  const idx = detail.indexOf(marker);
  if (idx < 0) return "unknown";
  const raw = detail.slice(idx + marker.length).trim();
  const endByPipe = raw.indexOf("|");
  const endByComma = raw.indexOf(",");
  const endBySpace = raw.indexOf(" ");
  const ends = [endByPipe, endByComma, endBySpace].filter((x) => x >= 0);
  const end = ends.length > 0 ? Math.min(...ends) : raw.length;
  const value = raw.slice(0, end).trim();
  return value || "unknown";
}

function workspaceStatusLabel(status: string): string {
  const map: Record<string, string> = {
    planned: t("regressionStatusPlanned"),
    running: t("regressionStatusRunning"),
    completed: t("regressionStatusCompleted"),
    failed: t("regressionStatusFailed"),
    cancelled: t("regressionStatusCancelled"),
  };
  return map[status] ?? status;
}

function reasonLabel(reason: string): string {
  const map: Record<string, string> = {
    expectationMet: t("regressionReasonExpectationMet"),
    modelNotFound: t("regressionReasonModelNotFound"),
    dependencyMissing: t("regressionReasonDependencyMissing"),
    newtonNonconverged: t("regressionReasonNewtonNonconverged"),
    parseError: t("regressionReasonParseError"),
    runtimeError: t("regressionReasonRuntimeError"),
    timeout: t("regressionReasonTimeout"),
    processError: t("regressionReasonProcessError"),
    cancelled: t("regressionReasonCancelled"),
  };
  return map[reason] ?? reason;
}

type RecordPreset = "latest" | "failed-triage" | "slowest";
type BatchField = "caseName" | "status" | "reason" | "category" | "exitCode" | "durationMs" | "timestamp";
type CliCaseMode = "combined" | "repeated";

const BATCH_FIELD_OPTIONS: Array<{ key: BatchField; labelKey: "name" | "status" | "regressionFilterReason" | "regressionFilterCategory" | "exitLabel" | "durationLabel" | "regressionSortTime" }> = [
  { key: "caseName", labelKey: "name" },
  { key: "status", labelKey: "status" },
  { key: "reason", labelKey: "regressionFilterReason" },
  { key: "category", labelKey: "regressionFilterCategory" },
  { key: "exitCode", labelKey: "exitLabel" },
  { key: "durationMs", labelKey: "durationLabel" },
  { key: "timestamp", labelKey: "regressionSortTime" },
];

function presetLabel(preset: RecordPreset): string {
  if (preset === "failed-triage") return t("regressionPresetFailedTriage");
  if (preset === "slowest") return t("regressionPresetSlowest");
  return t("regressionPresetLatest");
}

function recordKeyOf(r: { timestamp: string; caseName: string }): string {
  return `${r.timestamp}-${r.caseName}`;
}

export function RegressionWorkspacePanel({ theme: _theme = "dark" }: { theme?: "dark" | "light" }) {
  const [strategy, setStrategy] = useState<RegressionPlanStrategy>("relation");
  const [categories, setCategories] = useState<string[]>([]);
  const [featureIdsRaw, setFeatureIdsRaw] = useState("");
  const [changedFilesRaw, setChangedFilesRaw] = useState("");
  const [includeIndirect, setIncludeIndirect] = useState(true);
  const [maxCasesRaw, setMaxCasesRaw] = useState("");
  const [includeModelicaExamples, setIncludeModelicaExamples] = useState(true);
  const [includeModelicaTest, setIncludeModelicaTest] = useState(true);
  const [workspaceMode, setWorkspaceMode] = useState<"persistent" | "ephemeral">("persistent");

  const [workspaces, setWorkspaces] = useState<RegressionWorkspaceInfo[]>([]);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<string | null>(null);
  const [state, setState] = useState<RegressionWorkspaceState | null>(null);
  const [loading, setLoading] = useState(false);
  const [running, setRunning] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"monitor" | "stats" | "records">("monitor");
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [showFailedOnly, setShowFailedOnly] = useState(false);
  const [reasonFilter, setReasonFilter] = useState("all");
  const [statusFilter, setStatusFilter] = useState("all");
  const [categoryFilter, setCategoryFilter] = useState("all");
  const [caseQuery, setCaseQuery] = useState("");
  const [sortBy, setSortBy] = useState<"time" | "duration" | "name" | "reason" | "category">("time");
  const [sortDir, setSortDir] = useState<"asc" | "desc">("desc");
  const [failedFirst, setFailedFirst] = useState(true);
  const [recordLimit, setRecordLimit] = useState(500);
  const [activePreset, setActivePreset] = useState<RecordPreset>("latest");
  const [selectedRecordKeys, setSelectedRecordKeys] = useState<string[]>([]);
  const [pageSize, setPageSize] = useState(100);
  const [pageIndex, setPageIndex] = useState(0);
  const [lastClickedIndex, setLastClickedIndex] = useState<number | null>(null);
  const [selectedBatchFields, setSelectedBatchFields] = useState<BatchField[]>([
    "caseName",
    "status",
    "reason",
    "category",
  ]);
  const [cliCaseMode, setCliCaseMode] = useState<CliCaseMode>("combined");
  const [cliShardSizeRaw, setCliShardSizeRaw] = useState("100");
  const [cliCommandPrefix, setCliCommandPrefix] = useState("modai-worker run-batch");
  const [autoBootstrapDone, setAutoBootstrapDone] = useState(false);
  const [planCaseQuery, setPlanCaseQuery] = useState("");

  const viewPrefsKey = useMemo(
    () => `modai.regression.records.view.${selectedWorkspaceId ?? "global"}`,
    [selectedWorkspaceId]
  );
  const selectionPrefsKey = useMemo(
    () => `modai.regression.records.selection.${selectedWorkspaceId ?? "global"}`,
    [selectedWorkspaceId]
  );

  const refreshList = useCallback(async () => {
    const list = await regressionListWorkspaces();
    setWorkspaces(list);
    if (!selectedWorkspaceId && list.length > 0) setSelectedWorkspaceId(list[0].workspaceId);
  }, [selectedWorkspaceId]);

  const refreshState = useCallback(
    async (workspaceId?: string | null) => {
      const id = workspaceId ?? selectedWorkspaceId;
      if (!id) return;
      const next = await regressionGetWorkspaceState(id);
      setState(next);
    },
    [selectedWorkspaceId]
  );

  useEffect(() => {
    refreshList().catch((e) => setMessage(String(e)));
  }, [refreshList]);

  useEffect(() => {
    if (autoBootstrapDone) return;
    const autoEnabled = readPref(PREFS_KEYS.regressionAutoLoadOnOpen, (s) => s !== "false", true);
    if (!autoEnabled) {
      setAutoBootstrapDone(true);
      return;
    }
    if (workspaces.length > 0 || loading || running) {
      setAutoBootstrapDone(true);
      return;
    }
    setLoading(true);
    setMessage(null);
    const req: RegressionPlanRequest = {
      strategy: "relation",
      categories: [],
      featureIds: [],
      changedFiles: [],
      includeIndirect: true,
      maxCases: null,
      workspaceMode: "persistent",
      includeModelicaExamples: true,
      includeModelicaTest: true,
    };
    regressionCreateWorkspace(req)
      .then(async (created) => {
        setSelectedWorkspaceId(created.info.workspaceId);
        setState(created);
        await refreshList();
        setMessage(`${t("regressionAutoLoaded")} ${created.info.workspaceId}`);
      })
      .catch((e) => setMessage(String(e)))
      .finally(() => {
        setLoading(false);
        setAutoBootstrapDone(true);
      });
  }, [autoBootstrapDone, workspaces.length, loading, running, refreshList]);

  useEffect(() => {
    if (selectedWorkspaceId) {
      refreshState(selectedWorkspaceId).catch((e) => setMessage(String(e)));
    }
  }, [selectedWorkspaceId, refreshState]);

  useEffect(() => {
    if (!autoRefresh || !selectedWorkspaceId) return;
    const timer = window.setInterval(() => {
      refreshState(selectedWorkspaceId).catch(() => {});
      refreshList().catch(() => {});
    }, 3000);
    return () => window.clearInterval(timer);
  }, [autoRefresh, selectedWorkspaceId, refreshState, refreshList]);

  useEffect(() => {
    try {
      const raw = localStorage.getItem(viewPrefsKey);
      if (!raw) return;
      const p = JSON.parse(raw) as {
        showFailedOnly?: boolean;
        reasonFilter?: string;
        statusFilter?: string;
        categoryFilter?: string;
        caseQuery?: string;
        sortBy?: "time" | "duration" | "name" | "reason" | "category";
        sortDir?: "asc" | "desc";
        failedFirst?: boolean;
        recordLimit?: number;
        activePreset?: RecordPreset;
        cliCaseMode?: CliCaseMode;
        cliShardSizeRaw?: string;
        cliCommandPrefix?: string;
      };
      if (typeof p.showFailedOnly === "boolean") setShowFailedOnly(p.showFailedOnly);
      if (typeof p.reasonFilter === "string") setReasonFilter(p.reasonFilter);
      if (typeof p.statusFilter === "string") setStatusFilter(p.statusFilter);
      if (typeof p.categoryFilter === "string") setCategoryFilter(p.categoryFilter);
      if (typeof p.caseQuery === "string") setCaseQuery(p.caseQuery);
      if (p.sortBy) setSortBy(p.sortBy);
      if (p.sortDir) setSortDir(p.sortDir);
      if (typeof p.failedFirst === "boolean") setFailedFirst(p.failedFirst);
      if (typeof p.recordLimit === "number") setRecordLimit(Math.max(50, Math.min(5000, p.recordLimit)));
      if (p.activePreset) setActivePreset(p.activePreset);
      if (p.cliCaseMode) setCliCaseMode(p.cliCaseMode);
      if (typeof p.cliShardSizeRaw === "string") setCliShardSizeRaw(p.cliShardSizeRaw);
      if (typeof p.cliCommandPrefix === "string") setCliCommandPrefix(p.cliCommandPrefix);
    } catch {
      // Ignore broken persisted state.
    }
  }, [viewPrefsKey]);

  useEffect(() => {
    const payload = {
      showFailedOnly,
      reasonFilter,
      statusFilter,
      categoryFilter,
      caseQuery,
      sortBy,
      sortDir,
      failedFirst,
      recordLimit,
      activePreset,
      cliCaseMode,
      cliShardSizeRaw,
      cliCommandPrefix,
    };
    localStorage.setItem(viewPrefsKey, JSON.stringify(payload));
  }, [
    viewPrefsKey,
    showFailedOnly,
    reasonFilter,
    statusFilter,
    categoryFilter,
    caseQuery,
    sortBy,
    sortDir,
    failedFirst,
    recordLimit,
    activePreset,
    cliCaseMode,
    cliShardSizeRaw,
    cliCommandPrefix,
  ]);

  useEffect(() => {
    try {
      const raw = localStorage.getItem(selectionPrefsKey);
      if (!raw) {
        setSelectedRecordKeys([]);
        return;
      }
      const arr = JSON.parse(raw) as string[];
      if (Array.isArray(arr)) setSelectedRecordKeys(arr.filter((x) => typeof x === "string"));
      else setSelectedRecordKeys([]);
    } catch {
      setSelectedRecordKeys([]);
    }
  }, [selectionPrefsKey]);

  useEffect(() => {
    localStorage.setItem(selectionPrefsKey, JSON.stringify(selectedRecordKeys));
  }, [selectionPrefsKey, selectedRecordKeys]);

  const summary = useMemo(() => {
    if (!state?.records || state.records.length === 0) {
      return {
        total: 0,
        passed: 0,
        failed: 0,
        failByReason: [] as Array<{ key: string; count: number }>,
        passByCategory: [] as Array<{ key: string; count: number }>,
      };
    }
    const failReasonMap = new Map<string, number>();
    const passCategoryMap = new Map<string, number>();
    let passed = 0;
    let failed = 0;
    for (const r of state.records) {
      if (r.actualOk) {
        passed += 1;
        const cat = r.detail.split("category=").pop() ?? "unknown";
        passCategoryMap.set(cat, (passCategoryMap.get(cat) ?? 0) + 1);
      } else {
        failed += 1;
        failReasonMap.set(r.reason, (failReasonMap.get(r.reason) ?? 0) + 1);
      }
    }
    return {
      total: state.records.length,
      passed,
      failed,
      failByReason: Array.from(failReasonMap.entries())
        .map(([key, count]) => ({ key, count }))
        .sort((a, b) => b.count - a.count),
      passByCategory: Array.from(passCategoryMap.entries())
        .map(([key, count]) => ({ key, count }))
        .sort((a, b) => b.count - a.count),
    };
  }, [state]);

  const normalizedRecords = useMemo(() => {
    if (!state) return [];
    return state.records.map((r) => ({
      ...r,
      parsedCategory: parseCategoryFromDetail(r.detail),
    }));
  }, [state]);

  const reasonOptions = useMemo(() => {
    const set = new Set<string>();
    for (const r of normalizedRecords) set.add(r.reason);
    return Array.from(set).sort((a, b) => a.localeCompare(b));
  }, [normalizedRecords]);

  const categoryOptions = useMemo(() => {
    const set = new Set<string>();
    for (const r of normalizedRecords) set.add(r.parsedCategory);
    return Array.from(set).sort((a, b) => a.localeCompare(b));
  }, [normalizedRecords]);

  const statusOptions = useMemo(() => {
    const set = new Set<string>();
    for (const r of normalizedRecords) set.add(r.status);
    return Array.from(set).sort((a, b) => a.localeCompare(b));
  }, [normalizedRecords]);

  const visibleRecords = useMemo(() => {
    const q = caseQuery.trim().toLowerCase();
    const filtered = normalizedRecords.filter((r) => {
      if (showFailedOnly && r.actualOk) return false;
      if (reasonFilter !== "all" && r.reason !== reasonFilter) return false;
      if (statusFilter !== "all" && r.status !== statusFilter) return false;
      if (categoryFilter !== "all" && r.parsedCategory !== categoryFilter) return false;
      if (q.length > 0 && !r.caseName.toLowerCase().includes(q)) return false;
      return true;
    });

    const sorted = [...filtered].sort((a, b) => {
      if (failedFirst && a.actualOk !== b.actualOk) return a.actualOk ? 1 : -1;
      let cmp = 0;
      if (sortBy === "time") cmp = a.timestamp.localeCompare(b.timestamp);
      else if (sortBy === "duration") cmp = a.durationMs - b.durationMs;
      else if (sortBy === "name") cmp = a.caseName.localeCompare(b.caseName);
      else if (sortBy === "reason") cmp = a.reason.localeCompare(b.reason);
      else cmp = a.parsedCategory.localeCompare(b.parsedCategory);
      return sortDir === "asc" ? cmp : -cmp;
    });
    return sorted;
  }, [normalizedRecords, showFailedOnly, reasonFilter, statusFilter, categoryFilter, caseQuery, sortBy, sortDir, failedFirst]);

  const visibleSummary = useMemo(() => {
    let passed = 0;
    let failed = 0;
    for (const r of visibleRecords) {
      if (r.actualOk) passed += 1;
      else failed += 1;
    }
    return { passed, failed, total: visibleRecords.length };
  }, [visibleRecords]);

  const planCasesByCategory = useMemo(() => {
    const all = state?.plan?.plannedCases ?? [];
    const filteredByCategory = categories.length > 0 ? all.filter((x) => categories.includes(x.category)) : all;
    const q = planCaseQuery.trim().toLowerCase();
    if (!q) return filteredByCategory;
    return filteredByCategory.filter((x) => x.name.toLowerCase().includes(q));
  }, [state, categories, planCaseQuery]);

  useEffect(() => {
    setPageIndex(0);
  }, [reasonFilter, statusFilter, categoryFilter, caseQuery, sortBy, sortDir, showFailedOnly]);

  const cappedRecords = useMemo(() => visibleRecords.slice(0, recordLimit), [visibleRecords, recordLimit]);
  const totalPages = useMemo(() => Math.max(1, Math.ceil(cappedRecords.length / pageSize)), [cappedRecords.length, pageSize]);

  useEffect(() => {
    if (pageIndex >= totalPages) {
      setPageIndex(Math.max(0, totalPages - 1));
    }
  }, [pageIndex, totalPages]);

  const visibleSlice = useMemo(() => {
    const start = pageIndex * pageSize;
    return cappedRecords.slice(start, start + pageSize);
  }, [cappedRecords, pageIndex, pageSize]);

  const selectedRecords = useMemo(() => {
    const keySet = new Set(selectedRecordKeys);
    return normalizedRecords.filter((r) => keySet.has(recordKeyOf(r)));
  }, [normalizedRecords, selectedRecordKeys]);

  const selectedAgg = useMemo(() => {
    const byReason = new Map<string, number>();
    const byStatus = new Map<string, number>();
    const byCategory = new Map<string, number>();
    for (const r of selectedRecords) {
      byReason.set(r.reason, (byReason.get(r.reason) ?? 0) + 1);
      byStatus.set(r.status, (byStatus.get(r.status) ?? 0) + 1);
      byCategory.set(r.parsedCategory, (byCategory.get(r.parsedCategory) ?? 0) + 1);
    }
    const toSorted = (m: Map<string, number>) =>
      Array.from(m.entries())
        .map(([key, count]) => ({ key, count }))
        .sort((a, b) => b.count - a.count);
    return {
      reason: toSorted(byReason),
      status: toSorted(byStatus),
      category: toSorted(byCategory),
    };
  }, [selectedRecords]);

  const renderBatchField = useCallback((r: (typeof selectedRecords)[number], field: BatchField): string => {
    if (field === "caseName") return r.caseName;
    if (field === "status") return workspaceStatusLabel(r.status);
    if (field === "reason") return reasonLabel(r.reason);
    if (field === "category") return r.parsedCategory;
    if (field === "exitCode") return String(r.exitCode);
    if (field === "durationMs") return `${r.durationMs}ms`;
    return r.timestamp;
  }, []);

  const selectedBatchPreview = useMemo(() => {
    return selectedRecords.slice(0, 20).map((r) => selectedBatchFields.map((f) => renderBatchField(r, f)).join("\t"));
  }, [selectedRecords, selectedBatchFields, renderBatchField]);

  const cliShardSize = useMemo(() => {
    const n = Number(cliShardSizeRaw);
    if (!Number.isFinite(n)) return 100;
    return Math.max(1, Math.min(5000, Math.floor(n)));
  }, [cliShardSizeRaw]);

  const selectedCases = useMemo(() => selectedRecords.map((r) => r.caseName), [selectedRecords]);

  const cliCommands = useMemo(() => {
    if (selectedCases.length === 0) return [] as string[];
    const esc = (s: string) => s.replace(/"/g, "\\\"");
    const cols = selectedBatchFields.join(",");
    const base = `${cliCommandPrefix.trim() || "modai-worker run-batch"} --workspace "${esc(selectedWorkspaceId ?? "workspace")}" --columns "${cols}"`;
    const chunks: string[][] = [];
    for (let i = 0; i < selectedCases.length; i += cliShardSize) {
      chunks.push(selectedCases.slice(i, i + cliShardSize));
    }
    if (cliCaseMode === "combined") {
      return chunks.map((chunk) => `${base} --cases "${chunk.map(esc).join(";")}"`);
    }
    return chunks.map((chunk) => `${base} ${chunk.map((c) => `--case "${esc(c)}"`).join(" ")}`);
  }, [selectedCases, selectedBatchFields, selectedWorkspaceId, cliShardSize, cliCaseMode, cliCommandPrefix]);

  const applyRecordPreset = useCallback((preset: RecordPreset) => {
    setActivePreset(preset);
    setReasonFilter("all");
    setStatusFilter("all");
    setCategoryFilter("all");
    setCaseQuery("");
    if (preset === "failed-triage") {
      setShowFailedOnly(true);
      setFailedFirst(true);
      setSortBy("time");
      setSortDir("desc");
      return;
    }
    if (preset === "slowest") {
      setShowFailedOnly(false);
      setFailedFirst(true);
      setSortBy("duration");
      setSortDir("desc");
      return;
    }
    setShowFailedOnly(false);
    setFailedFirst(true);
    setSortBy("time");
    setSortDir("desc");
  }, []);

  const exportVisibleCsv = useCallback(() => {
    const rows = [
      "timestamp,caseName,category,status,reason,exitCode,durationMs,actualOk",
      ...visibleRecords.slice(0, recordLimit).map((r) =>
        [
          r.timestamp,
          JSON.stringify(r.caseName),
          JSON.stringify(r.parsedCategory),
          JSON.stringify(workspaceStatusLabel(r.status)),
          JSON.stringify(reasonLabel(r.reason)),
          String(r.exitCode),
          String(r.durationMs),
          r.actualOk ? "true" : "false",
        ].join(",")
      ),
    ];
    const blob = new Blob([rows.join("\n")], { type: "text/csv;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `regression-records-${selectedWorkspaceId ?? "workspace"}.csv`;
    a.click();
    URL.revokeObjectURL(url);
    setMessage(t("regressionExportedVisibleCsv"));
  }, [visibleRecords, recordLimit, selectedWorkspaceId]);

  const exportFailedCsv = useCallback(() => {
    const failed = cappedRecords.filter((r) => !r.actualOk);
    const rows = [
      "timestamp,caseName,category,status,reason,exitCode,durationMs,actualOk",
      ...failed.map((r) =>
        [
          r.timestamp,
          JSON.stringify(r.caseName),
          JSON.stringify(r.parsedCategory),
          JSON.stringify(workspaceStatusLabel(r.status)),
          JSON.stringify(reasonLabel(r.reason)),
          String(r.exitCode),
          String(r.durationMs),
          r.actualOk ? "true" : "false",
        ].join(",")
      ),
    ];
    const blob = new Blob([rows.join("\n")], { type: "text/csv;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `regression-failed-only-${selectedWorkspaceId ?? "workspace"}.csv`;
    a.click();
    URL.revokeObjectURL(url);
    setMessage(t("regressionExportedFailedCsv"));
  }, [cappedRecords, selectedWorkspaceId]);

  const exportSelectedCsv = useCallback(() => {
    if (selectedRecords.length === 0) {
      setMessage(t("regressionNoSelection"));
      return;
    }
    const rows = [
      "timestamp,caseName,category,status,reason,exitCode,durationMs,actualOk",
      ...selectedRecords.map((r) =>
        [
          r.timestamp,
          JSON.stringify(r.caseName),
          JSON.stringify(r.parsedCategory),
          JSON.stringify(workspaceStatusLabel(r.status)),
          JSON.stringify(reasonLabel(r.reason)),
          String(r.exitCode),
          String(r.durationMs),
          r.actualOk ? "true" : "false",
        ].join(",")
      ),
    ];
    const blob = new Blob([rows.join("\n")], { type: "text/csv;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `regression-selected-${selectedWorkspaceId ?? "workspace"}.csv`;
    a.click();
    URL.revokeObjectURL(url);
    setMessage(tf("regressionExportedSelectedCsv", { count: selectedRecords.length }));
  }, [selectedRecords, selectedWorkspaceId]);

  const exportSelectedJson = useCallback(() => {
    if (selectedRecords.length === 0) {
      setMessage(t("regressionNoSelection"));
      return;
    }
    const payload = selectedRecords.map((r) => ({
      timestamp: r.timestamp,
      caseName: r.caseName,
      category: r.parsedCategory,
      status: workspaceStatusLabel(r.status),
      reason: reasonLabel(r.reason),
      exitCode: r.exitCode,
      durationMs: r.durationMs,
      actualOk: r.actualOk,
    }));
    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `regression-selected-${selectedWorkspaceId ?? "workspace"}.json`;
    a.click();
    URL.revokeObjectURL(url);
    setMessage(tf("regressionExportedSelectedJson", { count: selectedRecords.length }));
  }, [selectedRecords, selectedWorkspaceId]);

  const exportSelectedBatchListTxt = useCallback(() => {
    if (selectedRecords.length === 0) {
      setMessage(t("regressionNoSelection"));
      return;
    }
    const lines = selectedRecords.map((r) => selectedBatchFields.map((f) => renderBatchField(r, f)).join("\t"));
    const blob = new Blob([lines.join("\n")], { type: "text/plain;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `regression-batch-list-${selectedWorkspaceId ?? "workspace"}.txt`;
    a.click();
    URL.revokeObjectURL(url);
    setMessage(tf("regressionBatchListGeneratedTxt", { count: selectedRecords.length }));
  }, [selectedRecords, selectedBatchFields, renderBatchField, selectedWorkspaceId]);

  const exportSelectedBatchListJson = useCallback(() => {
    if (selectedRecords.length === 0) {
      setMessage(t("regressionNoSelection"));
      return;
    }
    const payload = {
      workspaceId: selectedWorkspaceId ?? "workspace",
      generatedAt: new Date().toISOString(),
      total: selectedRecords.length,
      byReason: selectedAgg.reason.map((x) => ({ reason: reasonLabel(x.key), count: x.count })),
      byStatus: selectedAgg.status.map((x) => ({ status: workspaceStatusLabel(x.key), count: x.count })),
      byCategory: selectedAgg.category,
      columns: selectedBatchFields,
      records: selectedRecords.map((r) => {
        const obj: Record<string, string | number | boolean> = {};
        for (const f of selectedBatchFields) {
          if (f === "caseName") obj.caseName = r.caseName;
          else if (f === "status") obj.status = workspaceStatusLabel(r.status);
          else if (f === "reason") obj.reason = reasonLabel(r.reason);
          else if (f === "category") obj.category = r.parsedCategory;
          else if (f === "exitCode") obj.exitCode = r.exitCode;
          else if (f === "durationMs") obj.durationMs = r.durationMs;
          else obj.timestamp = r.timestamp;
        }
        return obj;
      }),
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `regression-batch-list-${selectedWorkspaceId ?? "workspace"}.json`;
    a.click();
    URL.revokeObjectURL(url);
    setMessage(tf("regressionBatchListGeneratedJson", { count: selectedRecords.length }));
  }, [selectedRecords, selectedAgg, selectedWorkspaceId, selectedBatchFields]);

  const copyBatchCliArgs = useCallback(async () => {
    if (selectedRecords.length === 0) {
      setMessage(t("regressionNoSelection"));
      return;
    }
    await navigator.clipboard.writeText(cliCommands.join("\n"));
    setMessage(tf("regressionCopiedCliArgs", { count: selectedRecords.length }));
  }, [selectedRecords, cliCommands]);

  const copySelectedCaseNames = useCallback(async () => {
    const text = selectedRecords.map((r) => r.caseName).join("\n");
    if (!text) {
      setMessage(t("regressionNoSelection"));
      return;
    }
    await navigator.clipboard.writeText(text);
    setMessage(tf("regressionCopiedCaseNames", { count: selectedRecords.length }));
  }, [selectedRecords]);

  const copySelectedFailReasons = useCallback(async () => {
    const failed = selectedRecords.filter((r) => !r.actualOk);
    const lines = failed.map((r) => `${r.caseName}\t${reasonLabel(r.reason)}`);
    if (lines.length === 0) {
      setMessage(t("regressionNoFailedSelection"));
      return;
    }
    await navigator.clipboard.writeText(lines.join("\n"));
    setMessage(tf("regressionCopiedFailReasons", { count: lines.length }));
  }, [selectedRecords]);

  const invertVisibleSelection = useCallback(() => {
    const visibleKeys = new Set(visibleSlice.map((r) => recordKeyOf(r)));
    setSelectedRecordKeys((prev) => {
      const set = new Set(prev);
      for (const k of visibleKeys) {
        if (set.has(k)) set.delete(k);
        else set.add(k);
      }
      return Array.from(set);
    });
  }, [visibleSlice]);

  const selectByReasonQuick = useCallback(() => {
    if (reasonFilter === "all") {
      setMessage(t("regressionSelectByReasonHint"));
      return;
    }
    setSelectedRecordKeys((prev) => {
      const set = new Set(prev);
      for (const r of visibleRecords) {
        if (r.reason === reasonFilter) set.add(recordKeyOf(r));
      }
      return Array.from(set);
    });
    setMessage(tf("regressionSelectedByReason", { reason: reasonLabel(reasonFilter) }));
  }, [reasonFilter, visibleRecords]);

  const selectByStatusQuick = useCallback(() => {
    if (statusFilter === "all") {
      setMessage(t("regressionSelectByStatusHint"));
      return;
    }
    setSelectedRecordKeys((prev) => {
      const set = new Set(prev);
      for (const r of visibleRecords) {
        if (r.status === statusFilter) set.add(recordKeyOf(r));
      }
      return Array.from(set);
    });
    setMessage(tf("regressionSelectedByStatus", { status: workspaceStatusLabel(statusFilter) }));
  }, [statusFilter, visibleRecords]);

  const selectAllFiltered = useCallback(() => {
    setSelectedRecordKeys((prev) => {
      const set = new Set(prev);
      for (const r of visibleRecords) set.add(recordKeyOf(r));
      return Array.from(set);
    });
    setMessage(tf("regressionSelectedAllFiltered", { count: visibleRecords.length }));
  }, [visibleRecords]);

  const unselectPassed = useCallback(() => {
    const passedSet = new Set(normalizedRecords.filter((r) => r.actualOk).map((r) => recordKeyOf(r)));
    setSelectedRecordKeys((prev) => prev.filter((k) => !passedSet.has(k)));
    setMessage(t("regressionUnselectedPassed"));
  }, [normalizedRecords]);

  const unselectFailed = useCallback(() => {
    const failedSet = new Set(normalizedRecords.filter((r) => !r.actualOk).map((r) => recordKeyOf(r)));
    setSelectedRecordKeys((prev) => prev.filter((k) => !failedSet.has(k)));
    setMessage(t("regressionUnselectedFailed"));
  }, [normalizedRecords]);

  const createPlan = useCallback(async () => {
    setLoading(true);
    setMessage(null);
    try {
      const req: RegressionPlanRequest = {
        strategy,
        categories,
        featureIds: featureIdsRaw
          .split(/[,\r\n]/)
          .map((s) => s.trim())
          .filter(Boolean),
        changedFiles: changedFilesRaw
          .split(/[\r\n]/)
          .map((s) => s.trim())
          .filter(Boolean),
        includeIndirect,
        maxCases: maxCasesRaw.trim() ? Math.max(1, Number(maxCasesRaw)) : null,
        workspaceMode,
        includeModelicaExamples,
        includeModelicaTest,
      };
      const created = await regressionCreateWorkspace(req);
      setSelectedWorkspaceId(created.info.workspaceId);
      setState(created);
      await refreshList();
      setMessage(`${t("regressionCreated")} ${created.info.workspaceId}`);
    } catch (e) {
      setMessage(String(e));
    } finally {
      setLoading(false);
    }
  }, [
    strategy,
    categories,
    featureIdsRaw,
    changedFilesRaw,
    includeIndirect,
    maxCasesRaw,
    workspaceMode,
    includeModelicaExamples,
    includeModelicaTest,
    refreshList,
  ]);

  const runCurrent = useCallback(async () => {
    if (!selectedWorkspaceId) return;
    setRunning(true);
    setMessage(null);
    try {
      const next = await regressionRunWorkspace(selectedWorkspaceId);
      setState(next);
      await refreshList();
      setMessage(`${t("regressionRunFinished")} ${next.info.status}`);
    } catch (e) {
      setMessage(String(e));
    } finally {
      setRunning(false);
    }
  }, [selectedWorkspaceId, refreshList]);

  const cancelCurrent = useCallback(async () => {
    if (!selectedWorkspaceId) return;
    setMessage(null);
    try {
      const next = await regressionCancelWorkspace(selectedWorkspaceId);
      setState(next);
      await refreshList();
      setMessage(`${t("regressionCancelled")} ${selectedWorkspaceId}`);
    } catch (e) {
      setMessage(String(e));
    }
  }, [selectedWorkspaceId, refreshList]);

  return (
    <div className="flex h-full min-h-0 w-full flex-col overflow-hidden bg-surface">
      <div className="panel-header-min-height shrink-0 border-b border-border bg-[var(--surface-elevated)] px-3 flex items-center justify-between gap-3">
        <div className="min-w-0">
          <div className="text-xs uppercase text-[var(--text-muted)]">{t("workspaceRegression")}</div>
          <div className="text-[11px] text-[var(--text-muted)] truncate">{t("testManagerDesc")}</div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <select
            value={selectedWorkspaceId ?? ""}
            onChange={(e) => setSelectedWorkspaceId(e.target.value || null)}
            className="theme-input border px-2 py-1 text-xs rounded w-72"
          >
            <option value="">{t("regressionSelectWorkspace")}</option>
            {workspaces.map((w) => (
              <option key={w.workspaceId} value={w.workspaceId}>
                {w.workspaceId} [{w.status}]
              </option>
            ))}
          </select>
          <button
            type="button"
            onClick={runCurrent}
            disabled={!selectedWorkspaceId || running}
            className="px-2.5 py-1 text-xs rounded border theme-banner-success disabled:opacity-50"
          >
            {running ? t("running") : t("run")}
          </button>
          <button
            type="button"
            onClick={() => refreshState().catch((e) => setMessage(String(e)))}
            disabled={!selectedWorkspaceId}
            className="px-2.5 py-1 text-xs rounded border theme-button-secondary disabled:opacity-50"
          >
            {t("refresh")}
          </button>
          <button
            type="button"
            onClick={cancelCurrent}
            disabled={!selectedWorkspaceId}
            className="px-2.5 py-1 text-xs rounded border theme-banner-danger disabled:opacity-50"
          >
            {t("cancel")}
          </button>
        </div>
      </div>

      <div className="flex-1 min-h-0 overflow-hidden grid grid-cols-12">
      <aside className="col-span-4 min-w-0 border-r border-border bg-[var(--panel-bg)] p-3 overflow-auto">
        <div className="text-xs uppercase text-[var(--text-muted)] mb-2">{t("regressionPlan")}</div>

        <label className="text-xs text-[var(--text-muted)]">{t("regressionStrategy")}</label>
        <select
          value={strategy}
          onChange={(e) => setStrategy(e.target.value as RegressionPlanStrategy)}
          className="w-full theme-input border px-2 py-1 text-xs rounded mb-2"
        >
          <option value="category">category</option>
          <option value="feature">feature</option>
          <option value="relation">relation</option>
        </select>

        <label className="text-xs text-[var(--text-muted)]">{t("regressionCategories")}</label>
        <div className="flex flex-wrap gap-1 mb-2">
          {CATEGORY_OPTIONS.map((c) => {
            const active = categories.includes(c);
            return (
              <button
                key={c}
                type="button"
                onClick={() =>
                  setCategories((prev) =>
                    active ? prev.filter((x) => x !== c) : [...prev, c]
                  )
                }
                className={`px-2 py-0.5 text-[10px] rounded border ${
                  active ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"
                }`}
              >
                {c}
              </button>
            );
          })}
        </div>

        <label className="text-xs text-[var(--text-muted)]">{t("regressionFeatureIds")}</label>
        <textarea
          value={featureIdsRaw}
          onChange={(e) => setFeatureIdsRaw(e.target.value)}
          rows={3}
          className="w-full theme-input border px-2 py-1 text-xs rounded mb-2"
        />

        <label className="text-xs text-[var(--text-muted)]">{t("regressionChangedFiles")}</label>
        <textarea
          value={changedFilesRaw}
          onChange={(e) => setChangedFilesRaw(e.target.value)}
          rows={4}
          className="w-full theme-input border px-2 py-1 text-xs rounded mb-2"
        />

        <div className="flex items-center gap-2 mb-2">
          <input
            id="reg-indirect"
            type="checkbox"
            checked={includeIndirect}
            onChange={(e) => setIncludeIndirect(e.target.checked)}
          />
          <label htmlFor="reg-indirect" className="text-xs text-[var(--text-muted)]">
            {t("regressionIncludeIndirect")}
          </label>
        </div>

        <div className="flex items-center gap-2 mb-2">
          <input
            id="reg-msl-examples"
            type="checkbox"
            checked={includeModelicaExamples}
            onChange={(e) => setIncludeModelicaExamples(e.target.checked)}
          />
          <label htmlFor="reg-msl-examples" className="text-xs text-[var(--text-muted)]">
            {t("regressionIncludeModelicaExamples")}
          </label>
        </div>

        <div className="flex items-center gap-2 mb-2">
          <input
            id="reg-modelica-test"
            type="checkbox"
            checked={includeModelicaTest}
            onChange={(e) => setIncludeModelicaTest(e.target.checked)}
          />
          <label htmlFor="reg-modelica-test" className="text-xs text-[var(--text-muted)]">
            {t("regressionIncludeModelicaTest")}
          </label>
        </div>

        <label className="text-xs text-[var(--text-muted)]">{t("regressionMaxCases")}</label>
        <input
          value={maxCasesRaw}
          onChange={(e) => setMaxCasesRaw(e.target.value)}
          className="w-full theme-input border px-2 py-1 text-xs rounded mb-2"
          placeholder={t("regressionMaxCasesPlaceholder")}
        />

        <label className="text-xs text-[var(--text-muted)]">{t("regressionWorkspaceMode")}</label>
        <select
          value={workspaceMode}
          onChange={(e) => setWorkspaceMode(e.target.value as "persistent" | "ephemeral")}
          className="w-full theme-input border px-2 py-1 text-xs rounded mb-3"
        >
          <option value="persistent">{t("regressionPersistent")}</option>
          <option value="ephemeral">{t("regressionEphemeral")}</option>
        </select>

        <button
          type="button"
          onClick={createPlan}
          disabled={loading}
          className="w-full px-3 py-1.5 text-xs rounded bg-primary hover:bg-blue-600 disabled:opacity-50 mb-2"
        >
          {loading ? t("regressionCreating") : t("regressionCreatePlan")}
        </button>

        {state && (
          <div className="border border-border rounded p-2 mb-2">
            <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionPlanDetails")}</div>
            <div className="text-[10px] text-[var(--text-muted)] mb-1">
              {tf("regressionPlanFilteredCount", { count: planCasesByCategory.length, total: state.plan.plannedCases.length })}
            </div>
            <input
              value={planCaseQuery}
              onChange={(e) => setPlanCaseQuery(e.target.value)}
              placeholder={t("regressionFilterCaseName")}
              className="w-full theme-input border px-2 py-1 text-xs rounded mb-2"
            />
            <div className="max-h-40 overflow-auto border border-border/40 rounded bg-[var(--surface-muted)] p-1">
              {planCasesByCategory.length === 0 ? (
                <div className="text-xs text-[var(--text-muted)] px-1 py-1">{t("none")}</div>
              ) : (
                planCasesByCategory.slice(0, 300).map((x) => (
                  <div key={`${x.name}-${x.category}`} className="text-xs py-1 px-1 border-b border-border/20 last:border-b-0">
                    <div className="font-mono truncate" title={x.name}>{x.name}</div>
                    <div className="text-[10px] text-[var(--text-muted)] truncate">{x.category} | {x.reason}</div>
                  </div>
                ))
              )}
            </div>
          </div>
        )}

        {state && (
          <div className="border border-border rounded p-2 mb-2">
            <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionChangedSources")}</div>
            <div className="max-h-24 overflow-auto">
              {state.plan.changedSources.length === 0 ? (
                <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
              ) : (
                state.plan.changedSources.slice(0, 200).map((s) => (
                  <div key={s} className="text-[10px] font-mono truncate" title={s}>{s}</div>
                ))
              )}
            </div>
          </div>
        )}

        {state && (
          <div className="border border-border rounded p-2 mb-2">
            <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionAffectedFeatures")}</div>
            <div className="max-h-24 overflow-auto">
              {state.plan.affectedFeatures.length === 0 ? (
                <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
              ) : (
                state.plan.affectedFeatures.slice(0, 200).map((f) => (
                  <div key={f} className="text-[10px] truncate" title={f}>{f}</div>
                ))
              )}
            </div>
          </div>
        )}

        <div className="border-t border-border mt-3 pt-3 flex items-center justify-between gap-2">
          <label className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
            <input type="checkbox" checked={autoRefresh} onChange={(e) => setAutoRefresh(e.target.checked)} />
            {t("refresh")} (3s)
          </label>
          <button
            type="button"
            onClick={() => refreshList().catch((e) => setMessage(String(e)))}
            className="px-2 py-1 text-xs rounded border theme-button-secondary"
          >
            {t("regressionRuns")}
          </button>
        </div>

        {message && (
          <div className="mt-3 text-xs break-all px-2 py-1.5 rounded border border-border bg-[var(--surface-muted)] text-[var(--text-muted)]">{message}</div>
        )}
      </aside>

      <main className="col-span-8 min-w-0 min-h-0 overflow-auto p-3">
        {!state ? (
          <div className="text-sm text-[var(--text-muted)]">{t("regressionNoWorkspaceSelected")}</div>
        ) : (
          <>
            <div className="grid grid-cols-4 gap-2 mb-3">
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("regressionWorkspace")}</div>
                <div className="text-xs font-mono">{state.info.workspaceId}</div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("status")}</div>
                <div className={`inline-flex px-1.5 py-0.5 rounded text-xs border ${statusTone(state.info.status)}`}>{workspaceStatusLabel(state.info.status)}</div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("regressionPlanCases")}</div>
                <div className="text-xs">{state.plan.plannedCases.length}</div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("regressionSkipped")}</div>
                <div className="text-xs">{state.plan.skippedCases.length}</div>
              </div>
            </div>

            <div className="panel-header-min-height border border-border rounded bg-[var(--surface-elevated)] px-2 flex items-center justify-between mb-3">
              <div className="flex gap-1">
                <button
                  type="button"
                  onClick={() => setActiveTab("monitor")}
                  className={`px-2.5 py-1 text-xs rounded border ${activeTab === "monitor" ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"}`}
                >
                  {t("regressionMonitor")}
                </button>
                <button
                  type="button"
                  onClick={() => setActiveTab("stats")}
                  className={`px-2.5 py-1 text-xs rounded border ${activeTab === "stats" ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"}`}
                >
                  {t("regressionStatistics")}
                </button>
                <button
                  type="button"
                  onClick={() => setActiveTab("records")}
                  className={`px-2.5 py-1 text-xs rounded border ${activeTab === "records" ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"}`}
                >
                  {t("regressionLatestRecords")}
                </button>
              </div>
              {activeTab === "records" && (
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => applyRecordPreset("failed-triage")}
                    className="px-2 py-1 text-xs rounded border theme-banner-danger"
                  >
                    {t("regressionFailedFirstView")}
                  </button>
                  <label className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
                    <input type="checkbox" checked={showFailedOnly} onChange={(e) => setShowFailedOnly(e.target.checked)} />
                    {t("testFailed")}
                  </label>
                </div>
              )}
            </div>

            {activeTab === "monitor" && (
            <div className="grid grid-cols-2 gap-3">
              <div className="border border-border rounded p-2">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionPlanCases")}</div>
                <div className="max-h-[420px] overflow-auto">
                  {planCasesByCategory.length === 0 ? (
                    <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                  ) : (
                    planCasesByCategory.slice(0, 500).map((x) => (
                      <div key={x.name} className="text-xs py-1 border-b border-border/30 last:border-b-0">
                        <div className="font-mono truncate">{x.name}</div>
                        <div className="text-[10px] text-[var(--text-muted)]">{x.reason} | p{String(x.priority)}</div>
                      </div>
                    ))
                  )}
                </div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionSkipped")}</div>
                <div className="max-h-[420px] overflow-auto">
                  {state.plan.skippedCases.length === 0 ? (
                    <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                  ) : (
                    state.plan.skippedCases.slice(0, 500).map((x) => (
                      <div key={x} className="text-xs py-1 border-b border-border/30 last:border-b-0 font-mono truncate">{x}</div>
                    ))
                  )}
                </div>
              </div>
            </div>
            )}

            {activeTab === "stats" && (
            <>
            <div className="grid grid-cols-4 gap-2 mb-3">
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("jitSummaryTotal")}</div>
                <div className="text-sm">{summary.total}</div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("testPassed")}</div>
                <div className="text-sm text-[var(--success-text)]">{summary.passed}</div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("testFailed")}</div>
                <div className="text-sm text-[var(--danger-text)]">{summary.failed}</div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("successRate")}</div>
                <div className="text-sm">{pct(summary.passed, summary.total)}</div>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="border border-border rounded p-2">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionFailByReason")}</div>
                <div className="max-h-48 overflow-auto">
                  {summary.failByReason.length === 0 ? (
                    <div className="text-xs text-[var(--text-muted)]">{t("regressionNoFailedRecord")}</div>
                  ) : (
                    summary.failByReason.map((x) => (
                      <button
                        type="button"
                        key={x.key}
                        className="w-full text-xs flex justify-between py-0.5 hover:bg-[var(--surface-muted)] rounded"
                        onClick={() => {
                          setActiveTab("records");
                          setReasonFilter(x.key);
                          setStatusFilter("all");
                          setCategoryFilter("all");
                          setCaseQuery("");
                          setShowFailedOnly(false);
                          setActivePreset("latest");
                        }}
                      >
                        <span className="truncate pr-2">{reasonLabel(x.key)}</span>
                        <span>{x.count}</span>
                      </button>
                    ))
                  )}
                </div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionPassByCategory")}</div>
                <div className="max-h-48 overflow-auto">
                  {summary.passByCategory.length === 0 ? (
                    <div className="text-xs text-[var(--text-muted)]">{t("regressionNoPassedRecord")}</div>
                  ) : (
                    summary.passByCategory.map((x) => (
                      <button
                        type="button"
                        key={x.key}
                        className="w-full text-xs flex justify-between py-0.5 hover:bg-[var(--surface-muted)] rounded"
                        onClick={() => {
                          setActiveTab("records");
                          setReasonFilter("all");
                          setStatusFilter("all");
                          setCategoryFilter(x.key);
                          setCaseQuery("");
                          setShowFailedOnly(false);
                          setActivePreset("latest");
                        }}
                      >
                        <span className="truncate pr-2">{x.key}</span>
                        <span>{x.count}</span>
                      </button>
                    ))
                  )}
                </div>
              </div>
            </div>
            </>
            )}

            {activeTab === "records" && (
            <div className="border border-border rounded p-2">
              <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionLatestRecords")}</div>
              <div className="grid grid-cols-12 gap-2 mb-2">
                <div className="col-span-3">
                  <label className="text-[10px] text-[var(--text-muted)]">{t("regressionFilterReason")}</label>
                  <select
                    value={reasonFilter}
                    onChange={(e) => setReasonFilter(e.target.value)}
                    className="w-full theme-input border px-2 py-1 text-xs rounded"
                  >
                    <option value="all">{t("all")}</option>
                    {reasonOptions.map((x) => (
                      <option key={x} value={x}>{reasonLabel(x)}</option>
                    ))}
                  </select>
                </div>
                <div className="col-span-2">
                  <label className="text-[10px] text-[var(--text-muted)]">{t("regressionFilterStatus")}</label>
                  <select
                    value={statusFilter}
                    onChange={(e) => setStatusFilter(e.target.value)}
                    className="w-full theme-input border px-2 py-1 text-xs rounded"
                  >
                    <option value="all">{t("all")}</option>
                    {statusOptions.map((x) => (
                      <option key={x} value={x}>{workspaceStatusLabel(x)}</option>
                    ))}
                  </select>
                </div>
                <div className="col-span-2">
                  <label className="text-[10px] text-[var(--text-muted)]">{t("regressionFilterCategory")}</label>
                  <select
                    value={categoryFilter}
                    onChange={(e) => setCategoryFilter(e.target.value)}
                    className="w-full theme-input border px-2 py-1 text-xs rounded"
                  >
                    <option value="all">{t("all")}</option>
                    {categoryOptions.map((x) => (
                      <option key={x} value={x}>{x}</option>
                    ))}
                  </select>
                </div>
                <div className="col-span-2">
                  <label className="text-[10px] text-[var(--text-muted)]">{t("regressionFilterCaseName")}</label>
                  <input
                    value={caseQuery}
                    onChange={(e) => setCaseQuery(e.target.value)}
                    placeholder={t("searchPlaceholder")}
                    className="w-full theme-input border px-2 py-1 text-xs rounded"
                  />
                </div>
                <div className="col-span-2">
                  <label className="text-[10px] text-[var(--text-muted)]">{t("regressionSortBy")}</label>
                  <select
                    value={sortBy}
                    onChange={(e) => setSortBy(e.target.value as "time" | "duration" | "name" | "reason" | "category")}
                    className="w-full theme-input border px-2 py-1 text-xs rounded"
                  >
                    <option value="time">{t("regressionSortTime")}</option>
                    <option value="duration">{t("regressionSortDuration")}</option>
                    <option value="name">{t("regressionSortName")}</option>
                    <option value="reason">{t("regressionSortReason")}</option>
                    <option value="category">{t("regressionSortCategory")}</option>
                  </select>
                </div>
                <div className="col-span-1">
                  <label className="text-[10px] text-[var(--text-muted)]">{t("regressionSortDirection")}</label>
                  <button
                    type="button"
                    onClick={() => setSortDir((prev) => (prev === "asc" ? "desc" : "asc"))}
                    className="w-full px-2 py-1 text-xs rounded border theme-button-secondary"
                    title={sortDir}
                  >
                    {sortDir === "asc" ? t("ascending") : t("descending")}
                  </button>
                </div>
                <div className="col-span-2">
                  <label className="text-[10px] text-[var(--text-muted)]">{t("view")}</label>
                  <button
                    type="button"
                    onClick={() => {
                      setReasonFilter("all");
                      setStatusFilter("all");
                      setCategoryFilter("all");
                      setCaseQuery("");
                      setShowFailedOnly(false);
                      setFailedFirst(true);
                      setSortBy("time");
                      setSortDir("desc");
                      setActivePreset("latest");
                      setSelectedRecordKeys([]);
                      setPageIndex(0);
                    }}
                    className="w-full px-2 py-1 text-xs rounded border theme-button-secondary"
                  >
                    {t("regressionResetFilters")}
                  </button>
                </div>
              </div>
              <div className="mb-2 flex items-center justify-between text-[10px] text-[var(--text-muted)]">
                <div>{tf("matchCount", { count: visibleRecords.length, files: 1 })}</div>
                <div className="flex items-center gap-2">
                  <span className="px-1.5 py-0.5 rounded border border-border">{tf("regressionVisiblePassed", { count: visibleSummary.passed })}</span>
                  <span className="px-1.5 py-0.5 rounded border border-border">{tf("regressionVisibleFailed", { count: visibleSummary.failed })}</span>
                  <label className="flex items-center gap-1.5">
                    <input type="checkbox" checked={failedFirst} onChange={(e) => setFailedFirst(e.target.checked)} />
                    {t("regressionFailedFirst")}
                  </label>
                </div>
              </div>
              <div className="mb-2 flex items-center gap-2">
                {(["latest", "failed-triage", "slowest"] as RecordPreset[]).map((preset) => (
                  <button
                    key={preset}
                    type="button"
                    onClick={() => applyRecordPreset(preset)}
                    className={`px-2 py-1 text-xs rounded border ${
                      activePreset === preset ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"
                    }`}
                  >
                    {presetLabel(preset)}
                  </button>
                ))}
                <div className="ml-auto flex items-center gap-2">
                  <label className="text-[10px] text-[var(--text-muted)]">{t("regressionRecordLimit")}</label>
                  <input
                    value={String(recordLimit)}
                    onChange={(e) => {
                      const n = Number(e.target.value);
                      if (!Number.isFinite(n)) return;
                      setRecordLimit(Math.max(50, Math.min(5000, Math.floor(n))));
                    }}
                    className="w-20 theme-input border px-2 py-1 text-xs rounded"
                  />
                  <button
                    type="button"
                    onClick={exportVisibleCsv}
                    className="px-2 py-1 text-xs rounded border theme-button-secondary"
                  >
                    {t("regressionExportVisibleCsv")}
                  </button>
                  <button
                    type="button"
                    onClick={exportFailedCsv}
                    className="px-2 py-1 text-xs rounded border theme-button-secondary"
                  >
                    {t("regressionExportFailedCsv")}
                  </button>
                  <label className="text-[10px] text-[var(--text-muted)]">{t("regressionPageSize")}</label>
                  <select
                    value={String(pageSize)}
                    onChange={(e) => {
                      const n = Number(e.target.value);
                      setPageSize(n);
                      setPageIndex(0);
                    }}
                    className="theme-input border px-2 py-1 text-xs rounded"
                  >
                    <option value="50">50</option>
                    <option value="100">100</option>
                    <option value="200">200</option>
                    <option value="500">500</option>
                  </select>
                </div>
              </div>
              <div className="mb-2 flex items-center gap-2">
                <button
                  type="button"
                  onClick={selectAllFiltered}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionSelectAllFiltered")}
                </button>
                <button
                  type="button"
                  onClick={() =>
                    setSelectedRecordKeys((prev) => {
                      const set = new Set(prev);
                      for (const r of visibleSlice) set.add(recordKeyOf(r));
                      return Array.from(set);
                    })
                  }
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionSelectAllVisible")}
                </button>
                <button
                  type="button"
                  onClick={() => setSelectedRecordKeys([])}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionClearSelection")}
                </button>
                <button
                  type="button"
                  onClick={unselectFailed}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionUnselectFailed")}
                </button>
                <button
                  type="button"
                  onClick={unselectPassed}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionUnselectPassed")}
                </button>
                <button
                  type="button"
                  onClick={invertVisibleSelection}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionInvertVisibleSelection")}
                </button>
                <button
                  type="button"
                  onClick={selectByReasonQuick}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionSelectByReason")}
                </button>
                <button
                  type="button"
                  onClick={selectByStatusQuick}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionSelectByStatus")}
                </button>
                <button
                  type="button"
                  onClick={() => copySelectedCaseNames().catch((e) => setMessage(String(e)))}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionCopyCaseNames")}
                </button>
                <button
                  type="button"
                  onClick={() => copySelectedFailReasons().catch((e) => setMessage(String(e)))}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionCopyFailReasons")}
                </button>
                <button
                  type="button"
                  onClick={exportSelectedCsv}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionExportSelectedCsv")}
                </button>
                <button
                  type="button"
                  onClick={exportSelectedJson}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionExportSelectedJson")}
                </button>
                <button
                  type="button"
                  onClick={exportSelectedBatchListTxt}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionGenerateBatchListTxt")}
                </button>
                <button
                  type="button"
                  onClick={exportSelectedBatchListJson}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionGenerateBatchListJson")}
                </button>
                <button
                  type="button"
                  onClick={() => copyBatchCliArgs().catch((e) => setMessage(String(e)))}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary"
                >
                  {t("regressionCopyCliArgs")}
                </button>
                <span className="text-[10px] text-[var(--text-muted)]">
                  {tf("regressionSelectedCount", { count: selectedRecords.length })}
                </span>
                <span className="text-[10px] text-[var(--text-muted)] ml-auto">
                  {tf("regressionPageIndicator", { page: pageIndex + 1, total: totalPages })}
                </span>
                <button
                  type="button"
                  disabled={pageIndex <= 0}
                  onClick={() => setPageIndex((p) => Math.max(0, p - 1))}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary disabled:opacity-50"
                >
                  {t("prevCases")}
                </button>
                <button
                  type="button"
                  disabled={pageIndex >= totalPages - 1}
                  onClick={() => setPageIndex((p) => Math.min(totalPages - 1, p + 1))}
                  className="px-2 py-1 text-xs rounded border theme-button-secondary disabled:opacity-50"
                >
                  {t("nextCases")}
                </button>
              </div>
              <div className="mb-2 grid grid-cols-3 gap-2">
                <div className="border border-border rounded p-2">
                  <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionSelectedByReason")}</div>
                  <div className="max-h-24 overflow-auto">
                    {selectedAgg.reason.length === 0 ? (
                      <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                    ) : (
                      selectedAgg.reason.map((x) => (
                        <div key={x.key} className="text-xs flex justify-between py-0.5">
                          <span className="truncate pr-2">{reasonLabel(x.key)}</span>
                          <span>{x.count}</span>
                        </div>
                      ))
                    )}
                  </div>
                </div>
                <div className="border border-border rounded p-2">
                  <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionSelectedByStatus")}</div>
                  <div className="max-h-24 overflow-auto">
                    {selectedAgg.status.length === 0 ? (
                      <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                    ) : (
                      selectedAgg.status.map((x) => (
                        <div key={x.key} className="text-xs flex justify-between py-0.5">
                          <span className="truncate pr-2">{workspaceStatusLabel(x.key)}</span>
                          <span>{x.count}</span>
                        </div>
                      ))
                    )}
                  </div>
                </div>
                <div className="border border-border rounded p-2">
                  <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionSelectedByCategory")}</div>
                  <div className="max-h-24 overflow-auto">
                    {selectedAgg.category.length === 0 ? (
                      <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                    ) : (
                      selectedAgg.category.map((x) => (
                        <div key={x.key} className="text-xs flex justify-between py-0.5">
                          <span className="truncate pr-2">{x.key}</span>
                          <span>{x.count}</span>
                        </div>
                      ))
                    )}
                  </div>
                </div>
              </div>
              <div className="mb-2 border border-border rounded p-2">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionBatchTemplateFields")}</div>
                <div className="flex flex-wrap gap-1 mb-2">
                  {BATCH_FIELD_OPTIONS.map((opt) => {
                    const active = selectedBatchFields.includes(opt.key);
                    return (
                      <button
                        key={opt.key}
                        type="button"
                        onClick={() =>
                          setSelectedBatchFields((prev) => {
                            if (active) {
                              if (prev.length <= 1) return prev;
                              return prev.filter((x) => x !== opt.key);
                            }
                            return [...prev, opt.key];
                          })
                        }
                        className={`px-2 py-0.5 text-[10px] rounded border ${
                          active ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"
                        }`}
                      >
                        {t(opt.labelKey)}
                      </button>
                    );
                  })}
                </div>
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionBatchPreview")}</div>
                <div className="max-h-24 overflow-auto border border-border/50 rounded bg-[var(--surface-muted)] p-2">
                  {selectedBatchPreview.length === 0 ? (
                    <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                  ) : (
                    selectedBatchPreview.map((line, idx) => (
                      <div key={`preview-${idx}`} className="text-[10px] font-mono truncate" title={line}>
                        {line}
                      </div>
                    ))
                  )}
                </div>
                <div className="mt-2 grid grid-cols-12 gap-2">
                  <div className="col-span-6">
                    <label className="text-[10px] text-[var(--text-muted)]">{t("regressionCliCommandPrefix")}</label>
                    <input
                      value={cliCommandPrefix}
                      onChange={(e) => setCliCommandPrefix(e.target.value)}
                      className="w-full theme-input border px-2 py-1 text-xs rounded"
                      placeholder="modai-worker run-batch"
                    />
                  </div>
                  <div className="col-span-3">
                    <label className="text-[10px] text-[var(--text-muted)]">{t("regressionCliCaseMode")}</label>
                    <select
                      value={cliCaseMode}
                      onChange={(e) => setCliCaseMode(e.target.value as CliCaseMode)}
                      className="w-full theme-input border px-2 py-1 text-xs rounded"
                    >
                      <option value="combined">{t("regressionCliCaseModeCombined")}</option>
                      <option value="repeated">{t("regressionCliCaseModeRepeated")}</option>
                    </select>
                  </div>
                  <div className="col-span-3">
                    <label className="text-[10px] text-[var(--text-muted)]">{t("regressionCliShardSize")}</label>
                    <input
                      value={cliShardSizeRaw}
                      onChange={(e) => setCliShardSizeRaw(e.target.value)}
                      className="w-full theme-input border px-2 py-1 text-xs rounded"
                    />
                  </div>
                  <div className="col-span-12 text-[10px] text-[var(--text-muted)]">
                    {tf("regressionCliShardCount", { count: cliCommands.length })}
                  </div>
                </div>
                <div className="text-[10px] uppercase text-[var(--text-muted)] mt-2 mb-1">{t("regressionCliPreview")}</div>
                <div className="max-h-24 overflow-auto border border-border/50 rounded bg-[var(--surface-muted)] p-2">
                  {cliCommands.length === 0 ? (
                    <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                  ) : (
                    cliCommands.map((line, idx) => (
                      <div key={`cli-${idx}`} className="text-[10px] font-mono truncate" title={line}>
                        {line}
                      </div>
                    ))
                  )}
                </div>
              </div>
              <div className="max-h-[520px] overflow-auto">
                {visibleRecords.length === 0 ? (
                  <div className="text-xs text-[var(--text-muted)]">{t("regressionNoRecordsYet")}</div>
                ) : (
                  <>
                  <div className="sticky top-0 z-10 grid grid-cols-12 gap-2 px-2 py-1 text-[10px] uppercase text-[var(--text-muted)] bg-[var(--surface-elevated)] border-b border-border">
                    <div className="col-span-1">{t("select")}</div>
                    <div className="col-span-4">{t("name")}</div>
                    <div className="col-span-2">{t("regressionFilterCategory")}</div>
                    <div className="col-span-2">{t("status")}</div>
                    <div className="col-span-2">{t("exitLabel")}</div>
                    <div className="col-span-1">{t("durationLabel")}</div>
                  </div>
                  {visibleSlice.map((r, localIdx) => (
                    <div key={recordKeyOf(r)} className="grid grid-cols-12 gap-2 px-2 py-1 text-xs border-b border-border/30 last:border-b-0">
                      <div className="col-span-1">
                        <input
                          type="checkbox"
                          checked={selectedRecordKeys.includes(recordKeyOf(r))}
                          onChange={(e) => {
                            const key = recordKeyOf(r);
                            const absoluteIdx = pageIndex * pageSize + localIdx;
                            const shift = (window.event as MouseEvent | undefined)?.shiftKey ?? false;
                            setSelectedRecordKeys((prev) => {
                              if (!shift || lastClickedIndex === null) {
                                if (e.target.checked) {
                                  if (prev.includes(key)) return prev;
                                  return [...prev, key];
                                }
                                return prev.filter((x) => x !== key);
                              }
                              const [a, b] = absoluteIdx >= lastClickedIndex ? [lastClickedIndex, absoluteIdx] : [absoluteIdx, lastClickedIndex];
                              const keysInRange = cappedRecords.slice(a, b + 1).map((x) => recordKeyOf(x));
                              const set = new Set(prev);
                              if (e.target.checked) {
                                for (const k of keysInRange) set.add(k);
                              } else {
                                for (const k of keysInRange) set.delete(k);
                              }
                              return Array.from(set);
                            });
                            setLastClickedIndex(absoluteIdx);
                          }}
                        />
                      </div>
                      <div className="col-span-4">
                        <div className="font-mono truncate" title={r.caseName}>{r.caseName}</div>
                        <div className="text-[10px] text-[var(--text-muted)] truncate" title={reasonLabel(r.reason)}>{reasonLabel(r.reason)}</div>
                      </div>
                      <div className="col-span-2 text-[var(--text-muted)] truncate" title={r.parsedCategory}>{r.parsedCategory}</div>
                      <div className="col-span-2">
                        <span className={r.actualOk ? "text-[var(--success-text)]" : "text-[var(--danger-text)]"}>
                          {r.actualOk ? t("testPassed") : t("testFailed")}
                        </span>
                      </div>
                      <div className="col-span-2 text-[var(--text-muted)]">{String(r.exitCode)}</div>
                      <div className="col-span-1 text-[var(--text-muted)]">{r.durationMs}ms</div>
                    </div>
                  ))
                  }
                  </>
                )}
              </div>
            </div>
            )}
          </>
        )}
      </main>
      </div>
    </div>
  );
}

export default RegressionWorkspacePanel;
