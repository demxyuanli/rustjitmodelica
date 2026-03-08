import { useEffect, useRef, useCallback } from "react";
import type monaco from "monaco-editor";
import { invoke } from "@tauri-apps/api/core";
import { createTwoFilesPatch } from "diff";

/** Line numbers in result are in the NEW (current) file. Modified = new line replaced old content. */
export function parseUnifiedDiffToLineRanges(diffText: string): {
  addedLines: number[];
  modifiedLines: number[];
} {
  const lines = diffText.split(/\r?\n/);
  const hunks: { newLineNumbers: number[]; removedCount: number }[] = [];
  let newLineNum = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.startsWith("@@")) {
      const m = line.match(/@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
      if (m) newLineNum = parseInt(m[1], 10);
      hunks.push({ newLineNumbers: [], removedCount: 0 });
      continue;
    }
    const hunk = hunks[hunks.length - 1];
    if (!hunk) continue;
    if (line.startsWith("+") && !line.startsWith("+++")) {
      hunk.newLineNumbers.push(newLineNum);
      newLineNum++;
    } else if (line.startsWith("-") && !line.startsWith("---")) {
      hunk.removedCount++;
    } else if (!line.startsWith("\\")) {
      newLineNum++;
    }
  }

  const addedLines: number[] = [];
  const modifiedLines: number[] = [];
  for (const h of hunks) {
    const k = Math.min(h.removedCount, h.newLineNumbers.length);
    for (let j = 0; j < h.newLineNumbers.length; j++) {
      if (j < k) modifiedLines.push(h.newLineNumbers[j]);
      else addedLines.push(h.newLineNumbers[j]);
    }
  }
  return { addedLines, modifiedLines };
}

function pathKey(projectDir: string, pathNorm: string): string {
  return `${projectDir}\0${pathNorm}`;
}

export function useEditorDiffDecorations(
  editorRef: React.MutableRefObject<monaco.editor.IStandaloneCodeEditor | null>,
  _monacoRef: React.MutableRefObject<typeof monaco | null>,
  projectDir: string | null | undefined,
  pathNorm: string | null,
  currentContent: string,
  editorReady: boolean
): void {
  const diffDecorationsRef = useRef<string[]>([]);
  const headContentCacheRef = useRef<Record<string, string>>({});
  const currentContentRef = useRef(currentContent);
  currentContentRef.current = currentContent;

  const applyDiffDecorations = useCallback(async () => {
    const editor = editorRef.current;
    if (!editor || !projectDir || !pathNorm) return;
    const model = editor.getModel();
    if (!model) return;
    const key = pathKey(projectDir, pathNorm);
    let headContent = headContentCacheRef.current[key];
    if (headContent === undefined) {
      try {
        headContent = (await invoke("git_show_file", {
          projectDir,
          revision: "HEAD",
          relativePath: pathNorm,
        })) as string;
      } catch {
        headContent = "";
      }
      headContentCacheRef.current[key] = headContent;
    }
    const content = currentContentRef.current;
    const patch = createTwoFilesPatch(
      `a/${pathNorm}`,
      `b/${pathNorm}`,
      headContent,
      content,
      "HEAD",
      "buffer"
    );
    const trimmed = (patch ?? "").trim();
    if (!trimmed || !trimmed.includes("@@")) {
      editor.deltaDecorations(diffDecorationsRef.current, []);
      diffDecorationsRef.current = [];
      return;
    }
    try {
      const { addedLines, modifiedLines } = parseUnifiedDiffToLineRanges(patch);
      const newDecorations: monaco.editor.IModelDeltaDecoration[] = [
        ...addedLines.map((lineNum) => ({
          range: { startLineNumber: lineNum, startColumn: 1, endLineNumber: lineNum, endColumn: 1 },
          options: {
            isWholeLine: true,
            className: "diff-line-added",
            glyphMarginClassName: "diff-gutter-added",
            linesDecorationsClassName: "diff-line-added",
          },
        })),
        ...modifiedLines.map((lineNum) => ({
          range: { startLineNumber: lineNum, startColumn: 1, endLineNumber: lineNum, endColumn: 1 },
          options: {
            isWholeLine: true,
            className: "diff-line-removed",
            glyphMarginClassName: "diff-gutter-removed",
            linesDecorationsClassName: "diff-line-removed",
          },
        })),
      ];
      const ids = editor.deltaDecorations(diffDecorationsRef.current, newDecorations);
      diffDecorationsRef.current = ids;
    } catch {
      editor.deltaDecorations(diffDecorationsRef.current, []);
      diffDecorationsRef.current = [];
    }
  }, [projectDir, pathNorm, editorRef]);

  useEffect(() => {
    if (!editorReady || pathNorm == null || projectDir == null) {
      const editor = editorRef.current;
      if (editor && diffDecorationsRef.current.length > 0) {
        editor.deltaDecorations(diffDecorationsRef.current, []);
        diffDecorationsRef.current = [];
      }
      return;
    }
    applyDiffDecorations();
    return () => {
      const editor = editorRef.current;
      if (editor && diffDecorationsRef.current.length > 0) {
        editor.deltaDecorations(diffDecorationsRef.current, []);
        diffDecorationsRef.current = [];
      }
    };
  }, [editorReady, pathNorm, projectDir, currentContent, applyDiffDecorations, editorRef]);
}
