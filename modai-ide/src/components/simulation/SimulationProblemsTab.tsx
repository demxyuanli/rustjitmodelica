import { t } from "../../i18n";
import type { JitValidateResult, LibrarySuggestion } from "../../types";
import { AppIcon } from "../Icon";
import { SimulationSectionHeader } from "./SimulationPanelChrome";
import type { TestAllResultItem } from "./types";

export interface SimulationProblemsTabProps {
  jitResult: JitValidateResult | null;
  testAllResults: TestAllResultItem[] | null;
  testSummary: { text: string; passed: number; failed: number } | null;
  onSuggestFixWithAi: (msg: string) => void;
  compilationExpanded: boolean;
  onCompilationExpandedToggle: () => void;
  variablesExpanded: boolean;
  onVariablesExpandedToggle: () => void;
  testResultsExpanded: boolean;
  onTestResultsExpandedToggle: () => void;
  suggestionByIndex: Record<number, LibrarySuggestion>;
  installBusy: boolean;
  installMessage: string | null;
  onInstallSuggestedLibrary: (suggestion: LibrarySuggestion) => void;
  selectedSymbol: string | null;
  onFocusSymbol?: (symbol: string) => void;
  onErrorContextMenu: (x: number, y: number, text: string) => void;
  jitErrorCount: number;
  jitWarnCount: number;
  totalVarCount: number;
}

function compilationStatusIcon(jitResult: JitValidateResult | null) {
  if (!jitResult) return null;
  if (jitResult.success)
    return (
      <AppIcon
        name="validate"
        className="!h-3.5 !w-3.5 text-[var(--success-text)]"
      />
    );
  return (
    <AppIcon name="error" className="!h-3.5 !w-3.5 text-[var(--danger-text)]" />
  );
}

