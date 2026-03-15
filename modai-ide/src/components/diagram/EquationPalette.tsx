import React, { useCallback } from "react";
import { t } from "../../i18n";

export const EQUATION_DRAG_TYPE = "application/modelica-equation-symbol";

export interface EquationSymbol {
  id: string;
  label: string;
  category: "arithmetic" | "trigonometric" | "calculus" | "logic" | "relation" | "special";
  template: string;
}

const SYMBOLS: EquationSymbol[] = [
  { id: "add", label: "+", category: "arithmetic", template: "({a} + {b})" },
  { id: "sub", label: "-", category: "arithmetic", template: "({a} - {b})" },
  { id: "mul", label: "*", category: "arithmetic", template: "({a} * {b})" },
  { id: "div", label: "/", category: "arithmetic", template: "({a} / {b})" },
  { id: "pow", label: "^", category: "arithmetic", template: "({a} ^ {b})" },
  { id: "neg", label: "-x", category: "arithmetic", template: "-{a}" },
  { id: "sqrt", label: "sqrt", category: "arithmetic", template: "sqrt({a})" },
  { id: "abs", label: "abs", category: "arithmetic", template: "abs({a})" },
  { id: "sin", label: "sin", category: "trigonometric", template: "sin({a})" },
  { id: "cos", label: "cos", category: "trigonometric", template: "cos({a})" },
  { id: "tan", label: "tan", category: "trigonometric", template: "tan({a})" },
  { id: "asin", label: "asin", category: "trigonometric", template: "asin({a})" },
  { id: "acos", label: "acos", category: "trigonometric", template: "acos({a})" },
  { id: "atan", label: "atan", category: "trigonometric", template: "atan({a})" },
  { id: "exp", label: "exp", category: "trigonometric", template: "exp({a})" },
  { id: "log", label: "log", category: "trigonometric", template: "log({a})" },
  { id: "der", label: "der", category: "calculus", template: "der({a})" },
  { id: "pre", label: "pre", category: "calculus", template: "pre({a})" },
  { id: "edge", label: "edge", category: "calculus", template: "edge({a})" },
  { id: "initial", label: "initial", category: "special", template: "initial()" },
  { id: "reinit", label: "reinit", category: "special", template: "reinit({a}, {b})" },
  { id: "and", label: "and", category: "logic", template: "({a} and {b})" },
  { id: "or", label: "or", category: "logic", template: "({a} or {b})" },
  { id: "not", label: "not", category: "logic", template: "not {a}" },
  { id: "ifthen", label: "if", category: "logic", template: "if {a} then {b} else {c}" },
  { id: "eq", label: "=", category: "relation", template: "{a} = {b}" },
  { id: "lt", label: "<", category: "relation", template: "{a} < {b}" },
  { id: "gt", label: ">", category: "relation", template: "{a} > {b}" },
  { id: "leq", label: "<=", category: "relation", template: "{a} <= {b}" },
  { id: "geq", label: ">=", category: "relation", template: "{a} >= {b}" },
  { id: "when", label: "when", category: "special", template: "when {a} then\n  {b}\nend when" },
];

const CATEGORY_LABELS: Record<EquationSymbol["category"], string> = {
  arithmetic: "Arithmetic",
  trigonometric: "Functions",
  calculus: "Calculus",
  logic: "Logic",
  relation: "Relations",
  special: "Special",
};

interface EquationPaletteProps {
  onInsertSymbol?: (symbol: EquationSymbol) => void;
  compact?: boolean;
}

const SymbolButton = React.memo(function SymbolButton({
  symbol,
  onInsert,
}: {
  symbol: EquationSymbol;
  onInsert?: (s: EquationSymbol) => void;
}) {
  const handleDragStart = useCallback(
    (e: React.DragEvent) => {
      e.dataTransfer.setData(EQUATION_DRAG_TYPE, JSON.stringify(symbol));
      e.dataTransfer.effectAllowed = "copy";
    },
    [symbol],
  );

  return (
    <button
      type="button"
      className="px-2 py-1 rounded border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text)] text-xs font-mono hover:border-primary hover:bg-primary/10 transition-colors cursor-grab active:cursor-grabbing"
      title={symbol.template}
      draggable
      onDragStart={handleDragStart}
      onClick={() => onInsert?.(symbol)}
    >
      {symbol.label}
    </button>
  );
});

export function EquationPalette({ onInsertSymbol, compact = false }: EquationPaletteProps) {
  const grouped = Object.entries(
    SYMBOLS.reduce(
      (acc, sym) => {
        (acc[sym.category] ??= []).push(sym);
        return acc;
      },
      {} as Record<string, EquationSymbol[]>,
    ),
  );

  if (compact) {
    return (
      <div className="flex flex-wrap gap-1 p-1">
        {SYMBOLS.map((sym) => (
          <SymbolButton key={sym.id} symbol={sym} onInsert={onInsertSymbol} />
        ))}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2 p-2">
      <div className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
        {t("equationPalette")}
      </div>
      {grouped.map(([cat, syms]) => (
        <div key={cat}>
          <div className="text-[10px] text-[var(--text-muted)] mb-1">
            {CATEGORY_LABELS[cat as EquationSymbol["category"]] ?? cat}
          </div>
          <div className="flex flex-wrap gap-1">
            {syms.map((sym) => (
              <SymbolButton key={sym.id} symbol={sym} onInsert={onInsertSymbol} />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
