import React, { useState, useCallback, useRef, useEffect, forwardRef, useImperativeHandle } from "react";
import type monaco from "monaco-editor";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import { EditorGroupColumn, type EditorGroupState } from "./EditorGroupColumn";
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

export interface EditorWorkbenchRef {
  openFile: (relativePath: string, groupIndex?: number) => void;
  save: () => void;
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
},
  ref
) {
  const [editorGroups, setEditorGroups] = useState<EditorGroupState[]>([{ tabs: [], activeIndex: 0 }]);
  const [focusedGroupIndex, setFocusedGroupIndex] = useState(0);
  const [contentByPath, setContentByPath] = useState<Record<string, string>>({});
  useEffect(() => {
    onContentByPathChange?.(contentByPath);
  }, [contentByPath, onContentByPathChange]);
  const [splitRatio, setSplitRatio] = useState(0.5);
  const splitResizeRef = useRef<{ startX: number; startRatio: number } | null>(null);
  const editorRef0 = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const editorRef1 = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef0 = useRef<typeof monaco | null>(null);
  const monacoRef1 = useRef<typeof monaco | null>(null);

  useEffect(() => {
    editorRef.current = focusedGroupIndex === 0 ? editorRef0.current : editorRef1.current;
    monacoRef.current = focusedGroupIndex === 0 ? monacoRef0.current : monacoRef1.current;
  }, [focusedGroupIndex, editorRef, monacoRef]);

  const focusedGroup = editorGroups[focusedGroupIndex];
  const focusedPath =
    focusedGroup?.tabs.length > 0 && focusedGroup.activeIndex >= 0 && focusedGroup.activeIndex < focusedGroup.tabs.length
      ? focusedGroup.tabs[focusedGroup.activeIndex].path
      : null;
  const focusedContent = focusedPath ? (contentByPath[focusedPath.replace(/\\/g, "/")] ?? "") : "";

  useEffect(() => {
    onFocusedChange({ path: focusedPath, content: focusedContent });
    if (focusedPath) setModelName(pathToModelName(focusedPath));
    else setModelName("BouncingBall");
  }, [focusedPath, focusedContent, onFocusedChange, setModelName]);

  const handleOpenFile = useCallback(
    async (relativePath: string, groupIndex?: number) => {
      if (!projectDir) return;
      const gi = groupIndex ?? focusedGroupIndex;
      const pathNorm = relativePath.replace(/\\/g, "/");
      const group = editorGroups[gi];
      if (group?.tabs.some((t) => t.path.replace(/\\/g, "/") === pathNorm)) {
        const existing = group.tabs.findIndex((t) => t.path.replace(/\\/g, "/") === pathNorm);
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
            i === gi ? { ...grp, tabs: [...grp.tabs, { path: relativePath, dirty: false }], activeIndex: grp.tabs.length } : grp
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

  const handleCloseTab = useCallback(
    (groupIndex: number, tabIndex: number) => {
      setEditorGroups((prev) => {
        const group = prev[groupIndex];
        if (!group) return prev;
        const tab = group.tabs[tabIndex];
        if (!tab) return prev;
        if (tab.dirty && !window.confirm(t("unsavedChanges"))) return prev;
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
          setModelName(pathToModelName(next[newActive].path));
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
      const path = group?.tabs[group.activeIndex]?.path;
      if (!projectDir || !path) return;
      const pathNorm = path.replace(/\\/g, "/");
      const content = contentByPath[pathNorm];
      if (content === undefined) return;
      try {
        await invoke("write_project_file", { projectDir, relativePath: pathNorm, content });
        setEditorGroups((prev) =>
          prev.map((g, i) =>
            i === gi ? { ...g, tabs: g.tabs.map((t) => (t.path.replace(/\\/g, "/") === pathNorm ? { ...t, dirty: false } : t)) } : g
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

  useImperativeHandle(ref, () => ({ openFile: handleOpenFile, save: () => handleSave() }), [handleOpenFile, handleSave]);

  const handleContentChange = useCallback(
    (groupIndex: number, value: string) => {
      const group = editorGroups[groupIndex];
      const path = group?.tabs[group.activeIndex]?.path;
      if (!path) return;
      const pathNorm = path.replace(/\\/g, "/");
      setContentByPath((prev) => ({ ...prev, [pathNorm]: value }));
      setEditorGroups((prev) =>
        prev.map((g, i) =>
          i === groupIndex ? { ...g, tabs: g.tabs.map((t, ti) => (ti === g.activeIndex ? { ...t, dirty: true } : t)) } : g
        )
      );
    },
    [editorGroups]
  );

  const handleSplit = useCallback(() => {
    setEditorGroups((prev) => (prev.length >= MAX_EDITOR_GROUPS ? prev : [...prev, { tabs: [], activeIndex: 0 }]));
    setFocusedGroupIndex(editorGroups.length);
  }, [editorGroups.length]);

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
            showSplitButton={gi === 0 && editorGroups.length < MAX_EDITOR_GROUPS}
            showCloseSplitButton={gi === 1}
            flexStyle={isSplit ? { flex: gi === 0 ? `${splitRatio} 1 0%` : `${1 - splitRatio} 1 0%` } : { flex: "1 1 0%" }}
            onSelectTab={(ti) => {
              setEditorGroups((prev) => prev.map((g, i) => (i === gi ? { ...g, activeIndex: ti } : g)));
              const group = editorGroups[gi];
              if (group?.tabs[ti]) setModelName(pathToModelName(group.tabs[ti].path));
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
          />
        </React.Fragment>
      ))}
    </div>
  );
});
