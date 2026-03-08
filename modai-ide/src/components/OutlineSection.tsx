import { useState, useMemo } from "react";
import type monaco from "monaco-editor";
import { t } from "../i18n";

export interface OutlineSymbol {
  kind: string;
  name: string;
  line: number;
}

const MODELICA_SYMBOL_RE = /^\s*(model|function|block|connector|record|package|class)\s+(\w+)/im;

function parseOutline(code: string): OutlineSymbol[] {
  const symbols: OutlineSymbol[] = [];
  const lines = code.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const m = line.match(MODELICA_SYMBOL_RE);
    if (m) {
      symbols.push({
        kind: m[1].toLowerCase(),
        name: m[2],
        line: i + 1,
      });
    }
  }
  return symbols;
}

interface OutlineSectionProps {
  code: string;
  openFilePath: string | null;
  editorRef: React.MutableRefObject<monaco.editor.IStandaloneCodeEditor | null>;
}

export function OutlineSection({ code, openFilePath, editorRef }: OutlineSectionProps) {
  const [expanded, setExpanded] = useState(true);

  const symbols = useMemo(() => parseOutline(code), [code]);
  const showOutline = openFilePath != null && (openFilePath.endsWith(".mo") || openFilePath.endsWith(".MO"));
  const displayName = openFilePath != null ? openFilePath.replace(/^.*[/\\]/, "") : "";

  const handleSymbolClick = (line: number) => {
    const editor = editorRef.current;
    if (!editor) return;
    editor.revealLineInCenter(line);
    editor.setPosition({ lineNumber: line, column: 1 });
    editor.focus();
  };

  return (
    <div className="shrink-0 border-t border-border">
      <button
        type="button"
        className="tree-row w-full text-left font-medium text-[var(--text-muted)] hover:bg-white/5 rounded-none"
        style={{ paddingLeft: 8 }}
        onClick={() => setExpanded((e) => !e)}
        aria-expanded={expanded}
      >
        <span className="tree-arrow">{expanded ? "\u02C5" : "\u203A"}</span>
        <span className="tree-label">{t("outline")}</span>
      </button>
      {expanded && (
        <div className="pb-2 px-2">
          {!showOutline ? (
            <div className="text-xs text-[var(--text-muted)] px-1">
              {t("noSymbolsInDocument").replace("{name}", displayName || "?")}
            </div>
          ) : symbols.length === 0 ? (
            <div className="text-xs text-[var(--text-muted)] px-1">
              {t("noSymbolsInDocument").replace("{name}", displayName)}
            </div>
          ) : (
            <ul className="text-xs space-y-0.5">
              {symbols.map((sym, i) => (
                <li key={`${sym.name}-${i}`}>
                  <button
                    type="button"
                    className="w-full text-left truncate px-1 py-0.5 rounded hover:bg-white/10 text-[var(--text)]"
                    onClick={() => handleSymbolClick(sym.line)}
                    title={`${sym.kind} ${sym.name} (line ${sym.line})`}
                  >
                    <span className="text-[var(--text-muted)]">{sym.kind}</span> {sym.name}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}
