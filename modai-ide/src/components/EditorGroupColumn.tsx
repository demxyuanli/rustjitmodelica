import React, { useState, useEffect } from "react";
import type monaco from "monaco-editor";
import { t } from "../i18n";
import { EditorTabBar, type EditorTab } from "./EditorTabBar";
import { CodeEditor } from "./CodeEditor";
import { EmptyEditorState } from "./EmptyEditorState";
import { DiagramView } from "./DiagramView";
import { IconButton } from "./IconButton";
import { AppIcon } from "./Icon";
import type { JitValidateResult } from "../types";

export interface EditorGroupState {
  tabs: EditorTab[];
  activeIndex: number;
}

interface EditorGroupColumnProps {
  group: EditorGroupState;
  groupIndex: number;
  isFocused: boolean;
  contentByPath: Record<string, string>;
  projectDir: string | null;
  pathToModelName: (path: string) => string;
  showSplitButton: boolean;
  showCloseSplitButton: boolean;
  flexStyle?: React.CSSProperties;
  onSelectTab: (ti: number) => void;
  onCloseTab: (ti: number) => void;
  onContentChange: (value: string) => void;
  onSave: () => void;
  onFocus: () => void;
  onSplit: () => void;
  onUnsplit: () => void;
  editorRef: React.MutableRefObject<monaco.editor.IStandaloneCodeEditor | null>;
  monacoRef: React.MutableRefObject<typeof monaco | null>;
  modelName: string;
  onModelNameChange: (v: string) => void;
  jitResult: JitValidateResult | null;
  onCursorPositionChange?: (lineNumber: number, column: number) => void;
  onSelectionChange?: (params: { path: string | null; selectedText: string | null }) => void;
  viewModeRequest?: "diagramReadOnly" | null;
  onViewModeRequestConsumed?: () => void;
  focusSymbolQuery?: string | null;
  onRequestWorkbenchView?: (view: "simulation" | "analysis") => void;
  onViewModeChange?: (mode: "code" | "icon" | "diagram" | "diagramReadOnly") => void;
  onNavigateToType?: (typeName: string, libraryId?: string) => void;
  libraryRefreshToken?: number;
  theme?: "dark" | "light";
}

const DEFAULT_MODEL = `model BouncingBall
  Real h(start = 1);
  Real v(start = 0);
  parameter Real g = 9.81;
  parameter Real c = 0.9;
equation
  der(h) = v;
  der(v) = -g;
  when h <= 0 then
    reinit(v, -c * pre(v));
    reinit(h, 0);
  end when;
end BouncingBall;
`;

