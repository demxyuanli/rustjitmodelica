import { t } from "../i18n";
import type { JitBottomTab } from "../hooks/useJitLayout";
import { IconButton } from "./IconButton";
import { AppIcon } from "./Icon";

interface TestRunOutput {
  name: string;
  passed: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  durationMs: number;
}

interface JitBottomPanelProps {
  activeTab: JitBottomTab;
  onTabChange: (tab: JitBottomTab) => void;
  buildOutput: string[];
  testResults: TestRunOutput[];
}

export function JitBottomPanel({ activeTab, onTabChange, buildOutput, testResults }: JitBottomPanelProps) {
  const TAB_ITEMS: { id: JitBottomTab; label: string }[] = [
    { id: "output", label: t("jitBottomOutput" as Parameters<typeof t>[0]) },
    { id: "testResults", label: t("jitBottomTests" as Parameters<typeof t>[0]) },
  ];

  return (
    <div className="flex flex-col h-full overflow-hidden bg-surface-alt">
      <div className="flex items-center justify-start gap-1 px-2 py-0.5 border-b border-border shrink-0">
        <IconButton
          icon={<AppIcon name="run" aria-hidden="true" />}
          variant="tab"
          size="xs"
          active={activeTab === "output"}
          onClick={() => onTabChange("output")}
          title={TAB_ITEMS[0].label}
          aria-label={TAB_ITEMS[0].label}
        />
        <IconButton
          icon={<AppIcon name="tests" aria-hidden="true" />}
          variant="tab"
          size="xs"
          active={activeTab === "testResults"}
          onClick={() => onTabChange("testResults")}
          title={TAB_ITEMS[1].label}
          aria-label={TAB_ITEMS[1].label}
        />
      </div>
      <div className="flex-1 min-h-0 overflow-hidden">
        {activeTab === "output" && (
          <div className="flex-1 overflow-auto p-2 text-xs font-mono scroll-vscode h-full">
            {buildOutput.length === 0 ? (
              <div className="text-[var(--text-muted)]">No build output yet. Run iteration sandbox to see output.</div>
            ) : (
              buildOutput.map((line, i) => (
                <div key={i} className="text-[var(--text-muted)]">{line}</div>
              ))
            )}
          </div>
        )}

        {activeTab === "testResults" && (
          <div className="flex-1 overflow-auto p-2 text-xs h-full">
            {testResults.length === 0 ? (
              <div className="text-[var(--text-muted)] p-2">No test results yet. Run a test or iteration to see results.</div>
            ) : (
              <div className="overflow-auto">
                <table className="w-full text-xs">
                  <thead className="sticky top-0 bg-surface-alt">
                    <tr className="text-left text-[var(--text-muted)] border-b border-gray-700">
                      <th className="px-2 py-1 font-medium">Test</th>
                      <th className="px-2 py-1 font-medium w-16">Status</th>
                      <th className="px-2 py-1 font-medium w-16">Exit</th>
                      <th className="px-2 py-1 font-medium w-20">Duration</th>
                    </tr>
                  </thead>
                  <tbody>
                    {testResults.map((r, i) => (
                      <tr key={`${r.name}-${i}`} className="border-b border-gray-700/50">
                        <td className="px-2 py-1 text-[var(--text)]">{r.name.replace("TestLib/", "")}</td>
                        <td className="px-2 py-1">
                          <span className={r.passed ? "text-green-400" : "text-red-400"}>
                            {r.passed ? "Pass" : "Fail"}
                          </span>
                        </td>
                        <td className="px-2 py-1 text-[var(--text-muted)]">{r.exitCode}</td>
                        <td className="px-2 py-1 text-[var(--text-muted)]">{r.durationMs}ms</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
