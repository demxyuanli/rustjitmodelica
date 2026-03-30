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
import type { BatchField, CliCaseMode, RecordPreset } from "./regression/regressionConstants";
import {
  parseCategoryFromDetail,
  reasonLabel,
  recordKeyOf,
  workspaceStatusLabel,
} from "./regression/regressionFormat";
import { RegressionMonitorTab } from "./regression/RegressionMonitorTab";
import { RegressionPlanSidebar } from "./regression/RegressionPlanSidebar";
import { RegressionRecordsTab } from "./regression/RegressionRecordsTab";
import { RegressionStatsTab } from "./regression/RegressionStatsTab";
import { RegressionWorkspaceHeader } from "./regression/RegressionWorkspaceHeader";
import { RegressionWorkspaceSummaryGrid } from "./regression/RegressionWorkspaceSummaryGrid";
import { RegressionWorkspaceTabChrome } from "./regression/RegressionWorkspaceTabChrome";

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

  const resetRecordFilters = useCallback(() => {
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
  }, []);

  const openRecordsForReason = useCallback((reasonKey: string) => {
    setActiveTab("records");
    setReasonFilter(reasonKey);
    setStatusFilter("all");
    setCategoryFilter("all");
    setCaseQuery("");
    setShowFailedOnly(false);
    setActivePreset("latest");
  }, []);

  const openRecordsForCategory = useCallback((categoryKey: string) => {
    setActiveTab("records");
    setReasonFilter("all");
    setStatusFilter("all");
    setCategoryFilter(categoryKey);
    setCaseQuery("");
    setShowFailedOnly(false);
    setActivePreset("latest");
  }, []);

  return (
    <div className="flex h-full min-h-0 w-full flex-col overflow-hidden bg-surface">
      <RegressionWorkspaceHeader
        workspaces={workspaces}
        selectedWorkspaceId={selectedWorkspaceId}
        onSelectWorkspace={setSelectedWorkspaceId}
        onRun={runCurrent}
        onRefresh={() => refreshState().catch((e) => setMessage(String(e)))}
        onCancel={cancelCurrent}
        running={running}
      />

      <div className="flex-1 min-h-0 overflow-hidden grid grid-cols-12">
        <RegressionPlanSidebar
          state={state}
          strategy={strategy}
          setStrategy={setStrategy}
          categories={categories}
          setCategories={setCategories}
          featureIdsRaw={featureIdsRaw}
          setFeatureIdsRaw={setFeatureIdsRaw}
          changedFilesRaw={changedFilesRaw}
          setChangedFilesRaw={setChangedFilesRaw}
          includeIndirect={includeIndirect}
          setIncludeIndirect={setIncludeIndirect}
          includeModelicaExamples={includeModelicaExamples}
          setIncludeModelicaExamples={setIncludeModelicaExamples}
          includeModelicaTest={includeModelicaTest}
          setIncludeModelicaTest={setIncludeModelicaTest}
          maxCasesRaw={maxCasesRaw}
          setMaxCasesRaw={setMaxCasesRaw}
          workspaceMode={workspaceMode}
          setWorkspaceMode={setWorkspaceMode}
          onCreatePlan={createPlan}
          loading={loading}
          planCasesByCategory={planCasesByCategory}
          planCaseQuery={planCaseQuery}
          setPlanCaseQuery={setPlanCaseQuery}
          autoRefresh={autoRefresh}
          setAutoRefresh={setAutoRefresh}
          onRefreshList={refreshList}
          message={message}
          onError={(msg) => setMessage(msg)}
        />

      <main className="col-span-8 min-w-0 min-h-0 overflow-auto p-3">
        {!state ? (
          <div className="text-sm text-[var(--text-muted)]">{t("regressionNoWorkspaceSelected")}</div>
        ) : (
          <>
            <RegressionWorkspaceSummaryGrid state={state} />

            <RegressionWorkspaceTabChrome
              activeTab={activeTab}
              onTabChange={setActiveTab}
              showFailedOnly={showFailedOnly}
              onShowFailedOnlyChange={setShowFailedOnly}
              onFailedFirstView={() => applyRecordPreset("failed-triage")}
            />

            {activeTab === "monitor" && (
              <RegressionMonitorTab
                planCasesByCategory={planCasesByCategory}
                skippedCases={state.plan.skippedCases}
              />
            )}

            {activeTab === "stats" && (
              <RegressionStatsTab
                summary={summary}
                onOpenRecordsForReason={openRecordsForReason}
                onOpenRecordsForCategory={openRecordsForCategory}
              />
            )}

            {activeTab === "records" && (
              <RegressionRecordsTab
                reasonFilter={reasonFilter}
                setReasonFilter={setReasonFilter}
                reasonOptions={reasonOptions}
                statusFilter={statusFilter}
                setStatusFilter={setStatusFilter}
                statusOptions={statusOptions}
                categoryFilter={categoryFilter}
                setCategoryFilter={setCategoryFilter}
                categoryOptions={categoryOptions}
                caseQuery={caseQuery}
                setCaseQuery={setCaseQuery}
                sortBy={sortBy}
                setSortBy={setSortBy}
                sortDir={sortDir}
                setSortDir={setSortDir}
                showFailedOnly={showFailedOnly}
                setShowFailedOnly={setShowFailedOnly}
                failedFirst={failedFirst}
                setFailedFirst={setFailedFirst}
                visibleRecords={visibleRecords}
                visibleSummary={visibleSummary}
                applyRecordPreset={applyRecordPreset}
                activePreset={activePreset}
                recordLimit={recordLimit}
                setRecordLimit={setRecordLimit}
                exportVisibleCsv={exportVisibleCsv}
                exportFailedCsv={exportFailedCsv}
                pageSize={pageSize}
                setPageSize={setPageSize}
                pageIndex={pageIndex}
                setPageIndex={setPageIndex}
                totalPages={totalPages}
                selectAllFiltered={selectAllFiltered}
                visibleSlice={visibleSlice}
                cappedRecords={cappedRecords}
                setSelectedRecordKeys={setSelectedRecordKeys}
                unselectFailed={unselectFailed}
                unselectPassed={unselectPassed}
                invertVisibleSelection={invertVisibleSelection}
                selectByReasonQuick={selectByReasonQuick}
                selectByStatusQuick={selectByStatusQuick}
                copySelectedCaseNames={copySelectedCaseNames}
                copySelectedFailReasons={copySelectedFailReasons}
                exportSelectedCsv={exportSelectedCsv}
                exportSelectedJson={exportSelectedJson}
                exportSelectedBatchListTxt={exportSelectedBatchListTxt}
                exportSelectedBatchListJson={exportSelectedBatchListJson}
                copyBatchCliArgs={copyBatchCliArgs}
                onUserError={(msg) => setMessage(msg)}
                selectedRecordsCount={selectedRecords.length}
                selectedAgg={selectedAgg}
                selectedBatchFields={selectedBatchFields}
                setSelectedBatchFields={setSelectedBatchFields}
                selectedBatchPreview={selectedBatchPreview}
                cliCommandPrefix={cliCommandPrefix}
                setCliCommandPrefix={setCliCommandPrefix}
                cliCaseMode={cliCaseMode}
                setCliCaseMode={setCliCaseMode}
                cliShardSizeRaw={cliShardSizeRaw}
                setCliShardSizeRaw={setCliShardSizeRaw}
                cliCommands={cliCommands}
                selectedRecordKeys={selectedRecordKeys}
                lastClickedIndex={lastClickedIndex}
                setLastClickedIndex={setLastClickedIndex}
                resetFilters={resetRecordFilters}
              />
            )}
          </>
        )}
      </main>
      </div>
    </div>
  );
}

export default RegressionWorkspacePanel;
