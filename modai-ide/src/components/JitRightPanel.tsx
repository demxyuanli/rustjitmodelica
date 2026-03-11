import { t } from "../i18n";
import { SelfIterateUI } from "./SelfIterateUI";
import type { JitRightTab } from "../hooks/useJitLayout";
import { IconButton } from "./IconButton";
import { AppIcon } from "./Icon";
import { AIPanel, type AIPanelProps } from "./AIPanel";

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
}

export function JitRightPanel({
  activeTab, onTabChange, targetPrefill, onClearPrefill, repoRoot,
  onDiffGenerated, onRunResult, openFilePaths, aiPanelProps,
  currentFilePath, currentSelectionText, onInsertAi,
}: JitRightPanelProps) {

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
      </div>
    </div>
  );
}
