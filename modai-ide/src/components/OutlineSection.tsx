import { useState, useMemo, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type monaco from "monaco-editor";
import { t } from "../i18n";
import { ContextMenu } from "./ContextMenu";

export interface OutlineSymbol {
  kind: string;
  name: string;
  line: number;
  lineEnd?: number;
  signature?: string;
  parentSymbolId?: number | null;
  children?: OutlineSymbol[];
}

interface IndexSymbol {
  id: number;
  fileId: number;
  name: string;
  kind: string;
  lineStart: number;
  lineEnd: number;
  parentSymbolId: number | null;
  signature: string | null;
  docComment: string | null;
  filePath: string;
}

const SYMBOL_ICONS: Record<string, string> = {
  model: "M",
  function: "F",
  block: "B",
  connector: "C",
  record: "R",
  package: "P",
  parameter: "P",
  variable: "V",
  type_alias: "T",
  class: "C",
  file: "F",
};

const SYMBOL_COLORS: Record<string, string> = {
  model: "#3b82f6",
  function: "#a855f7",
  block: "#f97316",
  connector: "#eab308",
  record: "#0ea5e9",
  package: "#10b981",
  parameter: "#6366f1",
  variable: "#22c55e",
  type_alias: "#6b7280",
  class: "#6b7280",
  file: "#9ca3af",
};

const MODELICA_SYMBOL_RE =
  /^\s*(model|function|block|connector|record|package|class)\s+(\w+)/im;

function parseOutlineRegex(code: string): OutlineSymbol[] {
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

function buildTree(flat: IndexSymbol[]): OutlineSymbol[] {
  const byId = new Map<number, OutlineSymbol>();
  const roots: OutlineSymbol[] = [];

  for (const s of flat) {
    const node: OutlineSymbol = {
      kind: s.kind,
      name: s.name,
      line: s.lineStart,
      lineEnd: s.lineEnd,
      signature: s.signature ?? undefined,
      parentSymbolId: s.parentSymbolId,
      children: [],
    };
    byId.set(s.id, node);
  }

  for (const s of flat) {
    const node = byId.get(s.id)!;
    if (s.parentSymbolId != null && byId.has(s.parentSymbolId)) {
      byId.get(s.parentSymbolId)!.children!.push(node);
    } else {
      roots.push(node);
    }
  }

  return roots;
}

interface OutlineSectionProps {
  code: string;
  openFilePath: string | null;
  editorRef: React.MutableRefObject<monaco.editor.IStandaloneCodeEditor | null>;
  projectDir?: string | null;
  onOpenDiagram?: () => void;
}

export function OutlineSection({
  code,
  openFilePath,
  editorRef,
  projectDir,
  onOpenDiagram,
}: OutlineSectionProps) {
  const [expanded, setExpanded] = useState(true);
  const [indexSymbols, setIndexSymbols] = useState<IndexSymbol[] | null>(null);
  const [menuVisible, setMenuVisible] = useState(false);
  const [menuPosition, setMenuPosition] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [menuSymbol, setMenuSymbol] = useState<OutlineSymbol | null>(null);

  const showOutline =
    openFilePath != null &&
    (openFilePath.endsWith(".mo") || openFilePath.endsWith(".MO"));
  const displayName =
    openFilePath != null ? openFilePath.replace(/^.*[/\\]/, "") : "";

  useEffect(() => {
    if (!projectDir || !openFilePath || !showOutline) {
      setIndexSymbols(null);
      return;
    }
    let cancelled = false;
    invoke("index_file_symbols", {
      projectDir,
      filePath: openFilePath,
    })
      .then((result) => {
        if (!cancelled) setIndexSymbols(result as IndexSymbol[]);
      })
      .catch(() => {
        if (!cancelled) setIndexSymbols(null);
      });
    return () => {
      cancelled = true;
    };
  }, [projectDir, openFilePath, code, showOutline]);

  const regexSymbols = useMemo(() => parseOutlineRegex(code), [code]);

  const treeSymbols = useMemo(() => {
    if (indexSymbols && indexSymbols.length > 0) {
      return buildTree(indexSymbols);
    }
    return null;
  }, [indexSymbols]);

  const symbols: OutlineSymbol[] = treeSymbols ?? regexSymbols;

  const handleSymbolClick = (line: number) => {
    const editor = editorRef.current;
    if (!editor) return;
    editor.revealLineInCenter(line);
    editor.setPosition({ lineNumber: line, column: 1 });
    editor.focus();
  };

  const handleContextMenu = useCallback((event: React.MouseEvent, symbol: OutlineSymbol) => {
    event.preventDefault();
    setMenuPosition({ x: event.clientX, y: event.clientY });
    setMenuSymbol(symbol);
    setMenuVisible(true);
  }, []);

  return (
    <div className="shrink-0 border-t border-border bg-[var(--surface-alt)]">
      <button
        type="button"
        className="tree-row w-full text-left font-medium text-[var(--text-muted)] hover:bg-white/5 rounded-none"
        style={{ paddingLeft: 8 }}
        onClick={() => setExpanded((e) => !e)}
        aria-expanded={expanded}
      >
        <span className="tree-arrow">
          {expanded ? "\u02C5" : "\u203A"}
        </span>
        <span className="tree-label">{t("outline")}</span>
        {indexSymbols && indexSymbols.length > 0 && (
          <span className="ml-1 text-[10px] text-[var(--text-muted)] opacity-60">
            (indexed)
          </span>
        )}
      </button>
      {expanded && (
        <div className="pb-2 px-2">
          {!showOutline ? (
            <div className="text-xs text-[var(--text-muted)] px-1">
              {t("noSymbolsInDocument").replace(
                "{name}",
                displayName || "?"
              )}
            </div>
          ) : symbols.length === 0 ? (
            <div className="text-xs text-[var(--text-muted)] px-1">
              {t("noSymbolsInDocument").replace("{name}", displayName)}
            </div>
          ) : (
            <>
              {onOpenDiagram && (
                <button
                  type="button"
                  className="tree-row w-full text-left text-xs px-1 py-0.5 rounded hover:bg-white/10 text-[var(--text)] flex items-center gap-1"
                  onClick={onOpenDiagram}
                  title={t("viewDiagramReadOnly")}
                >
                  <span
                    className="inline-flex items-center justify-center w-4 h-4 rounded-full text-[10px] font-mono text-white flex-shrink-0"
                    style={{ backgroundColor: SYMBOL_COLORS.model }}
                  >
                    D
                  </span>
                  <span className="text-[var(--text-muted)]">{t("outlineDiagram")}</span>
                </button>
              )}
              <ul className="text-xs space-y-0.5">
                {symbols.map((sym, i) => (
                  <SymbolNode
                    key={`${sym.name}-${sym.line}-${i}`}
                    symbol={sym}
                    depth={0}
                    onClick={handleSymbolClick}
                    onContextMenu={handleContextMenu}
                  />
                ))}
              </ul>
            </>
          )}
        </div>
      )}
      <ContextMenu
        visible={menuVisible}
        x={menuPosition.x}
        y={menuPosition.y}
        onClose={() => setMenuVisible(false)}
        items={[
          {
            id: "go-to-definition",
            label: t("goToDefinition") ?? "Go to definition",
            onClick: () => {
              if (menuSymbol) {
                handleSymbolClick(menuSymbol.line);
              }
            },
          },
          {
            id: "copy-name",
            label: t("contextCopyName"),
            disabled: !menuSymbol,
            onClick: () => {
              if (!menuSymbol) return;
              void navigator.clipboard.writeText(menuSymbol.name);
            },
          },
          {
            id: "copy-signature",
            label: t("contextCopySignature"),
            disabled: !menuSymbol?.signature,
            onClick: () => {
              if (!menuSymbol?.signature) return;
              void navigator.clipboard.writeText(menuSymbol.signature);
            },
          },
        ]}
      />
    </div>
  );
}

function SymbolNode({
  symbol,
  depth,
  onClick,
  onContextMenu,
}: {
  symbol: OutlineSymbol;
  depth: number;
  onClick: (line: number) => void;
  onContextMenu: (event: React.MouseEvent, symbol: OutlineSymbol) => void;
}) {
  const [open, setOpen] = useState(depth < 1);
  const hasChildren = symbol.children && symbol.children.length > 0;
  const icon = SYMBOL_ICONS[symbol.kind] ?? "?";

  return (
    <li>
      <div className="flex items-center">
        {hasChildren && (
          <button
            type="button"
            className="w-3 h-3 flex items-center justify-center text-[var(--text-muted)] mr-0.5 shrink-0"
            onClick={() => setOpen((o) => !o)}
          >
            {open ? "\u02C5" : "\u203A"}
          </button>
        )}
        {!hasChildren && <span className="w-3 mr-0.5 shrink-0" />}
        <button
          type="button"
          className="flex-1 text-left truncate px-1 py-0.5 rounded hover:bg-white/10 text-[var(--text)] flex items-center gap-1"
          style={{ paddingLeft: depth * 8 }}
          onClick={() => onClick(symbol.line)}
          onContextMenu={(event) => onContextMenu(event, symbol)}
          title={symbol.signature ?? `${symbol.kind} ${symbol.name} (line ${symbol.line})`}
        >
          <span
            className="inline-flex items-center justify-center w-4 h-4 rounded-full text-[10px] font-mono text-white flex-shrink-0"
            style={{ backgroundColor: SYMBOL_COLORS[symbol.kind] ?? "var(--surface-hover)" }}
          >
            {icon}
          </span>
          <span className="truncate">{symbol.name}</span>
        </button>
      </div>
      {hasChildren && open && (
        <ul className="space-y-0.5">
          {symbol.children!.map((child, ci) => (
            <SymbolNode
              key={`${child.name}-${child.line}-${ci}`}
              symbol={child}
              depth={depth + 1}
              onClick={onClick}
              onContextMenu={onContextMenu}
            />
          ))}
        </ul>
      )}
    </li>
  );
}
