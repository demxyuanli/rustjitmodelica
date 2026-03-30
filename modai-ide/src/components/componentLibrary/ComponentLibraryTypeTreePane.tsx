import { ChevronDown, ChevronRight } from "lucide-react";
import { t } from "../../i18n";
import type { InstantiableClass } from "../../types";
import { sortedChildEntries, type TypeTreeNode } from "./typeTree";
import { componentKey } from "./componentLibraryKeys";

const SCOPE_ICON: Record<string, { letter: string; bg: string }> = {
  system: { letter: "S", bg: "bg-blue-500/85" },
  global: { letter: "G", bg: "bg-emerald-500/85" },
  project: { letter: "P", bg: "bg-amber-500/85" },
};

function scopeIconTitle(scope: string): string {
  switch (scope) {
    case "system":
      return t("componentLibrariesSystem");
    case "global":
      return t("componentLibrariesGlobal");
    case "project":
      return t("componentLibrariesProject");
    default:
      return scope;
  }
}

function ScopeIcon({ scope }: { scope: string }) {
  const config = SCOPE_ICON[scope] ?? {
    letter: scope.charAt(0).toUpperCase(),
    bg: "bg-[var(--surface-hover)]",
  };
  return (
    <span
      className={`inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[10px] font-semibold text-white ${config.bg}`}
      title={scopeIconTitle(scope)}
      aria-label={scopeIconTitle(scope)}
    >
      {config.letter}
    </span>
  );
}

export function SectionIcon({ letter, bg, title }: { letter: string; bg: string; title: string }) {
  return (
    <span
      className={`inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[10px] font-semibold text-white ${bg}`}
      title={title}
      aria-hidden="true"
    >
      {letter}
    </span>
  );
}

const TREE_INDENT_PX = 16;

function TypeTreeLevel({
  node,
  depth,
  expandedPaths,
  toggleExpanded,
  selectedKey,
  onSelect,
  componentKey: keyOf,
}: {
  node: TypeTreeNode;
  depth: number;
  expandedPaths: Set<string>;
  toggleExpanded: (fullPath: string) => void;
  selectedKey: string | null;
  onSelect: (key: string) => void;
  componentKey: (item: InstantiableClass) => string;
}) {
  const hasChildren = sortedChildEntries(node).length > 0;
  const isExpanded = expandedPaths.has(node.fullPath);
  const indent = depth * TREE_INDENT_PX;

  return (
    <div className="flex flex-col">
      {node.items.length > 0 &&
        node.items.map((item) => {
          const active = keyOf(item) === selectedKey;
          return (
            <button
              key={keyOf(item)}
              type="button"
              className={`w-full px-4 py-3 text-left transition-colors ${
                active ? "bg-[var(--surface-active)]" : "hover:bg-[var(--surface-hover)]"
              }`}
              style={{ paddingLeft: indent + 16 }}
              onClick={() => onSelect(keyOf(item))}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium text-[var(--text)]">{item.qualifiedName}</div>
                  <div className="mt-1 truncate text-[11px] text-[var(--text-muted)]">
                    {item.libraryName} · {item.kind}
                  </div>
                  {item.summary && (
                    <div className="mt-2 line-clamp-2 text-xs text-[var(--text-muted)]">{item.summary}</div>
                  )}
                </div>
                <ScopeIcon scope={item.libraryScope} />
              </div>
            </button>
          );
        })}
      {hasChildren && (
        <>
          <button
            type="button"
            className="flex w-full items-center gap-1 px-4 py-2 text-left text-sm text-[var(--text)] hover:bg-[var(--surface-hover)]"
            style={{ paddingLeft: indent + 16 }}
            onClick={() => toggleExpanded(node.fullPath)}
          >
            {isExpanded ? (
              <ChevronDown className="h-4 w-4 shrink-0" />
            ) : (
              <ChevronRight className="h-4 w-4 shrink-0" />
            )}
            <span className="truncate font-medium">{node.segment}</span>
          </button>
          {isExpanded &&
            sortedChildEntries(node).map(([, child]) => (
              <TypeTreeLevel
                key={child.fullPath}
                node={child}
                depth={depth + 1}
                expandedPaths={expandedPaths}
                toggleExpanded={toggleExpanded}
                selectedKey={selectedKey}
                onSelect={onSelect}
                componentKey={keyOf}
              />
            ))}
        </>
      )}
    </div>
  );
}

