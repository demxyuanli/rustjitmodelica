import { useMemo, useState } from "react";
import { t } from "../i18n";
import type { ComponentLibrary } from "../types";
import { AppIcon } from "./Icon";

interface LibrarySourceSidebarProps {
  libraries: ComponentLibrary[];
  selectedLibraryId: string;
  onSelectLibrary: (libraryId: string) => void;
  busy: boolean;
  projectDir?: string | null;
  query: string;
  onQueryChange: (value: string) => void;
  enabledOnly: boolean;
  onEnabledOnlyChange: (value: boolean) => void;
  onToggleLibrary: (library: ComponentLibrary) => void;
  onRemoveLibrary: (library: ComponentLibrary) => void;
}

type ScopeGroup = "system" | "global" | "project";

const SCOPES: ScopeGroup[] = ["system", "global", "project"];

function scopeLabel(scope: ScopeGroup) {
  switch (scope) {
    case "system":
      return t("componentLibrariesSystem");
    case "global":
      return t("componentLibrariesGlobal");
    case "project":
      return t("componentLibrariesProject");
  }
}

export function LibrarySourceSidebar({
  libraries,
  selectedLibraryId,
  onSelectLibrary,
  busy,
  projectDir,
  query,
  onQueryChange,
  enabledOnly,
  onEnabledOnlyChange,
  onToggleLibrary,
  onRemoveLibrary,
}: LibrarySourceSidebarProps) {
  const [collapsed, setCollapsed] = useState<Record<ScopeGroup, boolean>>({
    system: false,
    global: false,
    project: false,
  });

  const normalizedQuery = query.trim().toLowerCase();
  const filteredLibraries = useMemo(
    () =>
      libraries.filter((library) => {
        if (enabledOnly && !library.enabled) {
          return false;
        }
        if (!normalizedQuery) {
          return true;
        }
        return [
          library.displayName,
          library.sourcePath ?? "",
          library.kind,
          library.scope,
        ].some((value) => value.toLowerCase().includes(normalizedQuery));
      }),
    [enabledOnly, libraries, normalizedQuery]
  );

  const librariesByScope = useMemo(
    () => ({
      system: filteredLibraries.filter((item) => item.scope === "system"),
      global: filteredLibraries.filter((item) => item.scope === "global"),
      project: filteredLibraries.filter((item) => item.scope === "project"),
    }),
    [filteredLibraries]
  );

  const totalVisible = filteredLibraries.length;

  return (
    <aside className="w-[248px] shrink-0 border-r border-border bg-[var(--bg-elevated)]">
      <div className="panel-header-bar-tall flex flex-col items-stretch border-b border-border">
        <div className="text-sm font-medium">{t("componentLibraryManager")}</div>
        <div className="mt-2 flex flex-col gap-2">
          <input
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder={t("librarySourceSearchPlaceholder")}
            className="w-full rounded border border-border bg-[var(--surface)] px-2.5 py-1.5 text-xs outline-none focus:border-primary"
          />
          <label className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
            <input
              type="checkbox"
              checked={enabledOnly}
              onChange={(event) => onEnabledOnlyChange(event.target.checked)}
            />
            {t("libraryEnabledOnly")}
          </label>
        </div>
      </div>

      <div className="h-full overflow-auto px-2 py-2">
        <button
          type="button"
          className={`mb-2 flex w-full items-center justify-between rounded border px-2.5 py-2 text-left text-xs ${
            selectedLibraryId === "all"
              ? "border-primary bg-[var(--surface-active)] text-primary"
              : "border-border bg-[var(--surface)] text-[var(--text)] hover:bg-[var(--surface-hover)]"
          }`}
          onClick={() => onSelectLibrary("all")}
        >
          <span className="truncate font-medium">{t("libraryAllSources")}</span>
          <span className="shrink-0 text-[11px] text-[var(--text-muted)]">{totalVisible}</span>
        </button>

        <div className="space-y-3">
          {SCOPES.map((scope) => {
            const items = librariesByScope[scope];
            const isCollapsed = collapsed[scope];
            return (
              <section key={scope}>
                <button
                  type="button"
                  className="mb-1 flex w-full items-center justify-between px-1 text-[11px] font-medium uppercase tracking-wide text-[var(--text-muted)]"
                  onClick={() => setCollapsed((prev) => ({ ...prev, [scope]: !prev[scope] }))}
                >
                  <span>{scopeLabel(scope)}</span>
                  <span>{isCollapsed ? "+" : "-"}</span>
                </button>
                {scope === "project" && !projectDir && !isCollapsed && (
                  <div className="mb-2 px-1 text-[11px] text-[var(--text-muted)]">
                    {t("componentLibrariesProjectUnavailable")}
                  </div>
                )}
                {!isCollapsed && (
                  <div className="space-y-1">
                    {items.length > 0 ? (
                      items.map((library) => {
                        const active = selectedLibraryId === library.id;
                        return (
                          <div
                            key={library.id}
                            className={`rounded border px-2 py-1.5 ${
                              active
                                ? "border-primary bg-[var(--surface-active)]"
                                : "border-border bg-[var(--surface)]"
                            }`}
                          >
                            <button
                              type="button"
                              className="flex w-full items-center gap-2 text-left"
                              onClick={() => onSelectLibrary(active ? "all" : library.id)}
                            >
                              <AppIcon
                                name={library.scope === "system" ? "explorer" : "library"}
                                aria-hidden="true"
                                className="h-3.5 w-3.5 shrink-0"
                              />
                              <div className="min-w-0 flex-1">
                                <div className="truncate text-xs font-medium text-[var(--text)]">
                                  {library.displayName}
                                </div>
                                <div className="truncate text-[10px] text-[var(--text-muted)]">
                                  {library.componentCount} · {library.kind}
                                </div>
                              </div>
                              <span
                                className={`h-2 w-2 shrink-0 rounded-full ${
                                  library.enabled ? "bg-emerald-500" : "bg-zinc-500"
                                }`}
                                aria-hidden="true"
                              />
                            </button>
                            {active && (
                              <div className="mt-1.5 space-y-1">
                                {library.sourcePath && (
                                  <div className="truncate text-[10px] text-[var(--text-muted)]">
                                    {library.sourcePath}
                                  </div>
                                )}
                                {!library.builtIn && (
                                  <div className="flex gap-1">
                                    <button
                                      type="button"
                                      onClick={() => onToggleLibrary(library)}
                                      disabled={busy}
                                      className="rounded border border-border px-1.5 py-0.5 text-[10px] hover:bg-[var(--surface-hover)] disabled:opacity-50"
                                    >
                                      {library.enabled ? t("disable") : t("enable")}
                                    </button>
                                    <button
                                      type="button"
                                      onClick={() => onRemoveLibrary(library)}
                                      disabled={busy}
                                      className="rounded border border-border px-1.5 py-0.5 text-[10px] hover:bg-[var(--surface-hover)] disabled:opacity-50"
                                    >
                                      {t("deleteTest")}
                                    </button>
                                  </div>
                                )}
                              </div>
                            )}
                          </div>
                        );
                      })
                    ) : (
                      <div className="px-1 text-[11px] text-[var(--text-muted)]">{t("componentLibrariesEmpty")}</div>
                    )}
                  </div>
                )}
              </section>
            );
          })}
        </div>
      </div>
    </aside>
  );
}
