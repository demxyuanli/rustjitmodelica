import { useState, useCallback, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import { useSearch } from "../hooks/useSearch";

interface SymbolResult {
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

type SearchMode = "text" | "symbol";

const SYMBOL_KINDS = [
  "model",
  "function",
  "block",
  "connector",
  "record",
  "package",
  "parameter",
  "variable",
];

interface SearchPanelProps {
  projectDir: string | null;
  onOpenFile: (relativePath: string) => void;
}

export function SearchPanel({ projectDir, onOpenFile }: SearchPanelProps) {
  const search = useSearch(projectDir);
  const [collapsedFiles, setCollapsedFiles] = useState<Set<string>>(new Set());
  const inputRef = useRef<HTMLInputElement>(null);
  const [mode, setMode] = useState<SearchMode>("text");
  const [symbolQuery, setSymbolQuery] = useState("");
  const [symbolKindFilter, setSymbolKindFilter] = useState<string>("");
  const [symbolResults, setSymbolResults] = useState<SymbolResult[]>([]);
  const [symbolLoading, setSymbolLoading] = useState(false);
  const [symbolSearched, setSymbolSearched] = useState(false);
  const symbolDebounce = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    setCollapsedFiles(new Set());
  }, [search.results]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        if (mode === "text") search.searchNow();
        else doSymbolSearch(symbolQuery);
      }
    },
    [search.searchNow, mode, symbolQuery]
  );

  const doSymbolSearch = useCallback(
    async (q: string) => {
      if (!projectDir || !q.trim()) {
        setSymbolResults([]);
        setSymbolSearched(false);
        return;
      }
      setSymbolLoading(true);
      setSymbolSearched(true);
      try {
        const results = (await invoke("index_search_symbols", {
          projectDir,
          query: q.trim(),
          kind: symbolKindFilter || null,
          limit: 200,
        })) as SymbolResult[];
        setSymbolResults(results);
      } catch {
        setSymbolResults([]);
      } finally {
        setSymbolLoading(false);
      }
    },
    [projectDir, symbolKindFilter]
  );

  const updateSymbolQuery = useCallback(
    (val: string) => {
      setSymbolQuery(val);
      if (symbolDebounce.current) clearTimeout(symbolDebounce.current);
      if (val.trim().length >= 2) {
        symbolDebounce.current = setTimeout(() => doSymbolSearch(val), 400);
      } else {
        setSymbolResults([]);
        setSymbolSearched(false);
      }
    },
    [doSymbolSearch]
  );

  const toggleFile = useCallback((file: string) => {
    setCollapsedFiles((prev) => {
      const next = new Set(prev);
      if (next.has(file)) next.delete(file);
      else next.add(file);
      return next;
    });
  }, []);

  if (!projectDir) {
    return (
      <div className="p-3 text-sm text-[var(--text-muted)]">
        {t("openProjectFirst")}
      </div>
    );
  }

  const symbolsByFile = new Map<string, SymbolResult[]>();
  for (const s of symbolResults) {
    const arr = symbolsByFile.get(s.filePath) ?? [];
    arr.push(s);
    symbolsByFile.set(s.filePath, arr);
  }

  return (
    <div className="flex flex-col h-full min-h-0">
      <div className="shrink-0 p-2 flex flex-col gap-1.5 border-b border-border">
        <div className="flex items-center gap-1 mb-1">
          <button
            type="button"
            className={`px-2 py-0.5 text-xs rounded ${mode === "text" ? "bg-[var(--accent)] text-white" : "bg-[var(--surface)] text-[var(--text-muted)]"}`}
            onClick={() => setMode("text")}
          >
            {t("textSearch")}
          </button>
          <button
            type="button"
            className={`px-2 py-0.5 text-xs rounded ${mode === "symbol" ? "bg-[var(--accent)] text-white" : "bg-[var(--surface)] text-[var(--text-muted)]"}`}
            onClick={() => setMode("symbol")}
          >
            {t("symbolSearch")}
          </button>
        </div>

        {mode === "text" ? (
          <>
            <input
              ref={inputRef}
              type="text"
              className="w-full text-sm rounded bg-[var(--surface)] border border-border px-2 py-1.5 text-[var(--text)] placeholder-[var(--text-muted)]"
              placeholder={t("searchPlaceholder")}
              value={search.query}
              onChange={(e) => search.updateQuery(e.target.value)}
              onKeyDown={handleKeyDown}
            />
            <div className="flex items-center gap-2 text-xs">
              <label className="flex items-center gap-1 cursor-pointer text-[var(--text-muted)]">
                <input
                  type="checkbox"
                  checked={search.options.caseSensitive}
                  onChange={(e) =>
                    search.setOption("caseSensitive", e.target.checked)
                  }
                  className="shrink-0"
                />
                {t("caseSensitive")}
              </label>
              <input
                type="text"
                className="flex-1 min-w-0 text-xs rounded bg-[var(--surface)] border border-border px-1.5 py-0.5 text-[var(--text)] placeholder-[var(--text-muted)]"
                placeholder={t("fileFilter")}
                value={search.options.filePattern}
                onChange={(e) =>
                  search.setOption("filePattern", e.target.value)
                }
              />
            </div>
          </>
        ) : (
          <>
            <input
              ref={inputRef}
              type="text"
              className="w-full text-sm rounded bg-[var(--surface)] border border-border px-2 py-1.5 text-[var(--text)] placeholder-[var(--text-muted)]"
              placeholder={t("symbolSearchPlaceholder")}
              value={symbolQuery}
              onChange={(e) => updateSymbolQuery(e.target.value)}
              onKeyDown={handleKeyDown}
            />
            <select
              className="text-xs rounded bg-[var(--surface)] border border-border px-1.5 py-1 text-[var(--text)]"
              value={symbolKindFilter}
              onChange={(e) => {
                setSymbolKindFilter(e.target.value);
                if (symbolQuery.trim().length >= 2) doSymbolSearch(symbolQuery);
              }}
            >
              <option value="">{t("allKinds")}</option>
              {SYMBOL_KINDS.map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </>
        )}
      </div>

      {mode === "text" && (
        <>
          {search.loading && (
            <div className="p-2 text-xs text-[var(--text-muted)]">
              {t("running")}
            </div>
          )}

          {search.searched && !search.loading && search.matchCount === 0 && (
            <div className="p-3 text-xs text-[var(--text-muted)]">
              {t("noSearchResults")}
            </div>
          )}

          {search.matchCount > 0 && (
            <div className="shrink-0 px-2 py-1 text-xs text-[var(--text-muted)] border-b border-border">
              {t("matchCount")
                .replace("{count}", String(search.matchCount))
                .replace("{files}", String(search.fileCount))}
              {search.matchCount >= 500 && (
                <span className="ml-1 text-amber-400">
                  ({t("searchLimitReached")})
                </span>
              )}
            </div>
          )}

          <div className="flex-1 min-h-0 overflow-auto scroll-vscode">
            {search.grouped.map((group) => {
              const isCollapsed = collapsedFiles.has(group.file);
              return (
                <div key={group.file} className="border-b border-border/50">
                  <button
                    type="button"
                    className="flex items-center gap-1.5 w-full px-2 py-1 text-left hover:bg-white/5"
                    onClick={() => toggleFile(group.file)}
                  >
                    <span className="text-[var(--text-muted)] text-[10px] shrink-0">
                      {isCollapsed ? "\u25B6" : "\u25BC"}
                    </span>
                    <span className="text-xs font-medium text-[var(--text)] truncate flex-1">
                      {group.file}
                    </span>
                    <span className="text-[10px] text-[var(--text-muted)] shrink-0">
                      {group.matches.length}
                    </span>
                  </button>
                  {!isCollapsed && (
                    <div className="pl-4">
                      {group.matches.map((m, i) => (
                        <button
                          key={`${m.line}:${m.column}:${i}`}
                          type="button"
                          className="flex items-start gap-2 w-full px-2 py-0.5 text-left hover:bg-white/10 text-xs"
                          onClick={() => onOpenFile(m.file)}
                        >
                          <span className="text-[var(--text-muted)] shrink-0 w-8 text-right font-mono">
                            {m.line}
                          </span>
                          <span className="text-[var(--text)] truncate font-mono">
                            {m.line_content.trim()}
                          </span>
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </>
      )}

      {mode === "symbol" && (
        <>
          {symbolLoading && (
            <div className="p-2 text-xs text-[var(--text-muted)]">
              {t("running")}
            </div>
          )}

          {symbolSearched && !symbolLoading && symbolResults.length === 0 && (
            <div className="p-3 text-xs text-[var(--text-muted)]">
              {t("noSearchResults")}
            </div>
          )}

          {symbolResults.length > 0 && (
            <div className="shrink-0 px-2 py-1 text-xs text-[var(--text-muted)] border-b border-border">
              {symbolResults.length} symbol(s) in{" "}
              {symbolsByFile.size} file(s)
            </div>
          )}

          <div className="flex-1 min-h-0 overflow-auto scroll-vscode">
            {Array.from(symbolsByFile.entries()).map(([filePath, syms]) => {
              const isCollapsed = collapsedFiles.has(filePath);
              return (
                <div key={filePath} className="border-b border-border/50">
                  <button
                    type="button"
                    className="flex items-center gap-1.5 w-full px-2 py-1 text-left hover:bg-white/5"
                    onClick={() => toggleFile(filePath)}
                  >
                    <span className="text-[var(--text-muted)] text-[10px] shrink-0">
                      {isCollapsed ? "\u25B6" : "\u25BC"}
                    </span>
                    <span className="text-xs font-medium text-[var(--text)] truncate flex-1">
                      {filePath}
                    </span>
                    <span className="text-[10px] text-[var(--text-muted)] shrink-0">
                      {syms.length}
                    </span>
                  </button>
                  {!isCollapsed && (
                    <div className="pl-4">
                      {syms.map((s) => (
                        <button
                          key={`${s.id}`}
                          type="button"
                          className="flex items-start gap-2 w-full px-2 py-0.5 text-left hover:bg-white/10 text-xs"
                          onClick={() => onOpenFile(s.filePath)}
                          title={s.signature ?? undefined}
                        >
                          <span className="text-[var(--text-muted)] shrink-0 w-8 text-right font-mono">
                            {s.lineStart}
                          </span>
                          <span className="text-[var(--accent)] shrink-0 w-16 truncate">
                            {s.kind}
                          </span>
                          <span className="text-[var(--text)] truncate">
                            {s.name}
                          </span>
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
}
