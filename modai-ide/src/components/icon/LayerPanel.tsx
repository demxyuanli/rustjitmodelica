import { useState } from "react";
import { Eye, EyeOff, GripVertical, Lock, Unlock } from "lucide-react";
import type { GraphicItem } from "../DiagramSvgRenderer";
import { t } from "../../i18n";

export interface LayerPanelProps {
  graphics: GraphicItem[];
  selectedIndices: number[];
  readOnly: boolean;
  onSelectLayer: (index: number, additive: boolean) => void;
  onToggleVisibility: (index: number) => void;
  onToggleLock: (index: number) => void;
  onReorder: (fromIndex: number, toIndex: number) => void;
}

function layerLabel(item: GraphicItem, index: number): string {
  if (item.type === "Group") {
    return `${t("shapeGroup")} (${item.children.length})`;
  }
  return `${item.type} #${index + 1}`;
}

export function LayerPanel({
  graphics,
  selectedIndices,
  readOnly,
  onSelectLayer,
  onToggleVisibility,
  onToggleLock,
  onReorder,
}: LayerPanelProps) {
  const [dragIndex, setDragIndex] = useState<number | null>(null);

  const rows = graphics.map((graphic, index) => ({ graphic, index }));

  const handleDragStart = (index: number) => (e: React.DragEvent) => {
    if (readOnly) {
      e.preventDefault();
      return;
    }
    setDragIndex(index);
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", String(index));
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
  };

  const handleDropOn = (targetIndex: number) => (e: React.DragEvent) => {
    e.preventDefault();
    if (readOnly) return;
    const raw = e.dataTransfer.getData("text/plain");
    const from = Number.parseInt(raw, 10);
    if (!Number.isFinite(from) || from === targetIndex || from < 0 || from >= graphics.length) {
      setDragIndex(null);
      return;
    }
    onReorder(from, targetIndex);
    setDragIndex(null);
  };

  const handleDragEnd = () => setDragIndex(null);

  return (
    <div className="flex flex-col min-h-0 border-r border-[var(--border)] bg-[var(--bg-elevated)] w-52 shrink-0">
      <div className="px-2 py-1.5 text-[10px] font-medium text-[var(--text-muted)] border-b border-[var(--border)]">
        {t("layerPanelTitle")}
      </div>
      <div className="flex-1 overflow-y-auto p-1 space-y-0.5">
        {rows.length === 0 && (
          <div className="text-[10px] text-[var(--text-muted)] px-2 py-2">{t("layerPanelEmpty")}</div>
        )}
        {rows.map(({ graphic, index }) => {
          const selected = selectedIndices.includes(index);
          const hidden = !!graphic.layerHidden;
          const locked = !!graphic.layerLocked;
          return (
            <div
              key={`layer-${index}`}
              draggable={!readOnly}
              onDragStart={handleDragStart(index)}
              onDragOver={handleDragOver}
              onDrop={handleDropOn(index)}
              onDragEnd={handleDragEnd}
              className={`flex items-center gap-0.5 rounded border px-0.5 py-1 text-[10px] ${
                selected ? "border-primary bg-primary/10" : "border-[var(--border)] bg-[var(--surface)]"
              } ${dragIndex === index ? "opacity-60" : ""}`}
            >
              <span className="text-[var(--text-muted)] cursor-grab shrink-0" title={t("layerPanelDragReorder")}>
                <GripVertical className="h-3 w-3" />
              </span>
              <button
                type="button"
                disabled={readOnly}
                className="shrink-0 p-0.5 rounded hover:bg-white/10 text-[var(--text-muted)]"
                title={hidden ? t("layerShow") : t("layerHide")}
                onClick={(e) => {
                  e.stopPropagation();
                  onToggleVisibility(index);
                }}
              >
                {hidden ? <EyeOff className="h-3 w-3" /> : <Eye className="h-3 w-3" />}
              </button>
              <button
                type="button"
                disabled={readOnly}
                className="shrink-0 p-0.5 rounded hover:bg-white/10 text-[var(--text-muted)]"
                title={locked ? t("layerUnlock") : t("layerLock")}
                onClick={(e) => {
                  e.stopPropagation();
                  onToggleLock(index);
                }}
              >
                {locked ? <Lock className="h-3 w-3" /> : <Unlock className="h-3 w-3" />}
              </button>
              <button
                type="button"
                className="flex-1 min-w-0 text-left truncate text-[var(--text)] hover:text-[var(--primary)]"
                onClick={(e) => {
                  onSelectLayer(index, e.metaKey || e.ctrlKey || e.shiftKey);
                }}
              >
                {layerLabel(graphic, index)}
              </button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
