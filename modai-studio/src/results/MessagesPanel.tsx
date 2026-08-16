import { useMemo, useState } from "react";
import { Cpu, Lightbulb, List, Unplug } from "lucide-react";
import type { StudioDiagnostic, StudioReport } from "../api/types";
import {
  diagnosticSource,
  formatDiagnostic,
  type DiagnosticSource,
} from "../advisor/format";
import { t, type MessageKey } from "../i18n";
import { IconButton } from "../shell/IconButton";

type MsgFilter = "all" | "compiler" | "advisor" | "connection";

function matchesFilter(source: DiagnosticSource, filter: MsgFilter): boolean {
  if (filter === "all") return true;
  if (filter === "compiler") return source === "compiler" || source === "structure";
  return source === filter;
}

function severityClass(d: StudioDiagnostic, isError: boolean): string {
  if (isError || d.severity === "error") return "error";
  if (diagnosticSource(d) === "structure") return "warn-structure";
  if (d.severity === "info") return "info";
  return "";
}

export function MessagesPanel({
  report,
  onMessageClick,
}: {
  report: StudioReport | null;
  onMessageClick: (d: { message: string; nodeId?: string }) => void;
}) {
  const [filter, setFilter] = useState<MsgFilter>("all");

  const rows = useMemo(() => {
    if (!report) return [];
    const out: { d: StudioDiagnostic; error: boolean }[] = [];
    for (const d of report.errors) out.push({ d, error: true });
    for (const d of report.warnings) out.push({ d, error: false });
    return out.filter((r) => matchesFilter(diagnosticSource(r.d), filter));
  }, [filter, report]);

  const counts = useMemo(() => {
    const all = [...(report?.errors ?? []), ...(report?.warnings ?? [])];
    const n = { all: all.length, compiler: 0, advisor: 0, connection: 0 };
    for (const d of all) {
      const s = diagnosticSource(d);
      if (s === "advisor") n.advisor += 1;
      else if (s === "connection") n.connection += 1;
      else n.compiler += 1;
    }
    return n;
  }, [report]);

  if (!report) {
    return <div style={{ color: "var(--muted)" }}>{t("resultsNoMessages")}</div>;
  }

  const chip = (id: MsgFilter, icon: typeof List, labelKey: MessageKey, n: number) => (
    <IconButton
      icon={icon}
      labelKey={labelKey}
      active={filter === id}
      title={`${t(labelKey)} (${n})`}
      onClick={() => setFilter(id)}
    />
  );

  return (
    <div className="messages-panel">
      <div className={`msg-item ${report.success ? "ok" : "error"}`}>
        [{report.stage}] {report.success ? "OK" : "Failed"}
        {report.stage === "validate_analyze"
          ? ` — ${t("validateModeAnalyze")}`
          : report.stage === "validate_full"
            ? ` — ${t("validateModeFull")}`
            : ""}
      </div>
      <div className="msg-filters">
        {chip("all", List, "msgAll", counts.all)}
        {chip("compiler", Cpu, "msgCompiler", counts.compiler)}
        {chip("advisor", Lightbulb, "msgAdvisor", counts.advisor)}
        {chip("connection", Unplug, "msgConnection", counts.connection)}
      </div>
      {rows.length ? (
        rows.map(({ d, error }, i) => {
          const fmt = formatDiagnostic(d);
          return (
            <button
              key={`${fmt.source}_${fmt.rule ?? "x"}_${i}`}
              type="button"
              className={`msg-item ${severityClass(d, error)}`}
              style={{ display: "block", width: "100%", textAlign: "left", background: "transparent" }}
              onClick={() => onMessageClick({ message: fmt.title, nodeId: d.nodeId })}
            >
              <div className="msg-line">
                {fmt.rule ? <span className="msg-rule">{fmt.rule}</span> : null}
                <span className="msg-source">{fmt.source}</span>
                <span>{fmt.title}</span>
              </div>
              {fmt.hint ? <div className="msg-hint">{fmt.hint}</div> : null}
            </button>
          );
        })
      ) : (
        <div style={{ color: "var(--muted)" }}>{t("resultsNoMessages")}</div>
      )}
    </div>
  );
}