export function EditorGroupColumn({
  group,
  groupIndex,
  isFocused,
  contentByPath,
  projectDir,
  pathToModelName,
  showSplitButton,
  showCloseSplitButton,
  flexStyle,
  onSelectTab,
  onCloseTab,
  onContentChange,
  onSave,
  onFocus,
  onSplit,
  onUnsplit,
  editorRef,
  monacoRef,
  modelName,
  onModelNameChange,
  jitResult,
  onCursorPositionChange,
  onSelectionChange,
  viewModeRequest,
  onViewModeRequestConsumed,
  focusSymbolQuery,
  onRequestWorkbenchView,
  onViewModeChange,
  onNavigateToType,
  libraryRefreshToken = 0,
  theme = "dark",
}: EditorGroupColumnProps) {
  const activeTab = group.tabs[group.activeIndex] ?? null;
  const groupPath = activeTab?.path ?? null;
  const contentKey = activeTab?.projectPath?.replace(/\\/g, "/") ?? activeTab?.id ?? null;
  const groupCode = contentKey
    ? (contentByPath[contentKey] ?? "")
    : DEFAULT_MODEL;

  const hasTabs = group.tabs.length > 0;
  const isMoFile = groupPath != null && /\.mo$/i.test(groupPath);
  const tabModelName = activeTab?.modelName ?? (activeTab?.projectPath ? pathToModelName(activeTab.projectPath) : "BouncingBall");
  const [viewMode, setViewMode] = useState<"code" | "icon" | "diagram" | "diagramReadOnly">("code");
  const diagramReadOnly = Boolean(activeTab?.readOnly) || viewMode === "diagramReadOnly";

  useEffect(() => {
    if (viewModeRequest === "diagramReadOnly" && isMoFile) {
      setViewMode("diagramReadOnly");
      onViewModeChange?.("diagramReadOnly");
      onViewModeRequestConsumed?.();
    }
  }, [viewModeRequest, isMoFile, onViewModeChange, onViewModeRequestConsumed]);

  return (
    <div
      className="editor-group-column flex min-w-0 flex-col min-h-0 overflow-hidden"
      style={flexStyle}
      onClick={onFocus}
      onFocus={onFocus}
      role="group"
      tabIndex={0}
    >
      <div
        className="editor-group-toolbar flex items-center shrink-0 w-full min-w-0 border-b border-border bg-[var(--surface-muted)]"
      >
        <EditorTabBar
          tabs={group.tabs}
          activeIndex={group.activeIndex}
          onSelectTab={onSelectTab}
          onCloseTab={onCloseTab}
        />
        {isMoFile && (
          <div className="flex items-center gap-0 overflow-hidden shrink-0" role="group" aria-label={t("viewMode")}>
            <IconButton
              icon={<span className="text-[10px] font-semibold" aria-hidden="true">&lt;/&gt;</span>}
              variant="tab"
              size="xs"
              active={viewMode === "code"}
              onClick={(e) => {
                e.stopPropagation();
                setViewMode("code");
                onViewModeChange?.("code");
              }}
              title={t("viewCode")}
              aria-label={t("viewCode")}
            />
            <IconButton
              icon={
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
                  <circle cx="12" cy="12" r="6" />
                  <path d="M12 2v4M12 18v4M2 12h4M18 12h4" />
                </svg>
              }
              variant="tab"
              size="xs"
              active={viewMode === "icon"}
              onClick={(e) => {
                e.stopPropagation();
                setViewMode("icon");
                onViewModeChange?.("icon");
              }}
              title={t("viewIcon")}
              aria-label={t("viewIcon")}
            />
            <IconButton
              icon={
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
                  <rect x="4" y="5" width="6" height="6" rx="1" />
                  <rect x="14" y="5" width="6" height="6" rx="1" />
                  <rect x="9" y="13" width="6" height="6" rx="1" />
                  <path d="M10 8h4M12 11v2" />
                </svg>
              }
              variant="tab"
              size="xs"
              active={viewMode === "diagram"}
              onClick={(e) => {
                e.stopPropagation();
                setViewMode("diagram");
                onViewModeChange?.("diagram");
              }}
              title={t("viewDiagram")}
              aria-label={t("viewDiagram")}
            />
            <IconButton
              icon={
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
                  <rect x="4" y="5" width="6" height="6" rx="1" />
                  <rect x="14" y="5" width="6" height="6" rx="1" />
                  <rect x="9" y="13" width="6" height="6" rx="1" />
                  <path d="M16 15.5a1.5 1.5 0 1 0-3 0v1.5h3z" />
                </svg>
              }
              variant="tab"
              size="xs"
              active={viewMode === "diagramReadOnly"}
              onClick={(e) => {
                e.stopPropagation();
                setViewMode("diagramReadOnly");
                onViewModeChange?.("diagramReadOnly");
              }}
              title={t("viewDiagramReadOnly")}
              aria-label={t("viewDiagramReadOnly")}
            />
            <IconButton
              icon={<AppIcon name="run" aria-hidden="true" />}
              variant="tab"
              size="xs"
              onClick={(e) => {
                e.stopPropagation();
                onRequestWorkbenchView?.("simulation");
              }}
              title={t("run")}
              aria-label={t("run")}
            />
            <IconButton
              icon={<AppIcon name="link" aria-hidden="true" />}
              variant="tab"
              size="xs"
              onClick={(e) => {
                e.stopPropagation();
                onRequestWorkbenchView?.("analysis");
              }}
              title={t("tabDependencies")}
              aria-label={t("tabDependencies")}
            />
          </div>
        )}
        {showSplitButton && (
          <IconButton
            icon={<AppIcon name="columns" aria-hidden="true" />}
            variant="ghost"
            size="xs"
            className="shrink-0"
            onClick={(e) => {
              e.stopPropagation();
              onSplit();
            }}
            title={t("splitEditor")}
            aria-label={t("splitEditor")}
          />
        )}
        {showCloseSplitButton && (
          <IconButton
            icon={<AppIcon name="close" aria-hidden="true" />}
            variant="ghost"
            size="xs"
            className="shrink-0"
            onClick={(e) => {
              e.stopPropagation();
              onUnsplit();
            }}
            title={t("closeSplit")}
            aria-label={t("closeSplit")}
          />
        )}
      </div>
      <div className="editor-group-body flex-1 min-h-0 flex flex-col min-w-0 overflow-hidden" onClick={onFocus}>
        {!hasTabs ? (
          <EmptyEditorState hasProject={!!projectDir} isSecondGroup={groupIndex === 1} />
        ) : (viewMode === "diagram" || viewMode === "diagramReadOnly" || viewMode === "icon") && isMoFile ? (
          <DiagramView
            source={groupCode}
            projectDir={projectDir}
            relativeFilePath={activeTab?.projectPath != null ? activeTab.projectPath.replace(/\\/g, "/") : null}
            onContentChange={!diagramReadOnly ? onContentChange : undefined}
            readOnly={diagramReadOnly}
            mode={viewMode === "icon" ? "icon" : "diagram"}
            focusSymbolQuery={focusSymbolQuery}
            onNavigateToType={onNavigateToType}
            libraryRefreshToken={libraryRefreshToken}
          />
        ) : (
          <CodeEditor
            value={groupCode}
            onChange={onContentChange}
            modelName={isFocused ? modelName : tabModelName}
            onModelNameChange={isFocused && !activeTab?.readOnly ? onModelNameChange : () => {}}
            modelNameReadOnly={Boolean(activeTab?.readOnly)}
            jitResult={isFocused ? jitResult : null}
            editorRef={editorRef}
            monacoRef={monacoRef}
            onCursorPositionChange={isFocused ? onCursorPositionChange : undefined}
            openFilePath={activeTab?.id ?? groupPath}
            projectDir={activeTab?.projectPath ? projectDir : null}
            onSave={activeTab?.readOnly ? undefined : onSave}
            onSelectionChange={isFocused ? onSelectionChange : undefined}
            theme={theme}
            readOnly={Boolean(activeTab?.readOnly)}
          />
        )}
      </div>
    </div>
  );
}
