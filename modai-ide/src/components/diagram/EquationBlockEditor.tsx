import { useState, useCallback } from "react";
import { Plus, Palette, Trash2 } from "lucide-react";
import { t } from "../../i18n";
import { EquationPalette, EQUATION_DRAG_TYPE, type EquationSymbol } from "./EquationPalette";

export interface EquationEntry {
  id: string;
  text: string;
  isWhen?: boolean;
}

interface EquationBlockEditorProps {
  equations: EquationEntry[];
  readOnly?: boolean;
  onChange: (equations: EquationEntry[]) => void;
}

let nextEqId = 1;

function generateEqId(): string {
  return `eq_${Date.now()}_${nextEqId++}`;
}

export function EquationBlockEditor({
  equations,
  readOnly = false,
  onChange,
}: EquationBlockEditorProps) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showPalette, setShowPalette] = useState(false);

  const handleAdd = useCallback(() => {
    const newEq: EquationEntry = { id: generateEqId(), text: "" };
    onChange([...equations, newEq]);
    setEditingId(newEq.id);
  }, [equations, onChange]);

  const handleDelete = useCallback(
    (id: string) => {
      onChange(equations.filter((eq) => eq.id !== id));
      if (editingId === id) setEditingId(null);
    },
    [equations, onChange, editingId],
  );

  const handleUpdate = useCallback(
    (id: string, text: string) => {
      onChange(equations.map((eq) => (eq.id === id ? { ...eq, text } : eq)));
    },
    [equations, onChange],
  );

  const handleInsertSymbol = useCallback(
    (symbol: EquationSymbol) => {
      if (editingId) {
        const eq = equations.find((e) => e.id === editingId);
        if (eq) {
          const newText = eq.text ? `${eq.text} ${symbol.template}` : symbol.template;
          handleUpdate(editingId, newText);
        }
      } else if (equations.length > 0) {
        const last = equations[equations.length - 1];
        const newText = last.text ? `${last.text} ${symbol.template}` : symbol.template;
        handleUpdate(last.id, newText);
        setEditingId(last.id);
      }
    },
    [editingId, equations, handleUpdate],
  );

  const handleDrop = useCallback(
    (e: React.DragEvent, targetId: string) => {
      e.preventDefault();
      const raw = e.dataTransfer.getData(EQUATION_DRAG_TYPE);
      if (!raw) return;
      try {
        const symbol = JSON.parse(raw) as EquationSymbol;
        const eq = equations.find((eq2) => eq2.id === targetId);
        if (eq) {
          const newText = eq.text ? `${eq.text} ${symbol.template}` : symbol.template;
          handleUpdate(targetId, newText);
          setEditingId(targetId);
        }
      } catch { /* ignore */ }
    },
    [equations, handleUpdate],
  );

  const handleDragOver = useCallback((e: React.DragEvent) => {
    if (Array.from(e.dataTransfer.types as ArrayLike<string>).includes(EQUATION_DRAG_TYPE)) {
      e.preventDefault();
    }
  }, []);

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between px-2 py-1">
        <span className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
          {t("equationEditor")}
        </span>
        <div className="flex items-center gap-1">
          {!readOnly && (
            <>
              <button
                type="button"
                className={`p-1 rounded ${showPalette ? "bg-primary/20 text-primary" : "text-[var(--text-muted)] hover:text-[var(--text)]"}`}
                onClick={() => setShowPalette(!showPalette)}
                title={t("equationPalette")}
              >
                <Palette className="h-4 w-4" />
              </button>
              <button
                type="button"
                className="p-1 rounded bg-primary/20 text-primary hover:bg-primary/30"
                onClick={handleAdd}
                title={t("addEquation")}
              >
                <Plus className="h-4 w-4" />
              </button>
            </>
          )}
        </div>
      </div>

      {showPalette && !readOnly && (
        <div className="border border-[var(--border)] rounded mx-2 mb-1">
          <EquationPalette onInsertSymbol={handleInsertSymbol} compact />
        </div>
      )}

      <div className="space-y-1 px-2 max-h-[400px] overflow-auto">
        {equations.map((eq) => {
          const editing = editingId === eq.id;
          const isWhenBlock = eq.text.trimStart().startsWith("when ");

          return (
            <div
              key={eq.id}
              className={`group flex items-start gap-1 rounded border p-1.5 transition-colors ${
                editing
                  ? "border-primary bg-primary/5"
                  : isWhenBlock
                    ? "border-yellow-500/30 bg-yellow-500/5"
                    : "border-[var(--border)] hover:border-[var(--text-muted)]"
              }`}
              onClick={() => !readOnly && setEditingId(eq.id)}
              onDrop={(e) => handleDrop(e, eq.id)}
              onDragOver={handleDragOver}
            >
              <div className="flex-1 min-w-0">
                {editing && !readOnly ? (
                  <textarea
                    className="w-full bg-transparent text-[var(--text)] text-xs font-mono outline-none resize-none min-h-[24px]"
                    value={eq.text}
                    onChange={(e) => handleUpdate(eq.id, e.target.value)}
                    rows={Math.max(1, eq.text.split("\n").length)}
                    autoFocus
                    onKeyDown={(e) => {
                      if (e.key === "Escape") setEditingId(null);
                    }}
                  />
                ) : (
                  <pre className="text-xs font-mono text-[var(--text)] whitespace-pre-wrap break-all min-h-[16px]">
                    {eq.text || <span className="text-[var(--text-muted)] italic">empty equation</span>}
                  </pre>
                )}
              </div>
              {!readOnly && (
                <button
                  type="button"
                  className="shrink-0 opacity-0 group-hover:opacity-100 p-0.5 text-red-400 hover:text-red-300 transition-opacity"
                  onClick={(e) => { e.stopPropagation(); handleDelete(eq.id); }}
                  title={t("deleteEquation")}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </button>
              )}
            </div>
          );
        })}
        {equations.length === 0 && (
          <div className="text-center py-4 text-[10px] text-[var(--text-muted)]">
            {readOnly ? "No equations" : "Click + to add equations, drag symbols from palette"}
          </div>
        )}
      </div>
    </div>
  );
}