export type LibraryScopeFilter = "all" | "system" | "global" | "project";

export interface ComponentLibraryTypeTreePaneProps {
  typeTree: TypeTreeNode;
  expandedPaths: Set<string>;
  toggleExpanded: (fullPath: string) => void;
  selectedKey: string | null;
  onSelectKey: (key: string) => void;
  typeQuery: string;
  onTypeQueryChange: (value: string) => void;
  scopeFilter: LibraryScopeFilter;
  onScopeFilterChange: (value: LibraryScopeFilter) => void;
  scopeOptions: Array<{ value: LibraryScopeFilter; label: string }>;
  enabledOnly: boolean;
  onEnabledOnlyChange: (value: boolean) => void;
  typeItemsLength: number;
  typeTotal: number;
  typesBusy: boolean;
  typeHasMore: boolean;
  onLoadMore: () => void;
  selectedLibraryDisplayName: string | null;
}

export function ComponentLibraryTypeTreePane({
  typeTree,
  expandedPaths,
  toggleExpanded,
  selectedKey,
  onSelectKey,
  typeQuery,
  onTypeQueryChange,
  scopeFilter,
  onScopeFilterChange,
  scopeOptions,
  enabledOnly,
  onEnabledOnlyChange,
  typeItemsLength,
  typeTotal,
  typesBusy,
  typeHasMore,
  onLoadMore,
  selectedLibraryDisplayName,
}: ComponentLibraryTypeTreePaneProps) {
  return (
    <section className="flex min-h-0 w-[340px] shrink-0 flex-col border-r border-border bg-surface-alt overflow-hidden">
      <div className="panel-header-bar-tall shrink-0 border-b border-border flex flex-col gap-2">
        <div className="flex items-center gap-2 text-sm font-medium">
          <SectionIcon letter="T" bg="bg-violet-500/85" title={t("libraryTypeList")} />
          {t("libraryTypeList")}
        </div>
        <div className="mt-2 flex flex-col gap-2">
          <input
            value={typeQuery}
            onChange={(event) => onTypeQueryChange(event.target.value)}
            placeholder={t("librarySearchPlaceholder")}
            className="w-full rounded border border-border bg-[var(--surface)] px-3 py-2 text-sm outline-none focus:border-primary"
          />
          <div className="flex items-center gap-2">
            <select
              value={scopeFilter}
              onChange={(event) => onScopeFilterChange(event.target.value as LibraryScopeFilter)}
              className="flex-1 rounded border border-border bg-[var(--surface)] px-2 py-1.5 text-xs"
            >
              {scopeOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
            <label className="flex items-center gap-1 text-xs text-[var(--text-muted)]">
              <input
                type="checkbox"
                checked={enabledOnly}
                onChange={(event) => onEnabledOnlyChange(event.target.checked)}
              />
              {t("libraryEnabledOnly")}
            </label>
          </div>
        </div>
        <div className="mt-2 flex items-center justify-between text-xs text-[var(--text-muted)]">
          <span>
            {typeItemsLength} / {typeTotal} {t("librarySearchResults")}
          </span>
          {selectedLibraryDisplayName && (
            <span className="truncate max-w-[120px]">{selectedLibraryDisplayName}</span>
          )}
        </div>
      </div>
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="min-h-0 flex-1 overflow-y-auto">
          {typeItemsLength === 0 && !typesBusy ? (
            <div className="px-4 py-6 text-sm text-[var(--text-muted)]">{t("libraryNoResults")}</div>
          ) : (
            <div className="py-1">
              {sortedChildEntries(typeTree).map(([, child]) => (
                <TypeTreeLevel
                  key={child.fullPath}
                  node={child}
                  depth={0}
                  expandedPaths={expandedPaths}
                  toggleExpanded={toggleExpanded}
                  selectedKey={selectedKey}
                  onSelect={onSelectKey}
                  componentKey={componentKey}
                />
              ))}
              {typesBusy && (
                <div className="px-4 py-3 text-xs text-[var(--text-muted)]">{t("loading")}</div>
              )}
            </div>
          )}
        </div>
        {typeHasMore && (
          <div className="shrink-0 border-t border-border p-3">
            <button
              type="button"
              onClick={onLoadMore}
              disabled={typesBusy}
              className="w-full rounded border border-border px-3 py-2 text-xs hover:bg-[var(--surface-hover)] disabled:opacity-50"
            >
              {t("libraryLoadMore")}
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
