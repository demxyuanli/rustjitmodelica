import { useMemo, useState } from "react";

interface IterateDiffPreviewProps {
  diff: string;
  title?: string;
  defaultExpanded?: boolean;
}

export function IterateDiffPreview({
  diff,
  title = "Patch preview",
  defaultExpanded = false,
}: IterateDiffPreviewProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);

  const stats = useMemo(() => {
    let added = 0;
    let removed = 0;
    for (const line of diff.split("\n")) {
      if (line.startsWith("+++") || line.startsWith("---")) continue;
      if (line.startsWith("+")) added += 1;
      if (line.startsWith("-")) removed += 1;
    }
    return { added, removed };
  }, [diff]);

  return (
    <div className="rounded-lg border border-border bg-[var(--panel-muted-bg)] overflow-hidden">
      <button
        type="button"
        onClick={() => setExpanded((value) => !value)}
        className="w-full flex items-center justify-between px-3 py-2 text-xs text-left hover:bg-[var(--surface-hover)]"
      >
        <span className="font-medium text-[var(--text)]">{title}</span>
        <span className="text-[var(--text-muted)]">
          +{stats.added} / -{stats.removed} {expanded ? "▾" : "▸"}
        </span>
      </button>
      {expanded && (
        <div className="max-h-72 overflow-auto border-t border-border">
          <pre className="text-[11px] leading-5 p-3 font-mono whitespace-pre-wrap break-all">
            {diff.split("\n").map((line, index) => {
              let className = "block text-[var(--text)]";
              if (line.startsWith("+") && !line.startsWith("+++")) className = "block text-[var(--success-text)]";
              if (line.startsWith("-") && !line.startsWith("---")) className = "block text-[var(--danger-text)]";
              if (line.startsWith("@@") || line.startsWith("diff --git")) className = "block text-[var(--info-text)]";
              return (
                <span key={`${index}-${line}`} className={className}>
                  {line || " "}
                </span>
              );
            })}
          </pre>
        </div>
      )}
    </div>
  );
}
