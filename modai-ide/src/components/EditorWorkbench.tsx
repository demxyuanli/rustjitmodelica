import React, { useState, useCallback, useRef, useEffect, forwardRef, useImperativeHandle } from "react";
import type monaco from "monaco-editor";
import { invoke } from "@tauri-apps/api/core";
import { readComponentTypeSource } from "../api/tauri";
import { t } from "../i18n";
import { EditorGroupColumn, type EditorGroupState } from "./EditorGroupColumn";
import type { EditorTab } from "./EditorTabBar";
import type { JitValidateResult } from "../types";

const MAX_EDITOR_GROUPS = 2;

function pathToModelName(relativePath: string): string {
  return relativePath
    .replace(/\.mo$/i, "")
    .replace(/\\/g, "/")
    .split("/")
    .filter(Boolean)
    .join(".");
}

export type DiagramViewModeRequest = "diagramReadOnly" | null;

export interface EditorWorkbenchRef {
  openFile: (relativePath: string, groupIndex?: number) => void;
  openType: (typeName: string, groupIndex?: number, libraryId?: string) => void;
  save: () => void;
  setViewModeRequest?: (mode: DiagramViewModeRequest) => void;
  getWorkspaceState: () => WorkspaceStateSnapshot | null;
}

function normalizePath(path: string): string {
  return path.replace(/\\/g, "/");
}

function tabContentKey(tab: EditorTab): string {
  return tab.projectPath != null ? normalizePath(tab.projectPath) : tab.id;
}

function tabModelName(tab: EditorTab | null | undefined): string {
  if (!tab) {
    return "BouncingBall";
  }
  if (tab.modelName) {
    return tab.modelName;
  }
  if (tab.projectPath) {
    return pathToModelName(tab.projectPath);
  }
  return "BouncingBall";
}

export interface WorkspaceStateSnapshot {
  editorGroups: EditorGroupState[];
  contentByPath: Record<string, string>;
  focusedGroupIndex: number;
  splitRatio: number;
}

export interface EditorWorkbenchProps {
  projectDir: string | null;
  gitStatus?: { modified: string[]; staged: string[] } | null;
  jitResult: JitValidateResult | null;
  modelName: string;
  setModelName: (name: string) => void;
  editorRef: React.MutableRefObject<monaco.editor.IStandaloneCodeEditor | null>;
  monacoRef: React.MutableRefObject<typeof monaco | null>;
  onFocusedChange: (params: { path: string | null; content: string }) => void;
  onCursorPositionChange?: (lineNumber: number, column: number) => void;
  onSelectionChange?: (params: { path: string | null; selectedText: string | null }) => void;
  onGitStatusChange?: (status: { modified: string[]; staged: string[] }) => void;
  onContentByPathChange?: (contentByPath: Record<string, string>) => void;
  log?: (msg: string) => void;
  focusSymbolQuery?: string | null;
  onRequestWorkbenchView?: (view: "simulation" | "analysis") => void;
  onViewModeChange?: (mode: "code" | "icon" | "diagram" | "diagramReadOnly") => void;
  libraryRefreshToken?: number;
  theme?: "dark" | "light";
  initialEditorGroups?: EditorGroupState[];
  initialContentByPath?: Record<string, string>;
  initialFocusedGroupIndex?: number;
  initialSplitRatio?: number;
  initialProjectDir?: string | null;
}

