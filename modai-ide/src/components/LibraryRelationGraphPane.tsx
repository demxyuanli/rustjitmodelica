import React, { useCallback, useMemo, useState } from "react";
import { ZoomIn, ZoomOut, Maximize2, Maximize } from "lucide-react";
import { t } from "../i18n";
import { EquationGraphView } from "./EquationGraphView";
import { DependencyGraphModal } from "./DependencyGraphModal";
import type { JointPaperHandle } from "../utils/jointUtils";
import type { IconGraphics, AnnotationViewModel } from "../utils/modelicaAnnotation";
import { parseAnnotationViewModelForType } from "../utils/modelicaAnnotation";

const ICON_VIEW = 100;

function IconSymbolSvg({ graphics, size = 48 }: { graphics: IconGraphics; size?: number }) {
  const [x1, y1, x2, y2] = graphics.extent;
  const rangeX = x2 - x1 || 1;
  const rangeY = y2 - y1 || 1;
  const toSvg = (x: number, y: number) => {
    const sx = ((x - x1) / rangeX) * ICON_VIEW;
    const sy = ((y2 - y) / rangeY) * ICON_VIEW;
    return `${sx},${sy}`;
  };
  const strokeW = Math.max(1, ICON_VIEW / 25);
  const elements: React.ReactNode[] = [];
  graphics.shapes.forEach((s, i) => {
    const strokeColor =
      s.lineColorRGB != null ? `rgb(${s.lineColorRGB[0]}, ${s.lineColorRGB[1]}, ${s.lineColorRGB[2]})` : "currentColor";
    const fillColor =
      s.fillColorRGB != null ? `rgb(${s.fillColorRGB[0]}, ${s.fillColorRGB[1]}, ${s.fillColorRGB[2]})` : "currentColor";
    if (s.type === "Rectangle" && s.extent) {
      const [a, b, c, d] = s.extent;
      const x = ((a - x1) / rangeX) * ICON_VIEW;
      const y = ((y2 - d) / rangeY) * ICON_VIEW;
      const w = ((c - a) / rangeX) * ICON_VIEW;
      const h = ((d - b) / rangeY) * ICON_VIEW;
      elements.push(
        <rect
          key={i}
          x={x}
          y={y}
          width={w}
          height={h}
          fill={fillColor}
          fillOpacity={0.2}
          stroke={strokeColor}
          strokeWidth={strokeW}
        />
      );
    } else if (s.type === "Ellipse" && s.extent) {
      const [a, b, c, d] = s.extent;
      const cx = ((a + c) / 2 - x1) / rangeX * ICON_VIEW;
      const cy = (y2 - (b + d) / 2) / rangeY * ICON_VIEW;
      const rx = ((c - a) / 2 / rangeX) * ICON_VIEW;
      const ry = ((d - b) / 2 / rangeY) * ICON_VIEW;
      elements.push(
        <ellipse
          key={i}
          cx={cx}
          cy={cy}
          rx={rx}
          ry={ry}
          fill={fillColor}
          fillOpacity={0.2}
          stroke={strokeColor}
          strokeWidth={strokeW}
        />
      );
    } else if (s.type === "Line" && s.points && s.points.length >= 2) {
      const d = s.points.map((p) => toSvg(p[0], p[1])).join(" L ");
      elements.push(<path key={i} d={`M ${d}`} fill="none" stroke={strokeColor} strokeWidth={strokeW} />);
    } else if (s.type === "Polygon" && s.points && s.points.length >= 2) {
      const d = s.points.map((p) => toSvg(p[0], p[1])).join(" L ") + " Z";
      elements.push(
        <path
          key={i}
          d={`M ${d}`}
          fill={fillColor}
          fillOpacity={0.25}
          stroke={strokeColor}
          strokeWidth={strokeW}
        />
      );
    }
  });
  return (
    <svg width={size} height={size} viewBox={`0 0 ${ICON_VIEW} ${ICON_VIEW}`} className="text-[var(--text)] shrink-0" aria-hidden="true">
      {elements}
      {graphics.hasTextLabel && (
        <text
          x={ICON_VIEW / 2}
          y={20}
          textAnchor="middle"
          fontSize={18}
          fill="currentColor"
        >
          %name
        </text>
      )}
    </svg>
  );
}

interface LibraryRelationGraphPaneProps {
  code: string | null;
  modelName: string | null;
  projectDir?: string | null;
}

type ContentTab = "graph" | "icon" | "info" | "annotation";

