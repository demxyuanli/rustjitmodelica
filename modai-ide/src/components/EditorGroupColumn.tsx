import React, { useState } from "react";
import type monaco from "monaco-editor";
import { t } from "../i18n";
import { EditorTabBar } from "./EditorTabBar";
import { CodeEditor } from "./CodeEditor";
import { EmptyEditorState } from "./EmptyEditorState";
import { DiagramView } from "./DiagramView";
import type { JitValidateResult } from "../types";

export interface EditorGroupState {
  tabs: { path: string; dirty: boolean }[];
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
}: EditorGroupColumnProps) {
  const groupPath = group.tabs[group.activeIndex]?.path ?? null;
  const groupCode = groupPath
    ? (contentByPath[groupPath.replace(/\\/g, "/")] ?? "")
    : DEFAULT_MODEL;

  const hasTabs = group.tabs.length > 0;
  const isMoFile = groupPath != null && /\.mo$/i.test(groupPath);
  const [viewMode, setViewMode] = useState<"code" | "diagram">("code");

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
        className={`editor-group-toolbar flex items-center shrink-0 border-b border-border ${isFocused ? "ring-1 ring-inset ring-primary/30" : ""}`}
      >
        <EditorTabBar
          tabs={group.tabs}
          activeIndex={group.activeIndex}
          onSelectTab={onSelectTab}
          onCloseTab={onCloseTab}
        />
        {isMoFile && (
          <div className="flex rounded border border-[var(--border)] overflow-hidden shrink-0" role="group" aria-label="View mode">
            <button
              type="button"
              className={`px-2 py-1 text-xs font-medium ${viewMode === "code" ? "bg-primary text-white" : "bg-transparent text-[var(--text-muted)] hover:text-[var(--text)]"}`}
              onClick={(e) => {
                e.stopPropagation();
                setViewMode("code");
              }}
              aria-pressed={viewMode === "code"}
            >
              {t("viewCode")}
            </button>
            <button
              type="button"
              className={`px-2 py-1 text-xs font-medium ${viewMode === "diagram" ? "bg-primary text-white" : "bg-transparent text-[var(--text-muted)] hover:text-[var(--text)]"}`}
              onClick={(e) => {
                e.stopPropagation();
                setViewMode("diagram");
              }}
              aria-pressed={viewMode === "diagram"}
            >
              {t("viewDiagram")}
            </button>
          </div>
        )}
        {showSplitButton && (
          <button
            type="button"
            className="shrink-0 px-2 py-1 text-xs text-[var(--text-muted)] hover:bg-white/10 hover:text-[var(--text)]"
            onClick={(e) => {
              e.stopPropagation();
              onSplit();
            }}
            title={t("splitEditor")}
          >
            {t("splitEditor")}
          </button>
        )}
        {showCloseSplitButton && (
          <button
            type="button"
            className="shrink-0 px-2 py-1 text-xs text-[var(--text-muted)] hover:bg-white/10 hover:text-[var(--text)]"
            onClick={(e) => {
              e.stopPropagation();
              onUnsplit();
            }}
            title={t("closeSplit")}
          >
            ×
          </button>
        )}
      </div>
      <div className="editor-group-body flex-1 min-h-0 flex flex-col min-w-0 overflow-hidden" onClick={onFocus}>
        {!hasTabs ? (
          <EmptyEditorState hasProject={!!projectDir} isSecondGroup={groupIndex === 1} />
        ) : viewMode === "diagram" && isMoFile ? (
          <DiagramView
            source={groupCode}
            projectDir={projectDir}
            onContentChange={onContentChange}
            readOnly={false}
          />
        ) : (
          <CodeEditor
            value={groupCode}
            onChange={onContentChange}
            modelName={isFocused ? modelName : (groupPath ? pathToModelName(groupPath) : "BouncingBall")}
            onModelNameChange={isFocused ? onModelNameChange : () => {}}
            jitResult={isFocused ? jitResult : null}
            editorRef={editorRef}
            monacoRef={monacoRef}
            onCursorPositionChange={isFocused ? onCursorPositionChange : undefined}
            openFilePath={groupPath}
            projectDir={projectDir}
            onSave={onSave}
          />
        )}
      </div>
    </div>
  );
}
