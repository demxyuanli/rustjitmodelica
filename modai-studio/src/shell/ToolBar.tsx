import {
  AlignHorizontalSpaceAround,
  AlignLeft,
  AlignStartVertical,
  AlignVerticalSpaceAround,
  Gauge,
  Grid3x3,
  Pause,
  Play,
  RotateCw,
  Route,
  Save,
  ShieldCheck,
  SlidersHorizontal,
  SkipForward,
  Square,
  Zap,
  Cpu,
} from "lucide-react";
import type { StudioProjectManifest } from "../api/types";
import type { AlignMode } from "../canvas/editOps";
import type { StudioSettings } from "../settings/studioSettings";
import { IconButton } from "./IconButton";
import { t } from "../i18n";

export interface ToolBarActions {
  onSave: () => void;
  onUpdate: () => void;
  onRun: () => void;
  onStop: () => void;
  onPause: () => void;
  onResume: () => void;
  onStep: () => void;
  paused: boolean;
  onAlign: (mode: AlignMode) => void;
  onReroute: () => void;
  onRotate: () => void;
  snapEnabled: boolean;
  onSnap: (v: boolean) => void;
  showDashboard: boolean;
  onDashboard: (v: boolean) => void;
  fastRestart: boolean;
  onFastRestart: (v: boolean) => void;
  fullJitValidate: boolean;
  onFullJitValidate: (v: boolean) => void;
  manifest: StudioProjectManifest;
  onManifest: (m: StudioProjectManifest) => void;
  onConfig: () => void;
  busy: boolean;
  bgValidate?: boolean;
  runId: string | null;
  canAlign: boolean;
  canRotate: boolean;
  status: string;
}

