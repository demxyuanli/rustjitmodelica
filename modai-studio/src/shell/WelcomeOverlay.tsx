import { useEffect, useMemo, useState } from "react";
import { BookOpen, FilePlus2, FolderOpen, Save, X } from "lucide-react";
import type { ExampleInfo, SimulationDefaults } from "../api/types";
import { listExamples, pickDirectory } from "../api/studio";
import {
  joinProjectDir,
  loadLastNewParent,
  sanitizeModelName,
  type NewProjectInit,
} from "../app/useStudioProject";
import { t, tf, type MessageKey } from "../i18n";
import welcomeHero from "../assets/welcome-hero.png";
import { exampleTitle } from "./ExamplesMenu";
import { exampleDomainLabelKey, groupExamples } from "./exampleGroups";
import { recentProjectFolder, type RecentProject } from "./recentProjects";
import { IconButton } from "./IconButton";

export type WelcomeTab = "examples" | "open" | "new";
export type WelcomeIntent = "new" | "saveAs";

const NEW_SOLVERS: { id: string; labelKey: MessageKey }[] = [
  { id: "rk45", labelKey: "solverRk45" },
  { id: "rk4", labelKey: "solverRk4" },
  { id: "implicit", labelKey: "solverImplicit" },
  { id: "radau", labelKey: "solverRadau" },
  { id: "qss", labelKey: "solverQss" },
];

const DEFAULT_NEW: SimulationDefaults = {
  stopTime: 1,
  numberOfIntervals: 500,
  solver: "rk45",
  tolerance: 1e-6,
  loggedVariables: [],
};

