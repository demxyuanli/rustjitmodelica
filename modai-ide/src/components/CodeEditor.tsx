import { useEffect, useState } from "react";
import Editor from "@monaco-editor/react";
import type monaco from "monaco-editor";
import { t } from "../i18n";
import type { JitValidateResult } from "../types";
import { useEditorDiffDecorations } from "../hooks/useEditorDiffDecorations";

interface CodeEditorProps {
  value: string;
  onChange: (value: string) => void;
  modelName: string;
  onModelNameChange: (v: string) => void;
  jitResult: JitValidateResult | null;
  editorRef: React.MutableRefObject<monaco.editor.IStandaloneCodeEditor | null>;
  monacoRef: React.MutableRefObject<typeof monaco | null>;
  onCursorPositionChange?: (lineNumber: number, column: number) => void;
  openFilePath?: string | null;
  projectDir?: string | null;
  onSave?: () => void;
}

export function CodeEditor({
  value,
  onChange,
  modelName,
  onModelNameChange,
  jitResult,
  editorRef,
  monacoRef,
  onCursorPositionChange,
  openFilePath,
  projectDir,
  onSave,
}: CodeEditorProps) {
  const [editorReady, setEditorReady] = useState(false);
  useEffect(() => {
    const editor = editorRef.current;
    const monacoInstance = monacoRef.current;
    if (!editor || !monacoInstance || !jitResult) return;
    const model = editor.getModel();
    if (!model) return;
    const markers: monaco.editor.IMarkerData[] = [];
    for (const w of jitResult.warnings) {
      markers.push({
        severity: monacoInstance.MarkerSeverity.Warning,
        message: w.message,
        startLineNumber: w.line || 1,
        startColumn: w.column || 1,
        endLineNumber: w.line || 1,
        endColumn: Math.max((w.column || 1) + 1, 2),
      });
    }
    if (jitResult.errors.length > 0) {
      markers.push({
        severity: monacoInstance.MarkerSeverity.Error,
        message: jitResult.errors.join(" "),
        startLineNumber: 1,
        startColumn: 1,
        endLineNumber: 1,
        endColumn: 2,
      });
    }
    monacoInstance.editor.setModelMarkers(model, "rustmodlica", markers);
    return () => {
      monacoInstance.editor.setModelMarkers(model, "rustmodlica", []);
    };
  }, [jitResult, editorRef, monacoRef]);

  const pathNorm = openFilePath != null ? openFilePath.replace(/\\/g, "/") : null;
  useEditorDiffDecorations(editorRef, monacoRef, projectDir, pathNorm, value, editorReady);

  return (
    <section className="flex-1 min-w-0 flex flex-col">
      <div className="flex items-center gap-2 px-2 py-1 border-b border-border">
        <span className="text-xs text-[var(--text-muted)]">Model:</span>
        <input
          type="text"
          value={modelName}
          onChange={(e) => onModelNameChange(e.target.value)}
          className="bg-[#3c3c3c] border border-gray-600 px-2 py-1 text-sm w-40 rounded"
        />
        {openFilePath != null && onSave != null && (
          <button
            type="button"
            className="text-xs px-2 py-1 rounded bg-white/10 hover:bg-white/15 text-[var(--text)]"
            onClick={onSave}
          >
            {t("save")}
          </button>
        )}
      </div>
      <div className="flex-1 min-h-0">
        <Editor
          height="100%"
          defaultLanguage="modelica"
          path={openFilePath != null ? `file:///${openFilePath.replace(/\\/g, "/")}` : undefined}
          value={openFilePath != null ? undefined : value}
          defaultValue={openFilePath != null ? value : undefined}
          onChange={(v) => onChange(v ?? "")}
          saveViewState
          onMount={(editor, monacoInstance) => {
            editorRef.current = editor;
            monacoRef.current = monacoInstance;
            setEditorReady(true);
            const pos = editor.getPosition();
            if (pos && onCursorPositionChange) onCursorPositionChange(pos.lineNumber, pos.column);
            editor.onDidChangeCursorPosition((e) => {
              if (onCursorPositionChange) onCursorPositionChange(e.position.lineNumber, e.position.column);
            });
          }}
          theme="vs-dark"
          options={{
            minimap: { enabled: false },
            fontSize: 14,
            wordWrap: "on",
            padding: { top: 8 },
            glyphMargin: true,
          }}
          beforeMount={(monaco) => {
            monaco.languages.register({ id: "modelica" });
            monaco.languages.setMonarchTokensProvider("modelica", {
              defaultToken: "",
              tokenPostfix: ".mo",
              keywords: [
                "model", "end", "equation", "algorithm", "initial", "extends",
                "parameter", "flow", "connect", "if", "then", "else", "elseif",
                "for", "loop", "in", "while", "when", "elsewhen", "partial",
                "input", "output", "package", "constant", "terminal", "function",
                "each", "redeclare", "annotation", "assert", "terminate",
                "operator", "type", "external", "replaceable", "record", "block",
                "class", "connector", "reinit",
              ],
              typeKeywords: ["Real", "Integer", "Boolean", "String"],
              operators: ["=", ":=", "+", "-", "*", "/", "^", "and", "or", "not"],
              tokenizer: {
                root: [
                  [/\b(parameter|constant|flow|discrete|input|output)\b/, "keyword"],
                  [/\b(model|block|class|connector|record|package|function|operator)\b/, "keyword"],
                  [/\b(equation|algorithm|initial|extends|each|redeclare)\b/, "keyword"],
                  [/\b(if|then|else|elseif|for|loop|in|while|when|elsewhen)\b/, "keyword"],
                  [/\b(connect|reinit|assert|terminate|annotation|external)\b/, "keyword"],
                  [/\b(end|partial|replaceable|type)\b/, "keyword"],
                  [/\b(der|pre)\s*\(/, "keyword"],
                  [/\b(Real|Integer|Boolean|String)\b/, "type"],
                  [/"[^"]*"/, "string"],
                  [/\/\/.*$/, "comment"],
                  [/\/\*/, "comment", "@comment"],
                  [/\d+\.?\d*([eE][+-]?\d+)?/, "number"],
                  [/[{}()\[\];,]/, "delimiter"],
                  [/[=:]/, "operator"],
                  [/[+\-*\/^]/, "operator"],
                  [/\b(and|or|not)\b/, "operator"],
                ],
                comment: [
                  [/[^\/*]+/, "comment"],
                  [/\*\//, "comment", "@pop"],
                  [/[\/*]/, "comment"],
                ],
              },
            });
            monaco.editor.defineTheme("modai-dark", {
              base: "vs-dark",
              inherit: true,
              rules: [],
              colors: {},
            });
          }}
        />
      </div>
    </section>
  );
}