export function LibraryRelationGraphPane({ code, modelName, projectDir }: LibraryRelationGraphPaneProps) {
  const [modalOpen, setModalOpen] = useState(false);
  const [paperHandle, setPaperHandle] = useState<JointPaperHandle | null>(null);
  const [contentTab, setContentTab] = useState<ContentTab>("graph");
  const canShowGraph = Boolean(code && modelName);

  const annotation: AnnotationViewModel = useMemo(
    () => (code && modelName ? parseAnnotationViewModelForType(code, modelName) : { rawEntries: [] }),
    [code, modelName]
  );
  const iconGraphics: IconGraphics | null = annotation.iconGraphics ?? null;

  const handleZoomIn = useCallback(() => paperHandle?.zoomIn(), [paperHandle]);
  const handleZoomOut = useCallback(() => paperHandle?.zoomOut(), [paperHandle]);
  const handleFitView = useCallback(() => paperHandle?.fitView(), [paperHandle]);

  return (
    <div className="flex h-full min-h-[220px] flex-col">
      <div className="panel-header-bar flex items-center justify-between border-b border-border">
        <div className="flex items-center gap-1">
          <button
            type="button"
            className={`rounded px-2 py-1.5 text-[11px] font-medium ${
              contentTab === "graph"
                ? "bg-[var(--surface-active)] text-[var(--text)]"
                : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
            }`}
            onClick={() => setContentTab("graph")}
          >
            {t("libraryInternalRelationGraph")}
          </button>
          <button
            type="button"
            className={`rounded px-2 py-1.5 text-[11px] font-medium ${
              contentTab === "icon"
                ? "bg-[var(--surface-active)] text-[var(--text)]"
                : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
            }`}
            onClick={() => setContentTab("icon")}
          >
            Icon
          </button>
          <button
            type="button"
            className={`rounded px-2 py-1.5 text-[11px] font-medium ${
              contentTab === "info"
                ? "bg-[var(--surface-active)] text-[var(--text)]"
                : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
            }`}
            onClick={() => setContentTab("info")}
          >
            {t("libraryDetailsTitle")}
          </button>
          <button
            type="button"
            className={`rounded px-2 py-1.5 text-[11px] font-medium ${
              contentTab === "annotation"
                ? "bg-[var(--surface-active)] text-[var(--text)]"
                : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
            }`}
            onClick={() => setContentTab("annotation")}
          >
            {t("libraryAnnotationLabel")}
          </button>
        </div>
        {canShowGraph && contentTab === "graph" && (
          <div className="flex items-center gap-[var(--toolbar-gap)]">
            <button type="button" className="toolbar-icon-btn flex rounded items-center justify-center hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]" title={t("zoomIn")} onClick={handleZoomIn}>
              <ZoomIn className="h-3 w-3" />
            </button>
            <button type="button" className="toolbar-icon-btn flex rounded items-center justify-center hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]" title={t("zoomOut")} onClick={handleZoomOut}>
              <ZoomOut className="h-3 w-3" />
            </button>
            <button type="button" className="toolbar-icon-btn flex rounded items-center justify-center hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]" title={t("fitView")} onClick={handleFitView}>
              <Maximize2 className="h-3 w-3" />
            </button>
            <div className="w-px bg-[var(--border)] mx-0.5 self-stretch min-h-[1em]" />
            <button type="button" className="toolbar-icon-btn flex rounded items-center justify-center hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]" title={t("expandToWindow")} onClick={() => setModalOpen(true)}>
              <Maximize className="h-3 w-3" />
            </button>
          </div>
        )}
      </div>
      <div className="flex-1 min-h-0 overflow-hidden">
        {contentTab === "graph" && (
          <>
            {!canShowGraph ? (
              <div className="flex h-full items-center justify-center text-sm text-[var(--text-muted)]">
                {t("libraryNoSelection")}
              </div>
            ) : (
              <EquationGraphView
                code={code ?? ""}
                modelName={modelName ?? ""}
                projectDir={projectDir}
                layoutOptions={{ algorithm: "layered", direction: "RIGHT" }}
                onReady={setPaperHandle}
              />
            )}
          </>
        )}
        {contentTab === "icon" && (
          <div className="flex h-full items-center justify-center bg-[var(--bg-elevated)]">
            {iconGraphics ? (
              <div className="flex flex-col items-center gap-2">
                <IconSymbolSvg graphics={iconGraphics} size={64} />
              </div>
            ) : (
              <p className="text-[11px] text-[var(--text-muted)] px-3">{t("libraryRelationGraphEmpty")}</p>
            )}
          </div>
        )}
        {contentTab === "info" && (
          <div className="h-full overflow-y-auto px-3 py-3 bg-[var(--bg-elevated)] space-y-3 text-[11px] text-[var(--text)]">
            {annotation.documentationInfo && (
              <section className="rounded border border-border bg-[var(--surface)] px-2.5 py-2">
                <div className="text-[11px] text-[var(--text-muted)]">
                  {annotation.documentationInfo.trim().startsWith("<") ? (
                    <div
                      className="prose prose-[0.7rem] max-w-none prose-invert"
                      dangerouslySetInnerHTML={{ __html: annotation.documentationInfo }}
                    />
                  ) : (
                    <div className="whitespace-pre-wrap">
                      {annotation.documentationInfo}
                    </div>
                  )}
                </div>
              </section>
            )}
            {annotation.experiment && (
              <section className="rounded border border-border bg-[var(--surface)] px-2.5 py-2">
                <div className="text-[11px] font-semibold mb-1">experiment</div>
                <div className="grid grid-cols-2 gap-x-3 gap-y-1 text-[11px] text-[var(--text-muted)]">
                  {annotation.experiment.startTime !== undefined && (
                    <div>StartTime = {annotation.experiment.startTime}</div>
                  )}
                  {annotation.experiment.stopTime !== undefined && (
                    <div>StopTime = {annotation.experiment.stopTime}</div>
                  )}
                  {annotation.experiment.interval !== undefined && (
                    <div>Interval = {annotation.experiment.interval}</div>
                  )}
                  {annotation.experiment.tolerance !== undefined && (
                    <div>Tolerance = {annotation.experiment.tolerance}</div>
                  )}
                </div>
              </section>
            )}
            {annotation.version && (
              <section className="rounded border border-border bg-[var(--surface)] px-2.5 py-2">
                <div className="text-[11px] font-semibold mb-1">version</div>
                <div className="space-y-0.5 text-[11px] text-[var(--text-muted)]">
                  {annotation.version.version && <div>version = {annotation.version.version}</div>}
                  {annotation.version.versionDate && <div>versionDate = {annotation.version.versionDate}</div>}
                  {annotation.version.versionBuild && <div>versionBuild = {annotation.version.versionBuild}</div>}
                </div>
              </section>
            )}
            {annotation.uses && annotation.uses.length > 0 && (
              <section className="rounded border border-border bg-[var(--surface)] px-2.5 py-2">
                <div className="text-[11px] font-semibold mb-1">uses</div>
                <ul className="space-y-0.5 text-[11px] text-[var(--text-muted)]">
                  {annotation.uses.map((u) => (
                    <li key={`${u.library}-${u.version ?? ""}`}>
                      {u.library}
                      {u.version ? ` (version=${u.version})` : ""}
                    </li>
                  ))}
                </ul>
              </section>
            )}
            {!annotation.documentationInfo && !annotation.experiment && !annotation.version && !annotation.uses && (
              <p className="text-[11px] text-[var(--text-muted)]">{t("libraryDetailsEmpty")}</p>
            )}
          </div>
        )}
        {contentTab === "annotation" && (
          <div className="h-full overflow-y-auto px-2 py-2 bg-[var(--bg-elevated)]">
            {annotation.rawEntries.length === 0 ? (
              <p className="text-[11px] text-[var(--text-muted)]">{t("libraryAnnotationEmpty")}</p>
            ) : (
              <div className="flex flex-col gap-1.5">
                {annotation.rawEntries.map((tag, i) => (
                  <div
                    key={`${tag.name}-${i}`}
                    className="w-full rounded border border-border bg-[var(--surface)] px-2 py-1.5 text-[11px]"
                    title={tag.value}
                  >
                    {!(tag.name === "Documentation.info" && tag.value.trim().startsWith("<")) && (
                      <div className="font-medium text-[var(--text)]">{tag.name}</div>
                    )}
                    <div className="mt-0.5 text-[var(--text-muted)]">
                      {tag.name === "Documentation.info" && tag.value.trim().startsWith("<") ? (
                        <div
                          className="prose prose-[0.7rem] max-w-none prose-invert"
                          dangerouslySetInnerHTML={{ __html: tag.value }}
                        />
                      ) : (
                        <div className="whitespace-pre-wrap">{tag.value}</div>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
      {canShowGraph && (
        <DependencyGraphModal
          open={modalOpen}
          onClose={() => setModalOpen(false)}
          code={code ?? ""}
          modelName={modelName ?? ""}
          projectDir={projectDir}
        />
      )}
    </div>
  );
}