export function ToolBar({
  settings,
  actions,
}: {
  settings: StudioSettings;
  actions: ToolBarActions;
}) {
  const show = (id: keyof StudioSettings["toolVisible"]) => settings.toolVisible[id] !== false;
  const tip = settings.showTips;
  const labels = false;
  const m = actions.manifest;

  return (
    <div className="tool-bar">
      {show("save") ? (
        <IconButton icon={Save} labelKey="save" tipKey="tipSave" onClick={actions.onSave} showLabel={labels} showTip={tip} />
      ) : null}
      {show("update") ? (
        <IconButton
          icon={ShieldCheck}
          labelKey="update"
          tipKey="tipUpdate"
          onClick={actions.onUpdate}
          disabled={actions.busy}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}
      {show("run") ? (
        <IconButton
          icon={Play}
          labelKey="run"
          tipKey="tipRun"
          onClick={actions.onRun}
          disabled={actions.busy}
          primary
          showLabel={labels}
          showTip={tip}
        />
      ) : null}
      {show("stop") ? (
        <IconButton
          icon={Square}
          labelKey="stop"
          tipKey="tipStop"
          onClick={actions.onStop}
          disabled={!actions.runId && !actions.bgValidate}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}
      {show("pause") ? (
        <IconButton
          icon={Pause}
          labelKey="pause"
          tipKey="tipPause"
          onClick={actions.onPause}
          disabled={!actions.busy || !actions.runId || actions.paused}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}
      {show("resume") ? (
        <IconButton
          icon={Play}
          labelKey="resume"
          tipKey="tipResume"
          onClick={actions.onResume}
          disabled={!actions.busy || !actions.runId || !actions.paused}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}
      {show("step") ? (
        <IconButton
          icon={SkipForward}
          labelKey="step"
          tipKey="tipStep"
          onClick={actions.onStep}
          disabled={!actions.busy || !actions.runId}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}

      <span className="tool-sep" />

      {show("snap") ? (
        <IconButton
          icon={Grid3x3}
          labelKey="snap"
          tipKey="tipSnap"
          onClick={() => actions.onSnap(!actions.snapEnabled)}
          active={actions.snapEnabled}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}
      {show("alignL") ? (
        <IconButton
          icon={AlignLeft}
          labelKey="alignL"
          tipKey="tipAlignL"
          onClick={() => actions.onAlign("left")}
          disabled={!actions.canAlign}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}
      {show("alignT") ? (
        <IconButton
          icon={AlignStartVertical}
          labelKey="alignT"
          tipKey="tipAlignT"
          onClick={() => actions.onAlign("top")}
          disabled={!actions.canAlign}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}
      {show("alignH") ? (
        <IconButton
          icon={AlignHorizontalSpaceAround}
          labelKey="alignH"
          tipKey="tipAlignH"
          onClick={() => actions.onAlign("hcenter")}
          disabled={!actions.canAlign}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}
      {show("alignV") ? (
        <IconButton
          icon={AlignVerticalSpaceAround}
          labelKey="alignV"
          tipKey="tipAlignV"
          onClick={() => actions.onAlign("vcenter")}
          disabled={!actions.canAlign}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}
      {show("reroute") ? (
        <IconButton
          icon={Route}
          labelKey="reroute"
          tipKey="tipReroute"
          onClick={actions.onReroute}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}
      <IconButton
        icon={RotateCw}
        labelKey="rotate"
        tipKey="tipRotate"
        onClick={actions.onRotate}
        disabled={!actions.canRotate}
        showLabel={labels}
        showTip={tip}
      />

      <span className="tool-sep" />

      {show("solver") ? (
        <label className="tool-field" title={tip ? t("solver") : undefined}>
          <span>{t("solver")}</span>
          <select
            value={m.simulationDefaults.solver}
            onChange={(ev) =>
              actions.onManifest({
                ...m,
                simulationDefaults: { ...m.simulationDefaults, solver: ev.target.value },
              })
            }
          >
            <option value="rk45">rk45</option>
            <option value="rk4">rk4</option>
            <option value="implicit">implicit</option>
          </select>
        </label>
      ) : null}
      {show("stopTime") ? (
        <label className="tool-field" title={tip ? t("stopTime") : undefined}>
          <span>{t("stopTime")}</span>
          <input
            type="number"
            step="0.1"
            value={m.simulationDefaults.stopTime}
            onChange={(ev) =>
              actions.onManifest({
                ...m,
                simulationDefaults: {
                  ...m.simulationDefaults,
                  stopTime: Number(ev.target.value),
                },
              })
            }
          />
        </label>
      ) : null}
      {show("intervals") ? (
        <label className="tool-field" title={tip ? t("intervals") : undefined}>
          <span>{t("intervals")}</span>
          <input
            type="number"
            value={m.simulationDefaults.numberOfIntervals}
            onChange={(ev) =>
              actions.onManifest({
                ...m,
                simulationDefaults: {
                  ...m.simulationDefaults,
                  numberOfIntervals: Number(ev.target.value),
                },
              })
            }
          />
          </label>
        ) : null}
      {show("config") ? (
        <IconButton
          icon={SlidersHorizontal}
          labelKey="config"
          tipKey="tipConfig"
          onClick={actions.onConfig}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}

      <span className="tool-sep" />

      {show("dashboard") ? (
        <IconButton
          icon={Gauge}
          labelKey="dashboard"
          tipKey="tipDashboard"
          onClick={() => actions.onDashboard(!actions.showDashboard)}
          active={actions.showDashboard}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}
      {show("fastRestart") ? (
        <IconButton
          icon={Zap}
          labelKey="fastRestart"
          tipKey="tipFastRestart"
          onClick={() => actions.onFastRestart(!actions.fastRestart)}
          active={actions.fastRestart}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}
      {show("fullJitValidate") ? (
        <IconButton
          icon={Cpu}
          labelKey="fullJitValidate"
          tipKey="tipFullJitValidate"
          onClick={() => actions.onFullJitValidate(!actions.fullJitValidate)}
          active={actions.fullJitValidate}
          showLabel={labels}
          showTip={tip}
        />
      ) : null}

      <span className="status-pill">{actions.status}</span>
    </div>
  );
}
