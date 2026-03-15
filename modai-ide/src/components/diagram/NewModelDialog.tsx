import { useState, useCallback } from "react";
import { X } from "lucide-react";
import { t } from "../../i18n";

export interface NewModelDialogProps {
  projectDir: string | null;
  open: boolean;
  onClose: () => void;
  onCreateModel: (relativePath: string, content: string) => Promise<void>;
}

interface TemplateOption {
  id: string;
  label: string;
  description: string;
  generate: (name: string) => string;
}

const TEMPLATES: TemplateOption[] = [
  {
    id: "empty",
    label: "Empty Model",
    description: "A blank model with no equations",
    generate: (name) =>
      `model ${name}\n\nequation\n\nend ${name};\n`,
  },
  {
    id: "bouncing-ball",
    label: "Bouncing Ball",
    description: "Classic bouncing ball with gravity and restitution",
    generate: (name) =>
      `model ${name}\n  Real h(start = 1);\n  Real v(start = 0);\n  parameter Real g = 9.81;\n  parameter Real c = 0.9;\nequation\n  der(h) = v;\n  der(v) = -g;\n  when h <= 0 then\n    reinit(v, -c * pre(v));\n    reinit(h, 0);\n  end when;\nend ${name};\n`,
  },
  {
    id: "rc-circuit",
    label: "RC Circuit",
    description: "Simple resistor-capacitor circuit",
    generate: (name) =>
      `model ${name}\n  parameter Real R = 1000;\n  parameter Real C = 1e-6;\n  parameter Real V0 = 5.0;\n  Real v(start = 0);\n  Real i;\nequation\n  i = C * der(v);\n  R * i = V0 - v;\nend ${name};\n`,
  },
  {
    id: "thermal",
    label: "Thermal System",
    description: "Simple thermal mass with heat exchange",
    generate: (name) =>
      `model ${name}\n  parameter Real m = 1.0;\n  parameter Real cp = 1000;\n  parameter Real h_conv = 10;\n  parameter Real A = 0.1;\n  parameter Real T_amb = 293.15;\n  Real T(start = 373.15);\n  Real Q_flow;\nequation\n  Q_flow = h_conv * A * (T_amb - T);\n  m * cp * der(T) = Q_flow;\nend ${name};\n`,
  },
  {
    id: "spring-damper",
    label: "Spring-Damper",
    description: "Mass-spring-damper mechanical system",
    generate: (name) =>
      `model ${name}\n  parameter Real m = 1.0;\n  parameter Real k = 100;\n  parameter Real d = 2;\n  parameter Real F0 = 0;\n  Real x(start = 0.1);\n  Real v(start = 0);\nequation\n  der(x) = v;\n  m * der(v) = -k * x - d * v + F0;\nend ${name};\n`,
  },
];

function sanitizeModelName(raw: string): string {
  const s = raw.replace(/[^A-Za-z0-9_]/g, "");
  if (s.length === 0) return "NewModel";
  if (/^[0-9]/.test(s)) return "M" + s;
  return s[0].toUpperCase() + s.slice(1);
}

export function NewModelDialog({
  projectDir,
  open,
  onClose,
  onCreateModel,
}: NewModelDialogProps) {
  const [modelName, setModelName] = useState("NewModel");
  const [subDir, setSubDir] = useState("");
  const [templateId, setTemplateId] = useState("empty");
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const safeName = sanitizeModelName(modelName);

  const handleCreate = useCallback(async () => {
    if (!projectDir) {
      setError("No project directory open");
      return;
    }
    setCreating(true);
    setError(null);
    try {
      const tmpl = TEMPLATES.find((tp) => tp.id === templateId) ?? TEMPLATES[0];
      const content = tmpl.generate(safeName);
      const dir = subDir.replace(/\\/g, "/").replace(/^\/+|\/+$/g, "");
      const relativePath = dir ? `${dir}/${safeName}.mo` : `${safeName}.mo`;
      await onCreateModel(relativePath, content);
      onClose();
      setModelName("NewModel");
      setSubDir("");
      setTemplateId("empty");
    } catch (err) {
      setError(String(err));
    } finally {
      setCreating(false);
    }
  }, [projectDir, safeName, subDir, templateId, onCreateModel, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="bg-[var(--bg-elevated)] border border-[var(--border)] rounded-lg shadow-xl w-[520px] max-h-[80vh] flex flex-col overflow-hidden">
        <div className="px-5 py-4 border-b border-[var(--border)] flex items-center justify-between">
          <h2 className="text-base font-semibold text-[var(--text)]">{t("newModel")}</h2>
          <button type="button" className="p-1 text-[var(--text-muted)] hover:text-[var(--text)]" onClick={onClose} title={t("close")}>
            <X className="h-5 w-5" />
          </button>
        </div>

        <div className="flex-1 overflow-auto p-5 space-y-4">
          <div>
            <label className="block text-xs font-medium text-[var(--text-muted)] mb-1">{t("newModelName")}</label>
            <input
              type="text"
              className="w-full px-3 py-1.5 rounded border border-[var(--border)] bg-[var(--bg-input)] text-[var(--text)] text-sm focus:outline-none focus:border-primary"
              value={modelName}
              onChange={(e) => setModelName(e.target.value)}
              placeholder="NewModel"
              autoFocus
              onKeyDown={(e) => { if (e.key === "Enter" && !creating) handleCreate(); }}
            />
            {modelName !== safeName && (
              <p className="text-[10px] text-[var(--text-muted)] mt-0.5">
                Will use: <span className="text-[var(--text)]">{safeName}</span>
              </p>
            )}
          </div>

          <div>
            <label className="block text-xs font-medium text-[var(--text-muted)] mb-1">{t("newModelSubDir")}</label>
            <input
              type="text"
              className="w-full px-3 py-1.5 rounded border border-[var(--border)] bg-[var(--bg-input)] text-[var(--text)] text-sm focus:outline-none focus:border-primary"
              value={subDir}
              onChange={(e) => setSubDir(e.target.value)}
              placeholder="(project root)"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5">{t("newModelTemplate")}</label>
            <div className="grid grid-cols-1 gap-1.5">
              {TEMPLATES.map((tmpl) => (
                <button
                  key={tmpl.id}
                  type="button"
                  className={`text-left px-3 py-2 rounded border text-sm transition-colors ${
                    templateId === tmpl.id
                      ? "border-primary bg-primary/10 text-[var(--text)]"
                      : "border-[var(--border)] bg-transparent text-[var(--text-muted)] hover:border-primary/50 hover:text-[var(--text)]"
                  }`}
                  onClick={() => setTemplateId(tmpl.id)}
                >
                  <div className="font-medium text-xs">{tmpl.label}</div>
                  <div className="text-[10px] opacity-70 mt-0.5">{tmpl.description}</div>
                </button>
              ))}
            </div>
          </div>

          {error && (
            <p className="text-xs text-red-400">{error}</p>
          )}
        </div>

        <div className="px-5 py-3 border-t border-[var(--border)] flex justify-end gap-2">
          <button
            type="button"
            className="px-4 py-1.5 rounded text-xs text-[var(--text-muted)] hover:text-[var(--text)] border border-[var(--border)] hover:border-[var(--text-muted)]"
            onClick={onClose}
          >
            {t("cancel")}
          </button>
          <button
            type="button"
            className="px-4 py-1.5 rounded text-xs bg-primary text-white hover:opacity-90 disabled:opacity-50"
            disabled={!projectDir || creating || !safeName}
            onClick={handleCreate}
          >
            {creating ? t("creating") : t("create")}
          </button>
        </div>
      </div>
    </div>
  );
}
