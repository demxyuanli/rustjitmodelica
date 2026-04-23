import { useCallback, useEffect, useState } from "react";
import type { AnnotationPoint, CoordinateSystem } from "../diagramGraphicTypes";
import { t } from "../../i18n";

export interface DiagramCoordinateStripProps {
  readOnly: boolean;
  coordinateSystem?: CoordinateSystem;
  onCommit: (next: CoordinateSystem) => void;
}

function defaultExtent() {
  return {
    p1: { x: -100, y: -100 } as AnnotationPoint,
    p2: { x: 100, y: 100 } as AnnotationPoint,
  };
}

export function DiagramCoordinateStrip({ readOnly, coordinateSystem, onCommit }: DiagramCoordinateStripProps) {
  const ext = coordinateSystem?.extent ?? defaultExtent();
  const [p1x, setP1x] = useState(String(ext.p1.x));
  const [p1y, setP1y] = useState(String(ext.p1.y));
  const [p2x, setP2x] = useState(String(ext.p2.x));
  const [p2y, setP2y] = useState(String(ext.p2.y));
  const [preserve, setPreserve] = useState(Boolean(coordinateSystem?.preserveAspectRatio));
  const [scale, setScale] = useState(
    coordinateSystem?.initialScale != null ? String(coordinateSystem.initialScale) : "0.1",
  );

  useEffect(() => {
    const e = coordinateSystem?.extent ?? defaultExtent();
    setP1x(String(e.p1.x));
    setP1y(String(e.p1.y));
    setP2x(String(e.p2.x));
    setP2y(String(e.p2.y));
    setPreserve(Boolean(coordinateSystem?.preserveAspectRatio));
    setScale(coordinateSystem?.initialScale != null ? String(coordinateSystem.initialScale) : "0.1");
  }, [coordinateSystem]);

  const apply = useCallback(() => {
    const n1x = Number(p1x);
    const n1y = Number(p1y);
    const n2x = Number(p2x);
    const n2y = Number(p2y);
    const sc = Number(scale);
    if (![n1x, n1y, n2x, n2y, sc].every((v) => Number.isFinite(v))) return;
    onCommit({
      extent: { p1: { x: n1x, y: n1y }, p2: { x: n2x, y: n2y } },
      preserveAspectRatio: preserve || undefined,
      initialScale: sc,
    });
  }, [onCommit, p1x, p1y, p2x, p2y, preserve, scale]);

  return (
    <div className="shrink-0 flex flex-wrap items-center gap-2 px-2 py-1 border-b border-[var(--border)] bg-[var(--bg-elevated)] text-[10px]">
      <span className="text-[var(--text-muted)] uppercase tracking-wide">{t("diagramCoordinateSystem")}</span>
      <label className="flex items-center gap-1">
        <span className="text-[var(--text-muted)]">x1</span>
        <input
          type="number"
          value={p1x}
          disabled={readOnly}
          onChange={(e) => setP1x(e.target.value)}
          className="w-16 rounded bg-[var(--surface)] border border-[var(--border)] px-1 py-0.5"
        />
      </label>
      <label className="flex items-center gap-1">
        <span className="text-[var(--text-muted)]">y1</span>
        <input
          type="number"
          value={p1y}
          disabled={readOnly}
          onChange={(e) => setP1y(e.target.value)}
          className="w-16 rounded bg-[var(--surface)] border border-[var(--border)] px-1 py-0.5"
        />
      </label>
      <label className="flex items-center gap-1">
        <span className="text-[var(--text-muted)]">x2</span>
        <input
          type="number"
          value={p2x}
          disabled={readOnly}
          onChange={(e) => setP2x(e.target.value)}
          className="w-16 rounded bg-[var(--surface)] border border-[var(--border)] px-1 py-0.5"
        />
      </label>
      <label className="flex items-center gap-1">
        <span className="text-[var(--text-muted)]">y2</span>
        <input
          type="number"
          value={p2y}
          disabled={readOnly}
          onChange={(e) => setP2y(e.target.value)}
          className="w-16 rounded bg-[var(--surface)] border border-[var(--border)] px-1 py-0.5"
        />
      </label>
      <label className="flex items-center gap-1 cursor-pointer">
        <input type="checkbox" checked={preserve} disabled={readOnly} onChange={(e) => setPreserve(e.target.checked)} />
        <span>{t("diagramCsPreserve")}</span>
      </label>
      <label className="flex items-center gap-1">
        <span className="text-[var(--text-muted)]">{t("diagramCsScale")}</span>
        <input
          type="number"
          step="0.01"
          value={scale}
          disabled={readOnly}
          onChange={(e) => setScale(e.target.value)}
          className="w-16 rounded bg-[var(--surface)] border border-[var(--border)] px-1 py-0.5"
        />
      </label>
      {!readOnly && (
        <button
          type="button"
          className="rounded border border-[var(--border)] bg-[var(--surface)] px-2 py-0.5 hover:bg-white/10"
          onClick={apply}
        >
          {t("diagramCsApply")}
        </button>
      )}
    </div>
  );
}
