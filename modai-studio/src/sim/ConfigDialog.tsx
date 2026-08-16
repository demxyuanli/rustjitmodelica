import { useEffect, useState } from "react";
import { Check, X } from "lucide-react";
import type { SimulationDefaults } from "../api/types";
import { t, type MessageKey } from "../i18n";
import { IconButton } from "../shell/IconButton";

const SOLVERS: {
  id: string;
  available: boolean;
  labelKey: MessageKey;
  descKey: MessageKey;
  whyKey?: MessageKey;
}[] = [
  { id: "rk45", available: true, labelKey: "solverRk45", descKey: "solverDescRk45" },
  { id: "rk4", available: true, labelKey: "solverRk4", descKey: "solverDescRk4" },
  { id: "implicit", available: true, labelKey: "solverImplicit", descKey: "solverDescImplicit" },
  { id: "radau", available: true, labelKey: "solverRadau", descKey: "solverDescRadau" },
  { id: "qss", available: true, labelKey: "solverQss", descKey: "solverDescQss" },
  {
    id: "cvode",
    available: false,
    labelKey: "solverCvode",
    descKey: "solverDescCvode",
    whyKey: "solverNeedSundials",
  },
  {
    id: "ida",
    available: false,
    labelKey: "solverIda",
    descKey: "solverDescIda",
    whyKey: "solverNeedSundials",
  },
];

function optNum(raw: string): number | null {
  const n = Number(raw);
  return raw.trim() && Number.isFinite(n) && n > 0 ? n : null;
}

export function ConfigDialog({
  open,
  defaults,
  onClose,
  onApply,
}: {
  open: boolean;
  defaults: SimulationDefaults;
  onClose: () => void;
  onApply: (next: SimulationDefaults) => void;
}) {
  const [stopTime, setStopTime] = useState(String(defaults.stopTime));
  const [intervals, setIntervals] = useState(String(defaults.numberOfIntervals));
  const [solver, setSolver] = useState(defaults.solver);
  const [tolerance, setTolerance] = useState(String(defaults.tolerance));
  const [maxStep, setMaxStep] = useState(defaults.maxStep != null ? String(defaults.maxStep) : "");
  const [minStep, setMinStep] = useState(defaults.minStep != null ? String(defaults.minStep) : "");
  const [logged, setLogged] = useState((defaults.loggedVariables ?? []).join(", "));

  useEffect(() => {
    if (!open) return;
    setStopTime(String(defaults.stopTime));
    setIntervals(String(defaults.numberOfIntervals));
    setSolver(defaults.solver);
    setTolerance(String(defaults.tolerance));
    setMaxStep(defaults.maxStep != null ? String(defaults.maxStep) : "");
    setMinStep(defaults.minStep != null ? String(defaults.minStep) : "");
    setLogged((defaults.loggedVariables ?? []).join(", "));
  }, [open, defaults]);

  useEffect(() => {
    if (!open) return;
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") {
        ev.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const selected = SOLVERS.find((s) => s.id === solver) ?? SOLVERS[0]!;
  const apply = () => {
    const st = Number(stopTime);
    const ni = Math.max(1, Math.round(Number(intervals)));
    const tol = Number(tolerance);
    onApply({
      stopTime: Number.isFinite(st) && st > 0 ? st : defaults.stopTime,
      numberOfIntervals: Number.isFinite(ni) ? ni : defaults.numberOfIntervals,
      solver: selected.available ? solver : defaults.solver,
      tolerance: Number.isFinite(tol) && tol > 0 ? tol : defaults.tolerance,
      maxStep: optNum(maxStep),
      minStep: optNum(minStep),
      loggedVariables: logged
        .split(/[,;\s]+/)
        .map((s) => s.trim())
        .filter(Boolean),
    });
    onClose();
  };

  return (
    <div className="settings-backdrop" role="presentation" onClick={onClose}>
      <div
        className="settings-dialog export-wizard"
        role="dialog"
        aria-modal="true"
        aria-label={t("configTitle")}
        onClick={(ev) => ev.stopPropagation()}
      >
        <header className="settings-header">
          <h2>{t("configTitle")}</h2>
          <IconButton icon={X} labelKey="settingsClose" showLabel={false} onClick={onClose} />
        </header>
        <section className="settings-section">
          <p className="settings-hint">{t("configHint")}</p>
          <label className="tool-field">
            <span>{t("stopTime")}</span>
            <input type="number" step="0.1" min="0" value={stopTime} onChange={(e) => setStopTime(e.target.value)} />
          </label>
          <label className="tool-field">
            <span>{t("intervals")}</span>
            <input type="number" min="1" step="1" value={intervals} onChange={(e) => setIntervals(e.target.value)} />
          </label>
          <label className="tool-field">
            <span>{t("solver")}</span>
            <select value={solver} onChange={(e) => setSolver(e.target.value)}>
              {SOLVERS.map((s) => (
                <option key={s.id} value={s.id} disabled={!s.available}>
                  {t(s.labelKey)}
                  {!s.available && s.whyKey ? ` (${t(s.whyKey)})` : ""}
                </option>
              ))}
            </select>
          </label>
          <p className="settings-hint">
            {t(selected.descKey)}
            {selected.whyKey && !selected.available ? ` ${t(selected.whyKey)}` : ""}
          </p>
          <label className="tool-field">
            <span>{t("tolerance")}</span>
            <input type="number" step="any" min="0" value={tolerance} onChange={(e) => setTolerance(e.target.value)} />
          </label>
          <label className="tool-field">
            <span>{t("maxStep")}</span>
            <input type="number" step="any" min="0" value={maxStep} onChange={(e) => setMaxStep(e.target.value)} placeholder="auto" />
          </label>
          <label className="tool-field">
            <span>{t("minStep")}</span>
            <input type="number" step="any" min="0" value={minStep} onChange={(e) => setMinStep(e.target.value)} placeholder="auto" />
          </label>
          <label className="tool-field">
            <span>{t("loggedVars")}</span>
            <input type="text" value={logged} onChange={(e) => setLogged(e.target.value)} placeholder="c.v, gain.y" />
          </label>
        </section>
        <footer className="settings-footer">
          <IconButton icon={X} labelKey="settingsClose" onClick={onClose} />
          <IconButton icon={Check} labelKey="configApply" primary onClick={apply} />
        </footer>
      </div>
    </div>
  );
}
