import { lazy, Suspense } from "react";
import { t } from "../i18n";
import { SelfIterateUI } from "./SelfIterateUI";
import type { JitRightTab } from "../hooks/useJitLayout";
import { IconButton } from "./IconButton";
import { AppIcon } from "./Icon";
import { AIPanel, type AIPanelProps } from "./AIPanel";
import type { DiffTarget } from "./DiffView";

const DiffView = lazy(() => import("./DiffView").then((m) => ({ default: m.DiffView })));

export type JitDiffTarget =
  | (DiffTarget & { type?: "git" })
  | { type: "iteration"; iterationId: number; unifiedDiff: string; title?: string };

function isGitDiffTarget(t: JitDiffTarget | null): t is DiffTarget {
  return t != null && "projectDir" in t && "relativePath" in t;
}

interface JitRightPanelProps {
  activeTab: JitRightTab;
  onTabChange: (tab: JitRightTab) => void;
  targetPrefill?: string | null;
  onClearPrefill?: () => void;
  repoRoot?: string | null;
  onDiffGenerated?: (diff: string) => void;
  onRunResult?: (result: unknown) => void;
  openFilePaths?: string[];
  aiPanelProps?: AIPanelProps;
  currentFilePath?: string | null;
  currentSelectionText?: string | null;
  onInsertAi?: () => void;
  jitDiffTarget?: JitDiffTarget | null;
  onCloseDiff?: () => void;
  onOpenInEditor?: (relativePath: string) => void;
  contentByPath?: Record<string, string>;
  onViewIterationDiff?: (iterationId: number, unifiedDiff: string, title?: string) => void;
}

export function JitRightPanel({
  activeTab, onTabChange, targetPrefill, onClearPrefill, repoRoot,
  onDiffGenerated, onRunResult, openFilePaths, aiPanelProps,
  currentFilePath, currentSelectionText, onInsertAi,
  jitDiffTarget, onCloseDiff, onOpenInEditor, contentByPath, onViewIterationDiff,
}: JitRightPanelProps) {
  const gitDiffTarget = jitDiffTarget != null && isGitDiffTarget(jitDiffTarget) ? jitDiffTarget : null;
  const iterationDiff = jitDiffTarget != null && jitDiffTarget.type === "iteration" ? jitDiffTarget : null;
  const currentFileContent =
    gitDiffTarget && contentByPath
      ? (contentByPath[gitDiffTarget.relativePath.replace(/\\/g, "/")] ?? null)
      : null;

  return (
    <div className="flex flex-col h-full overflow-hidden bg-surface-alt">
      <div className="shrink-0 flex border-b border-border justify-around py-0.5">
        <IconButton
          icon={<AppIcon name="iterate" aria-hidden="true" />}
          variant="tab"
          size="xs"
          active={activeTab === "iterate"}
          onClick={() => onTabChange("iterate")}
          title={t("selfIterate")}
          aria-label={t("selfIterate")}
        />
        <IconButton
          icon={<AppIcon name="ai" aria-hidden="true" />}
          variant="tab"
          size="xs"
          active={activeTab === "ai"}
          onClick={() => onTabChange("ai")}
          title={t("aiCoding")}
          aria-label={t("aiCoding")}
        />
        <IconButton
          icon={<AppIcon name="sourceControl" aria-hidden="true" />}
          variant="tab"
          size="xs"
          active={activeTab === "diff"}
          onClick={() => onTabChange("diff")}
          title={t("sourceControl")}
          aria-label="Diff"
        />
      </div>
      <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
        {activeTab === "iterate" && (
          <SelfIterateUI
            fullScreen
            targetPrefill={targetPrefill}
            onClearPrefill={onClearPrefill}
            repoRoot={repoRoot}
            onDiffGenerated={onDiffGenerated}
            onRunResult={onRunResult}
            initialContextFiles={openFilePaths}
            onViewIterationDiff={onViewIterationDiff}
          />
        )}
        {activeTab === "ai" && (
          <div className="flex-1 overflow-auto p-3 scroll-vscode">
            {aiPanelProps ? (
              <AIPanel
                {...aiPanelProps}
                projectDir={undefined}
                repoRoot={repoRoot}
                currentFilePath={currentFilePath ?? undefined}
                currentSelectionText={currentSelectionText ?? undefined}
                lastJitErrorText={undefined}
                onInsert={onInsertAi ?? aiPanelProps.onInsert}
              />
            ) : (
              <div className="text-xs text-[var(--text-muted)]">
                {t("aiCoding")}
              </div>
            )}
          </div>
        )}
        {activeTab === "diff" && (
          <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
            {iterationDiff ? (
              <Suspense fallback={<div className="p-3 text-[var(--text-muted)] text-xs">{t("loading")}</div>}>
                <DiffView
                  diffTarget={null}
                  iterationUnifiedDiff={iterationDiff.unifiedDiff}
                  iterationTitle={iterationDiff.title}
                  currentFileContent={null}
                  currentFilePath={currentFilePath ?? null}
                  onClose={onCloseDiff ?? (() => {})}
                  onOpenInEditor={onOpenInEditor}
                />
              </Suspense>
            ) : gitDiffTarget ? (
              <Suspense fallback={<div className="p-3 text-[var(--text-muted)] text-xs">{t("loading")}</div>}>
                <DiffView
                  diffTarget={gitDiffTarget}
                  currentFileContent={currentFileContent}
                  currentFilePath={currentFilePath ?? null}
                  onClose={onCloseDiff ?? (() => {})}
                  onOpenInEditor={onOpenInEditor}
                />
              </Suspense>
            ) : (
              <div className="p-4 text-sm text-[var(--text-muted)]">
                {t("noFileSelected")}. Open a file diff from Source Control or iteration history.
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