export const EditorWorkbench = forwardRef<EditorWorkbenchRef, EditorWorkbenchProps>(function EditorWorkbench(
  {
    projectDir,
    gitStatus: _gitStatus,
    jitResult,
    modelName,
    setModelName,
    editorRef,
    monacoRef,
    onFocusedChange,
    onCursorPositionChange,
    onSelectionChange,
    onGitStatusChange,
    onContentByPathChange,
    log = () => {},
    focusSymbolQuery = null,
    onRequestWorkbenchView,
    onViewModeChange,
    libraryRefreshToken = 0,
    theme = "dark",
    initialEditorGroups,
    initialContentByPath,
    initialFocusedGroupIndex = 0,
    initialSplitRatio = 0.5,
    initialProjectDir = null,
  },
  ref
) {
  const [editorGroups, setEditorGroups] = useState<EditorGroupState[]>(
    () => initialEditorGroups ?? [{ tabs: [], activeIndex: 0 }]
  );
  const [focusedGroupIndex, setFocusedGroupIndex] = useState(initialFocusedGroupIndex);
  const [viewModeRequest, setViewModeRequest] = useState<DiagramViewModeRequest>(null);
  const [contentByPath, setContentByPath] = useState<Record<string, string>>(
    () => initialContentByPath ?? {}
  );
  const [splitRatio, setSplitRatio] = useState(initialSplitRatio);
  const appliedInitialRef = useRef(false);

  useEffect(() => {
    onContentByPathChange?.(contentByPath);
  }, [contentByPath, onContentByPathChange]);

  useEffect(() => {
    const norm = (p: string) => p.replace(/\\/g, "/").trim();
    const dirMatch = projectDir && initialProjectDir && norm(projectDir) === norm(initialProjectDir);
    const hasTabs = initialEditorGroups != null && initialEditorGroups.some((g) => g.tabs?.length > 0);
    if (projectDir && dirMatch && hasTabs && !appliedInitialRef.current) {
      setEditorGroups(initialEditorGroups!);
      if (initialContentByPath != null) setContentByPath(initialContentByPath);
      setFocusedGroupIndex(initialFocusedGroupIndex);
      setSplitRatio(initialSplitRatio);
      appliedInitialRef.current = true;
    }
    if (!dirMatch) {
      appliedInitialRef.current = false;
    }
  }, [
    projectDir,
    initialProjectDir,
    initialEditorGroups,
    initialContentByPath,
    initialFocusedGroupIndex,
    initialSplitRatio,
  ]);

  const splitResizeRef = useRef<{ startX: number; startRatio: number } | null>(null);
  const editorRef0 = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const editorRef1 = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef0 = useRef<typeof monaco | null>(null);
  const monacoRef1 = useRef<typeof monaco | null>(null);
  const stateSnapshotRef = useRef<WorkspaceStateSnapshot>({
    editorGroups: [],
    contentByPath: {},
    focusedGroupIndex: 0,
    splitRatio: 0.5,
  });
  stateSnapshotRef.current = {
    editorGroups,
    contentByPath,
    focusedGroupIndex,
    splitRatio,
  };

  useEffect(() => {
    editorRef.current = focusedGroupIndex === 0 ? editorRef0.current : editorRef1.current;
    monacoRef.current = focusedGroupIndex === 0 ? monacoRef0.current : monacoRef1.current;
  }, [focusedGroupIndex, editorRef, monacoRef]);

  const focusedGroup = editorGroups[focusedGroupIndex];
  const focusedTab =
    focusedGroup?.tabs.length > 0 && focusedGroup.activeIndex >= 0 && focusedGroup.activeIndex < focusedGroup.tabs.length
      ? focusedGroup.tabs[focusedGroup.activeIndex]
      : null;
  const focusedPath = focusedTab?.projectPath ?? null;
  const focusedContent = focusedTab ? (contentByPath[tabContentKey(focusedTab)] ?? "") : "";

  useEffect(() => {
    onFocusedChange({ path: focusedPath, content: focusedContent });
    if (focusedTab) {
      setModelName(tabModelName(focusedTab));
    }
    else setModelName("BouncingBall");
  }, [focusedPath, focusedContent, focusedTab, onFocusedChange, setModelName]);

  useEffect(() => {
    if (!projectDir || !focusedTab?.projectPath) return;
    const contentKey = tabContentKey(focusedTab);
    if ((contentByPath[contentKey] ?? "") !== "") return;
    let cancelled = false;
    invoke<string>("read_project_file", { projectDir, relativePath: normalizePath(focusedTab.projectPath) })
      .then((content) => {
        if (!cancelled) setContentByPath((prev) => ({ ...prev, [contentKey]: content }));
      })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [projectDir, focusedTab, contentByPath]);

  const handleOpenFile = useCallback(
    async (relativePath: string, groupIndex?: number) => {
      if (!projectDir) return;
      const gi = groupIndex ?? focusedGroupIndex;
      const pathNorm = normalizePath(relativePath);
      const group = editorGroups[gi];
      if (group?.tabs.some((t) => normalizePath(t.projectPath ?? t.path) === pathNorm)) {
        const existing = group.tabs.findIndex((t) => normalizePath(t.projectPath ?? t.path) === pathNorm);
        setEditorGroups((prev) => prev.map((g, i) => (i === gi ? { ...g, activeIndex: existing } : g)));
        setModelName(pathToModelName(relativePath));
        if (gi !== focusedGroupIndex) setFocusedGroupIndex(gi);
        return;
      }
      try {
        const content = (await invoke("read_project_file", { projectDir, relativePath })) as string;
        setContentByPath((prev) => ({ ...prev, [pathNorm]: content }));
        setEditorGroups((prev) => {
          const g = prev[gi];
          if (!g) return prev;
          return prev.map((grp, i) =>
            i === gi
              ? {
                  ...grp,
                  tabs: [
                    ...grp.tabs,
                    { id: pathNorm, path: relativePath, dirty: false, projectPath: relativePath, readOnly: false },
                  ],
                  activeIndex: grp.tabs.length,
                }
              : grp
          );
        });
        setModelName(pathToModelName(relativePath));
        if (gi !== focusedGroupIndex) setFocusedGroupIndex(gi);
      } catch {
        // ignore
      }
    },
    [projectDir, focusedGroupIndex, editorGroups, setModelName]
  );

  const handleOpenType = useCallback(
    async (typeName: string, groupIndex?: number, libraryId?: string) => {
      if (!projectDir) return;
      const gi = groupIndex ?? focusedGroupIndex;
      try {
        const typeSource = await readComponentTypeSource(projectDir, typeName, libraryId);
        const tabId = `library://${typeSource.libraryId}/${typeSource.qualifiedName}`;
        const displayPath = typeSource.path
          ? `library/${typeSource.libraryName}/${typeSource.path}`
          : `library/${typeSource.libraryName}/${typeSource.qualifiedName.replace(/\./g, "/")}.mo`;
        const group = editorGroups[gi];
        const existing = group?.tabs.findIndex((tab) => tab.id === tabId) ?? -1;
        setContentByPath((prev) => ({ ...prev, [tabId]: typeSource.content }));
        if (existing >= 0) {
          setEditorGroups((prev) => prev.map((g, i) => (i === gi ? { ...g, activeIndex: existing } : g)));
        } else {
          setEditorGroups((prev) => {
            const current = prev[gi];
            if (!current) return prev;
            const nextTab: EditorTab = {
              id: tabId,
              path: displayPath,
              dirty: false,
              readOnly: true,
              modelName: typeSource.qualifiedName,
            };
            return prev.map((g, i) =>
              i === gi ? { ...g, tabs: [...g.tabs, nextTab], activeIndex: g.tabs.length } : g
            );
          });
        }
        setModelName(typeSource.qualifiedName);
        if (gi !== focusedGroupIndex) {
          setFocusedGroupIndex(gi);
        }
      } catch (error) {
        log("Open type error: " + String(error));
      }
    },
    [projectDir, focusedGroupIndex, editorGroups, setModelName, log]
  );

  const handleCloseTab = useCallback(
    (groupIndex: number, tabIndex: number) => {
      setEditorGroups((prev) => {
        const group = prev[groupIndex];
        if (!group) return prev;
        const tab = group.tabs[tabIndex];
        if (!tab) return prev;
        if (tab.dirty && !tab.readOnly && !window.confirm(t("unsavedChanges"))) return prev;
        const next = group.tabs.filter((_, i) => i !== tabIndex);
        const newActive =
          next.length === 0
            ? 0
            : group.activeIndex >= next.length
              ? next.length - 1
              : group.activeIndex === tabIndex
                ? Math.min(tabIndex, next.length - 1)
                : group.activeIndex > tabIndex
                  ? group.activeIndex - 1
                  : group.activeIndex;
        if (groupIndex === focusedGroupIndex && next.length > 0 && newActive < next.length) {
          setModelName(tabModelName(next[newActive]));
        } else if (groupIndex === focusedGroupIndex && next.length === 0) {
          setModelName("BouncingBall");
        }
        return prev.map((g, i) => (i === groupIndex ? { tabs: next, activeIndex: newActive } : g));
      });
    },
    [focusedGroupIndex, setModelName]
  );

  const handleSave = useCallback(
    async (groupIndex?: number) => {
      const gi = groupIndex ?? focusedGroupIndex;
      const group = editorGroups[gi];
      const tab = group?.tabs[group.activeIndex];
      const path = tab?.projectPath;
      if (!projectDir || !path || tab?.readOnly) return;
      const pathNorm = normalizePath(path);
      const content = contentByPath[pathNorm];
      if (content === undefined) return;
      try {
        await invoke("write_project_file", { projectDir, relativePath: pathNorm, content });
        setEditorGroups((prev) =>
          prev.map((g, i) =>
            i === gi
              ? {
                  ...g,
                  tabs: g.tabs.map((t) =>
                    normalizePath(t.projectPath ?? t.path) === pathNorm ? { ...t, dirty: false } : t
                  ),
                }
              : g
          )
        );
        const status = (await invoke("git_status", { projectDir })) as { modified: string[]; staged: string[] };
        onGitStatusChange?.({ modified: status.modified ?? [], staged: status.staged ?? [] });
        invoke("index_update_file", { projectDir, filePath: pathNorm }).catch(() => {});
      } catch (e) {
        log("Save error: " + String(e));
      }
    },
    [projectDir, focusedGroupIndex, editorGroups, contentByPath, log, onGitStatusChange]
  );

  useImperativeHandle(
    ref,
    () => ({
      openFile: handleOpenFile,
      openType: handleOpenType,
      save: () => handleSave(),
      setViewModeRequest,
      getWorkspaceState: () => (projectDir ? { ...stateSnapshotRef.current } : null),
    }),
    [handleOpenFile, handleOpenType, handleSave, projectDir]
  );

  const handleContentChange = useCallback(
    (groupIndex: number, value: string) => {
      const group = editorGroups[groupIndex];
      const tab = group?.tabs[group.activeIndex];
      if (!tab || tab.readOnly) return;
      const contentKey = tabContentKey(tab);
      setContentByPath((prev) => ({ ...prev, [contentKey]: value }));
      setEditorGroups((prev) =>
        prev.map((g, i) =>
          i === groupIndex ? { ...g, tabs: g.tabs.map((t, ti) => (ti === g.activeIndex ? { ...t, dirty: true } : t)) } : g
        )
      );
    },
    [editorGroups]
  );

  const handleSplit = useCallback(() => {
    setEditorGroups((prev) => {
      if (prev.length >= MAX_EDITOR_GROUPS) return prev;
      const first = prev[0];
      const activeTab = first?.tabs[first.activeIndex];
      const secondTabs = activeTab ? [{ ...activeTab }] : [];
      return [...prev, { tabs: secondTabs, activeIndex: 0 }];
    });
    setFocusedGroupIndex(1);
  }, []);

  const handleUnsplit = useCallback((groupIndex: number) => {
    if (groupIndex === 0) return;
    setEditorGroups((prev) => prev.filter((_, i) => i !== groupIndex));
    setFocusedGroupIndex(0);
    if (groupIndex === 1) {
      editorRef1.current = null;
      monacoRef1.current = null;
    }
  }, []);

  const startResizeSplit = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    splitResizeRef.current = { startX: e.clientX, startRatio: splitRatio };
    const onMove = (ev: MouseEvent) => {
      if (!splitResizeRef.current) return;
      const container = document.querySelector(".editor-workbench");
      if (!container) return;
      const rect = container.getBoundingClientRect();
      const delta = (ev.clientX - splitResizeRef.current.startX) / rect.width;
      setSplitRatio(Math.min(0.8, Math.max(0.2, splitResizeRef.current.startRatio + delta)));
    };
    const onUp = () => {
      splitResizeRef.current = null;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }, [splitRatio]);

  const isSplit = editorGroups.length === 2;

  return (
    <div
      className={`editor-workbench flex-1 min-w-0 min-h-0 flex overflow-hidden ${isSplit ? "flex-row" : "flex-col"}`}
    >
      {editorGroups.map((group, gi) => (
        <React.Fragment key={gi}>
          {isSplit && gi === 1 && (
            <div
              className="editor-workbench-resize resize-handle shrink-0 flex-shrink-0 w-1 min-w-[4px]"
              onMouseDown={startResizeSplit}
              aria-hidden
            />
          )}
          <EditorGroupColumn
            group={group}
            groupIndex={gi}
            isFocused={gi === focusedGroupIndex}
            contentByPath={contentByPath}
            projectDir={projectDir}
            pathToModelName={pathToModelName}
            showSplitButton={gi === 0 && editorGroups.length < MAX_EDITOR_GROUPS && (editorGroups[0]?.tabs.length ?? 0) > 0}
            showCloseSplitButton={gi === 1}
            flexStyle={isSplit ? { flex: gi === 0 ? `${splitRatio} 1 0%` : `${1 - splitRatio} 1 0%` } : { flex: "1 1 0%" }}
            onSelectTab={(ti) => {
              setEditorGroups((prev) => prev.map((g, i) => (i === gi ? { ...g, activeIndex: ti } : g)));
              const group = editorGroups[gi];
              if (group?.tabs[ti]) setModelName(tabModelName(group.tabs[ti]));
              setFocusedGroupIndex(gi);
            }}
            onCloseTab={(ti) => handleCloseTab(gi, ti)}
            onContentChange={(v) => handleContentChange(gi, v)}
            onSave={() => handleSave(gi)}
            onFocus={() => setFocusedGroupIndex(gi)}
            onSplit={handleSplit}
            onUnsplit={() => handleUnsplit(gi)}
            editorRef={gi === 0 ? editorRef0 : editorRef1}
            monacoRef={gi === 0 ? monacoRef0 : monacoRef1}
            modelName={modelName}
            onModelNameChange={setModelName}
            jitResult={jitResult}
            onCursorPositionChange={onCursorPositionChange}
            onSelectionChange={onSelectionChange}
            viewModeRequest={gi === focusedGroupIndex ? viewModeRequest : null}
            onViewModeRequestConsumed={gi === focusedGroupIndex ? () => setViewModeRequest(null) : undefined}
            focusSymbolQuery={gi === focusedGroupIndex ? focusSymbolQuery : null}
            onRequestWorkbenchView={onRequestWorkbenchView}
            onViewModeChange={gi === focusedGroupIndex ? onViewModeChange : undefined}
            onNavigateToType={(typeName, libraryId) => {
              void handleOpenType(typeName, undefined, libraryId);
            }}
            libraryRefreshToken={libraryRefreshToken}
            theme={theme}
          />
        </React.Fragment>
      ))}
    </div>
  );
});