export function WelcomeOverlay({
  visible,
  initialTab = "examples",
  intent = "new",
  seedName = "Plant",
  seedSim,
  openSeq = 0,
  recentProjects,
  onClose,
  onOpenExample,
  onOpenProject,
  onOpenRecent,
  onNewProject,
}: {
  visible: boolean;
  initialTab?: WelcomeTab;
  intent?: WelcomeIntent;
  seedName?: string;
  seedSim?: SimulationDefaults;
  openSeq?: number;
  recentProjects: RecentProject[];
  onClose: () => void;
  onOpenExample: (id: string) => Promise<boolean> | boolean;
  onOpenProject: () => Promise<boolean> | boolean;
  onOpenRecent: (path: string) => Promise<boolean> | boolean;
  onNewProject: (init: NewProjectInit) => Promise<boolean> | boolean;
}) {
  const [items, setItems] = useState<ExampleInfo[]>([]);
  const [tab, setTab] = useState<WelcomeTab>(initialTab);
  const [modelName, setModelName] = useState("Plant");
  const [parentDir, setParentDir] = useState("");
  const [templateId, setTemplateId] = useState("");
  const [stopTime, setStopTime] = useState("1");
  const [intervals, setIntervals] = useState("500");
  const [solver, setSolver] = useState("rk45");
  const [tolerance, setTolerance] = useState("1e-6");

  useEffect(() => {
    if (!visible) return;
    setTab(initialTab);
    const last = loadLastNewParent();
    if (last) setParentDir(last);
    if (intent === "saveAs") {
      setModelName(seedName || "Plant");
      setTemplateId("");
      const sim = seedSim ?? DEFAULT_NEW;
      setStopTime(String(sim.stopTime));
      setIntervals(String(sim.numberOfIntervals));
      setSolver(sim.solver);
      setTolerance(String(sim.tolerance));
    }
    void listExamples()
      .then(setItems)
      .catch(() => setItems([]));
  }, [visible, initialTab, openSeq, intent, seedName, seedSim]);

  useEffect(() => {
    if (!visible) return;
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") {
        ev.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [visible, onClose]);

  const exampleGroups = useMemo(() => groupExamples(items), [items]);

  const projectPath = useMemo(
    () => joinProjectDir(parentDir, sanitizeModelName(modelName)),
    [parentDir, modelName],
  );

  const closeIfOk = async (ok: Promise<boolean> | boolean) => {
    if (await ok) onClose();
  };

  const browseParent = async () => {
    const dir = await pickDirectory();
    if (dir) setParentDir(dir);
  };

  const createProject = () => {
    if (!parentDir.trim()) return;
    const st = Number(stopTime);
    const ni = Math.max(1, Math.round(Number(intervals)));
    const tol = Number(tolerance);
    void closeIfOk(
      onNewProject({
        modelName,
        parentDir: parentDir.trim(),
        templateId: intent === "saveAs" ? null : templateId || null,
        simulationDefaults: {
          stopTime: Number.isFinite(st) && st > 0 ? st : DEFAULT_NEW.stopTime,
          numberOfIntervals: Number.isFinite(ni) ? ni : DEFAULT_NEW.numberOfIntervals,
          solver,
          tolerance: Number.isFinite(tol) && tol > 0 ? tol : DEFAULT_NEW.tolerance,
          loggedVariables: [],
        },
      }),
    );
  };

  if (!visible) return null;

  return (
    <div className="welcome-backdrop" role="presentation" onClick={onClose}>
      <div
        className="welcome-modal"
        role="dialog"
        aria-modal="true"
        aria-label={t("welcomeTitle")}
        onClick={(ev) => ev.stopPropagation()}
      >
        <div className="welcome-hero">
          <img src={welcomeHero} alt="" />
          <div className="welcome-hero-caption">
            <h2>{t("welcomeTitle")}</h2>
            <p>{t("welcomeHint")}</p>
          </div>
          <IconButton
            icon={X}
            labelKey="settingsClose"
            className="welcome-close"
            showLabel={false}
            onClick={onClose}
          />
        </div>
        <div className="welcome-body">
          <div className="welcome-tabs" role="tablist">
            {(
              [
                ["examples", "welcomeTabExamples", BookOpen],
                ["open", "welcomeTabOpen", FolderOpen],
                ["new", "welcomeTabNew", FilePlus2],
              ] as const
            ).map(([id, key, Icon]) => (
              <IconButton
                key={id}
                icon={Icon}
                labelKey={key}
                active={tab === id}
                onClick={() => setTab(id)}
              />
            ))}
          </div>
          <div className="welcome-tab-panel">
            {tab === "examples" ? (
              <div className="welcome-example-groups">
                {exampleGroups.map((group) => (
                  <section key={group.domain} className="welcome-example-group">
                    <h3>{t(exampleDomainLabelKey(group.domain))}</h3>
                    <ul>
                      {group.items.map((ex) => (
                        <li key={ex.id}>
                          <button
                            type="button"
                            className="welcome-example-row"
                            onClick={() => void closeIfOk(onOpenExample(ex.id))}
                          >
                            <strong>{exampleTitle(ex)}</strong>
                            <span>{ex.modelName}</span>
                          </button>
                        </li>
                      ))}
                    </ul>
                  </section>
                ))}
              </div>
            ) : null}
            {tab === "open" ? (
              <>
                <p className="welcome-section">{t("welcomeOpenHint")}</p>
                <div className="welcome-actions">
                  <IconButton
                    icon={FolderOpen}
                    labelKey="welcomeOpenBrowse"
                    onClick={() => void closeIfOk(onOpenProject())}
                  />
                </div>
                <div className="welcome-section">{t("recentProjects")}</div>
                {recentProjects.length ? (
                  <div className="welcome-card-grid welcome-card-grid-recent">
                    {recentProjects.map((p) => (
                      <button
                        key={p.path}
                        type="button"
                        className="welcome-tile"
                        title={p.path}
                        onClick={() => void closeIfOk(onOpenRecent(p.path))}
                      >
                        <strong>{p.modelName || recentProjectFolder(p.path)}</strong>
                        <span>{p.path}</span>
                      </button>
                    ))}
                  </div>
                ) : (
                  <p className="welcome-empty">{t("welcomeNoRecents")}</p>
                )}
              </>
            ) : null}
            {tab === "new" ? (
              <div className="welcome-new">
                <p className="welcome-section">
                  {t(intent === "saveAs" ? "welcomeSaveHint" : "welcomeNewHint")}
                </p>
                <label className="tool-field">
                  <span>{t("modelName")}</span>
                  <input value={modelName} onChange={(ev) => setModelName(ev.target.value)} />
                </label>
                <div className="welcome-new-path">
                  <span>{t("welcomeNewLocation")}</span>
                  <input
                    value={parentDir}
                    onChange={(ev) => setParentDir(ev.target.value)}
                    placeholder={t("welcomeNewLocationPlaceholder")}
                  />
                  <IconButton
                    icon={FolderOpen}
                    labelKey="welcomeNewBrowse"
                    onClick={() => void browseParent()}
                  />
                </div>
                {projectPath ? (
                  <p className="hint">{tf("welcomeNewPathPreview", { path: projectPath })}</p>
                ) : null}
                {intent !== "saveAs" ? (
                  <div className="welcome-new-templates">
                    <span>{t("welcomeNewTemplate")}</span>
                    <div className="welcome-template-list" role="listbox" aria-label={t("welcomeNewTemplate")}>
                      <button
                        type="button"
                        role="option"
                        className={`welcome-template-row${templateId === "" ? " active" : ""}`}
                        aria-selected={templateId === ""}
                        onClick={() => setTemplateId("")}
                      >
                        {t("welcomeNewTemplateBlank")}
                      </button>
                      {exampleGroups.map((group) => (
                        <div key={group.domain} className="welcome-template-group">
                          <div className="welcome-template-group-label">
                            {t(exampleDomainLabelKey(group.domain))}
                          </div>
                          {group.items.map((ex) => (
                            <button
                              key={ex.id}
                              type="button"
                              role="option"
                              className={`welcome-template-row${templateId === ex.id ? " active" : ""}`}
                              aria-selected={templateId === ex.id}
                              onClick={() => setTemplateId(ex.id)}
                            >
                              <strong>{exampleTitle(ex)}</strong>
                              <span>{ex.modelName}</span>
                            </button>
                          ))}
                        </div>
                      ))}
                    </div>
                  </div>
                ) : null}
                <label className="tool-field">
                  <span>{t("stopTime")}</span>
                  <input
                    type="number"
                    step="0.1"
                    min="0"
                    value={stopTime}
                    onChange={(ev) => setStopTime(ev.target.value)}
                  />
                </label>
                <label className="tool-field">
                  <span>{t("intervals")}</span>
                  <input
                    type="number"
                    min="1"
                    step="1"
                    value={intervals}
                    onChange={(ev) => setIntervals(ev.target.value)}
                  />
                </label>
                <label className="tool-field">
                  <span>{t("solver")}</span>
                  <select value={solver} onChange={(ev) => setSolver(ev.target.value)}>
                    {NEW_SOLVERS.map((s) => (
                      <option key={s.id} value={s.id}>
                        {t(s.labelKey)}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="tool-field">
                  <span>{t("tolerance")}</span>
                  <input
                    type="number"
                    step="any"
                    min="0"
                    value={tolerance}
                    onChange={(ev) => setTolerance(ev.target.value)}
                  />
                </label>
                <div className="welcome-actions">
                  <IconButton
                    icon={intent === "saveAs" ? Save : FilePlus2}
                    labelKey={intent === "saveAs" ? "welcomeSave" : "welcomeCreate"}
                    primary
                    disabled={!parentDir.trim()}
                    onClick={createProject}
                  />
                </div>
              </div>
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
}
