import { useEffect, useMemo, useState } from "react";
import { FolderOpen, Package, X } from "lucide-react";
import { pickExportDirectory } from "../api/studio";
import type { EmitOptions, ExportMode } from "../api/types";
import { t } from "../i18n";
import type { ExportOptionsPrefs } from "../settings/studioSettings";
import { IconButton } from "../shell/IconButton";

function expectedFiles(mode: ExportMode, coderPackage: boolean, modelId: string): string[] {
  const id = (modelId.trim() || "Model").replace(/[^A-Za-z0-9_]/g, "_");
  const files = ["model.c", "model.h"];
  if (coderPackage || mode === "coder") {
    files.push(
      "rml_step.h",
      "rml_step.c",
      "rml_main.c",
      "CMakeLists.txt",
      "INTEGRATION.txt",
      "interface.json",
    );
  }
  if (mode === "fmu-cs") {
    files.push("modelDescription.xml", "fmi2_cs.c", `${id}.fmu`);
  } else if (mode === "fmu-me") {
    files.push("modelDescription.xml", "fmi2_me.c", `${id}.fmu`);
  }
  return files;
}

export function ExportWizard({
  open,
  modelName,
  prefs,
  presetMode,
  busy,
  onClose,
  onExport,
}: {
  open: boolean;
  modelName: string;
  prefs: ExportOptionsPrefs;
  presetMode: ExportMode;
  busy: boolean;
  onClose: () => void;
  onExport: (opts: EmitOptions) => void;
}) {
  const [mode, setMode] = useState<ExportMode>(presetMode);
  const [coderPackage, setCoderPackage] = useState(true);
  const [modelIdentifier, setModelIdentifier] = useState("");
  const [guid, setGuid] = useState("");
  const [outDir, setOutDir] = useState("");

  useEffect(() => {
    if (!open) return;
    setMode(presetMode);
    setCoderPackage(prefs.coderPackage !== false);
    setModelIdentifier(prefs.modelIdentifier || "");
    setGuid(prefs.guid || "");
    setOutDir(prefs.lastOutDir || "");
  }, [open, presetMode, prefs]);

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

  const preview = useMemo(
    () => expectedFiles(mode, coderPackage, modelIdentifier || modelName),
    [mode, coderPackage, modelIdentifier, modelName],
  );

  if (!open) return null;

  const canExport = !!outDir.trim() && !busy;

  return (
    <div className="settings-backdrop" role="presentation" onClick={onClose}>
      <div
        className="settings-dialog export-wizard"
        role="dialog"
        aria-modal="true"
        aria-label={t("exportWizardTitle")}
        onClick={(ev) => ev.stopPropagation()}
      >
        <header className="settings-header">
          <h2>{t("exportWizardTitle")}</h2>
          <IconButton icon={X} labelKey="settingsClose" showLabel={false} onClick={onClose} disabled={busy} />
        </header>

        <section className="settings-section">
          <p className="settings-hint">{t("exportWizardHint")}</p>

          <fieldset className="export-mode-group">
            <legend>{t("exportMode")}</legend>
            <label>
              <input
                type="radio"
                name="exportMode"
                checked={mode === "coder"}
                onChange={() => setMode("coder")}
                disabled={busy}
              />{" "}
              {t("exportModeCoder")}
            </label>
            <label>
              <input
                type="radio"
                name="exportMode"
                checked={mode === "fmu-cs"}
                onChange={() => setMode("fmu-cs")}
                disabled={busy}
              />{" "}
              {t("exportModeFmuCs")}
            </label>
            <label>
              <input
                type="radio"
                name="exportMode"
                checked={mode === "fmu-me"}
                onChange={() => setMode("fmu-me")}
                disabled={busy}
              />{" "}
              {t("exportModeFmuMe")}
            </label>
          </fieldset>

          {mode !== "coder" ? (
            <label className="export-check">
              <input
                type="checkbox"
                checked={coderPackage}
                onChange={(ev) => setCoderPackage(ev.target.checked)}
                disabled={busy}
              />{" "}
              {t("exportIncludeCoder")}
            </label>
          ) : null}

          <label>
            {t("exportModelId")}
            <input
              type="text"
              value={modelIdentifier}
              placeholder={modelName}
              onChange={(ev) => setModelIdentifier(ev.target.value)}
              disabled={busy}
            />
          </label>

          <label>
            {t("exportGuid")}
            <input
              type="text"
              value={guid}
              placeholder={t("exportGuidPlaceholder")}
              onChange={(ev) => setGuid(ev.target.value)}
              disabled={busy}
            />
          </label>

          <label>
            {t("exportOutDir")}
            <div className="export-out-row">
              <input
                type="text"
                value={outDir}
                onChange={(ev) => setOutDir(ev.target.value)}
                disabled={busy}
              />
              <IconButton
                icon={FolderOpen}
                labelKey="exportBrowse"
                disabled={busy}
                onClick={() => {
                  void pickExportDirectory().then((dir) => {
                    if (dir) setOutDir(dir);
                  });
                }}
              />
            </div>
          </label>

          <div className="export-preview">
            <div className="export-preview-title">{t("exportPreview")}</div>
            <ul>
              {preview.map((f) => (
                <li key={f}>{f}</li>
              ))}
            </ul>
          </div>
        </section>

        <footer className="settings-footer">
          <IconButton icon={X} labelKey="settingsClose" onClick={onClose} disabled={busy} />
          <IconButton
            icon={Package}
            labelKey="exportRun"
            title={busy ? t("running") : t("exportRun")}
            primary
            disabled={!canExport}
            onClick={() =>
              onExport({
                mode,
                outDir: outDir.trim(),
                coderPackage: mode === "coder" ? true : coderPackage,
                modelIdentifier: modelIdentifier.trim() || undefined,
                guid: guid.trim() || undefined,
              })
            }
          />
        </footer>
      </div>
    </div>
  );
}