export function SimulationProblemsTab({
  jitResult,
  testAllResults,
  testSummary,
  onSuggestFixWithAi,
  compilationExpanded,
  onCompilationExpandedToggle,
  variablesExpanded,
  onVariablesExpandedToggle,
  testResultsExpanded,
  onTestResultsExpandedToggle,
  suggestionByIndex,
  installBusy,
  installMessage,
  onInstallSuggestedLibrary,
  selectedSymbol,
  onFocusSymbol,
  onErrorContextMenu,
  jitErrorCount,
  jitWarnCount,
  totalVarCount,
}: SimulationProblemsTabProps) {
  return (
    <div className="flex flex-1 min-h-0 flex-col overflow-hidden">
      <div className="flex-1 overflow-auto scroll-vscode">
        <SimulationSectionHeader
          title={t("sectionCompilation")}
          expanded={compilationExpanded}
          onToggle={onCompilationExpandedToggle}
          statusIcon={compilationStatusIcon(jitResult)}
          badge={
            jitErrorCount > 0 ? (
              <span className="ml-1 rounded bg-[var(--danger-text)]/15 px-1.5 text-[10px] text-[var(--danger-text)]">
                {jitErrorCount}
              </span>
            ) : jitWarnCount > 0 ? (
              <span className="ml-1 rounded bg-[var(--warning-text)]/15 px-1.5 text-[10px] text-[var(--warning-text)]">
                {jitWarnCount}
              </span>
            ) : undefined
          }
        />
        {compilationExpanded && (
          <div className="px-3 py-2 text-xs font-mono">
            {!jitResult && (
              <div className="italic text-[var(--text-muted)]">
                {t("jitStatusNotRun")}
              </div>
            )}
            {jitResult?.success && (
              <div className="flex items-center gap-2">
                <AppIcon
                  name="validate"
                  className="!h-3.5 !w-3.5 shrink-0 text-[var(--success-text)]"
                />
                <span className="text-[var(--success-text)]">
                  {t("jitStatusOk")}
                </span>
                {totalVarCount > 0 && (
                  <span className="text-[var(--text-muted)]">
                    &mdash; {totalVarCount}{" "}
                    {t("variablesSelect").toLowerCase()}
                  </span>
                )}
              </div>
            )}
            {jitResult && !jitResult.success && (
              <div className="space-y-1">
                {installMessage && (
                  <div className="rounded border border-border bg-[var(--bg-elevated)] px-2 py-1.5 text-xs text-[var(--text-muted)]">
                    {installMessage}
                  </div>
                )}
                {jitResult.errors.map((e, i) => (
                  <div
                    key={i}
                    className="text-[var(--danger-text)]"
                    onContextMenu={(event) => {
                      event.preventDefault();
                      onErrorContextMenu(event.clientX, event.clientY, e);
                    }}
                  >
                    <div>{e}</div>
                    {suggestionByIndex[i] && (
                      <div className="mt-1 flex flex-wrap items-center gap-1">
                        <span className="text-[11px] text-[var(--text-muted)]">
                          {t("suggestLibraryForType")}:{" "}
                          {suggestionByIndex[i].displayName}
                        </span>
                        <button
                          type="button"
                          onClick={() =>
                            void onInstallSuggestedLibrary(suggestionByIndex[i])
                          }
                          disabled={installBusy}
                          className="rounded border border-border bg-primary/80 px-1.5 py-0.5 text-[10px] text-white hover:bg-primary disabled:opacity-50"
                        >
                          {t("installSuggestedLibrary")}
                        </button>
                      </div>
                    )}
                  </div>
                ))}
                <button
                  type="button"
                  onClick={() =>
                    onSuggestFixWithAi(
                      "Fix the following Modelica compile error and suggest corrected code: " +
                        jitResult.errors.join(" ")
                    )
                  }
                  className="mt-2 rounded bg-primary/80 px-2 py-0.5 text-xs text-white hover:bg-primary"
                >
                  {t("suggestFixWithAi")}
                </button>
              </div>
            )}
            {jitResult?.warnings && jitResult.warnings.length > 0 && (
              <div
                className={`space-y-0.5 ${jitResult.success ? "mt-2" : "mt-1"}`}
              >
                {jitResult.warnings.map((w, i) => (
                  <div key={i} className="text-[var(--warning-text)]">
                    {w.path}:{w.line}:{w.column} {w.message}
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {jitResult && (
          <>
            <SimulationSectionHeader
              title={t("sectionVariables")}
              expanded={variablesExpanded}
              onToggle={onVariablesExpandedToggle}
              badge={
                totalVarCount > 0 ? (
                  <span className="ml-1 rounded bg-[var(--text-muted)]/15 px-1.5 text-[10px] text-[var(--text-muted)]">
                    {totalVarCount}
                  </span>
                ) : undefined
              }
            />
            {variablesExpanded && (
              <div className="space-y-3 px-3 py-2 text-xs">
                {(jitResult.state_vars?.length ?? 0) > 0 && (
                  <div>
                    <div className="mb-1 text-[10px] uppercase tracking-wide text-[var(--text-muted)]">
                      state
                    </div>
                    <div className="space-y-0.5">
                      {jitResult.state_vars!.map((name) => (
                        <button
                          key={`state:${name}`}
                          type="button"
                          className={`block w-full rounded px-2 py-0.5 text-left font-mono text-[11px] ${
                            selectedSymbol === name
                              ? "bg-primary/20 text-primary"
                              : "text-[var(--text)] hover:bg-white/5"
                          }`}
                          onClick={() => onFocusSymbol?.(name)}
                        >
                          {name}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
                {(jitResult.output_vars?.length ?? 0) > 0 && (
                  <div>
                    <div className="mb-1 text-[10px] uppercase tracking-wide text-[var(--text-muted)]">
                      output
                    </div>
                    <div className="space-y-0.5">
                      {jitResult.output_vars!.map((name) => (
                        <button
                          key={`output:${name}`}
                          type="button"
                          className={`block w-full rounded px-2 py-0.5 text-left font-mono text-[11px] ${
                            selectedSymbol === name
                              ? "bg-primary/20 text-primary"
                              : "text-[var(--text)] hover:bg-white/5"
                          }`}
                          onClick={() => onFocusSymbol?.(name)}
                        >
                          {name}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
                {totalVarCount === 0 && (
                  <div className="text-[11px] italic text-[var(--text-muted)]">
                    {t("runJitFirst")}
                  </div>
                )}
              </div>
            )}
          </>
        )}

        {testAllResults !== null && testSummary && (
          <>
            <SimulationSectionHeader
              title={t("sectionTestResults")}
              expanded={testResultsExpanded}
              onToggle={onTestResultsExpandedToggle}
              badge={
                testSummary.failed > 0 ? (
                  <span className="ml-1 rounded bg-[var(--danger-text)]/15 px-1.5 text-[10px] text-[var(--danger-text)]">
                    {testSummary.failed} failed
                  </span>
                ) : (
                  <span className="ml-1 rounded bg-[var(--success-text)]/15 px-1.5 text-[10px] text-[var(--success-text)]">
                    {testSummary.passed} passed
                  </span>
                )
              }
              toolbar={
                <button
                  type="button"
                  className="rounded border border-border px-1.5 py-0.5 text-[10px] theme-button-secondary"
                  onClick={() =>
                    void navigator.clipboard.writeText(testSummary.text)
                  }
                >
                  {t("copyTestAllOutput")}
                </button>
              }
            />
            {testResultsExpanded && (
              <div className="space-y-0.5 px-3 py-2 text-xs">
                {testAllResults.map((r, i) => (
                  <div
                    key={i}
                    className={
                      r.success
                        ? "text-[var(--success-text)]"
                        : "text-[var(--danger-text)]"
                    }
                  >
                    {r.success ? "\u2713" : "\u2717"} {r.path}
                    {!r.success && r.errors.length > 0 && (
                      <div className="pl-3 font-mono text-[11px] text-[var(--warning-text)]">
                        {r.errors[0]}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </>
        )}

        {!jitResult && testAllResults === null && (
          <div className="px-3 py-10 text-center text-xs text-[var(--text-muted)]">
            {t("jitStatusNotRun")}
          </div>
        )}
      </div>
    </div>
  );
}
