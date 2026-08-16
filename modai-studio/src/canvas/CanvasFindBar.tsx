import { useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import type { DiagramNode } from "../api/types";
import { t } from "../i18n";
import { IconButton } from "../shell/IconButton";

/** Canvas find bar: filter instances and focus first match (P357 / F36). */
export function CanvasFindBar({
  open,
  nodes,
  onClose,
  onFocus,
}: {
  open: boolean;
  nodes: DiagramNode[];
  onClose: () => void;
  onFocus: (nodeId: string) => void;
}) {
  const [q, setQ] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQ("");
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  if (!open) return null;

  const needle = q.trim().toLowerCase();
  const hits = needle
    ? nodes.filter(
        (n) =>
          n.instanceName.toLowerCase().includes(needle) ||
          n.typeName.toLowerCase().includes(needle) ||
          n.catalogId.toLowerCase().includes(needle),
      )
    : [];

  return (
    <div className="canvas-find-bar">
      <input
        ref={inputRef}
        value={q}
        placeholder={t("findPlaceholder")}
        onChange={(ev) => setQ(ev.target.value)}
        onKeyDown={(ev) => {
          if (ev.key === "Escape") {
            onClose();
            return;
          }
          if (ev.key === "Enter" && hits[0]) {
            onFocus(hits[0]!.id);
          }
        }}
      />
      <span className="canvas-find-count">
        {needle ? `${hits.length}` : ""}
      </span>
      <IconButton icon={X} labelKey="settingsClose" className="canvas-find-close" showLabel={false} onClick={onClose} />
      {hits.length > 0 ? (
        <ul className="canvas-find-hits">
          {hits.slice(0, 8).map((n) => (
            <li key={n.id}>
              <button type="button" onClick={() => onFocus(n.id)}>
                {n.instanceName}
                <span>{n.typeName.split(".").slice(-1)[0]}</span>
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
