import { BookOpen } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { ExampleInfo } from "../api/types";
import { listExamples } from "../api/studio";
import { t, type MessageKey } from "../i18n";
import { exampleDomainLabelKey, groupExamples } from "./exampleGroups";
import { IconButton } from "./IconButton";

const TITLE_KEY: Record<string, MessageKey> = {
  signal_feedback: "exampleSignalFeedback",
  first_order_step: "exampleFirstOrderStep",
  pid_plant: "examplePidPlant",
  tf_plant: "exampleTfPlant",
  limiter_integrator: "exampleLimiterIntegrator",
  switch_select: "exampleSwitchSelect",
  goto_from: "exampleGotoFrom",
  rate_transition: "exampleRateTransition",
  rc_circuit: "exampleRcCircuit",
  rl_circuit: "exampleRlCircuit",
  rlc_series: "exampleRlcSeries",
  multibody_preview: "exampleMultibodyPreview",
  elec_sensor_control: "exampleElecSensorControl",
  rot_drive: "exampleRotDrive",
  rot_gear_train: "exampleRotGearTrain",
  rot_sensor_control: "exampleRotSensorControl",
  trans_mass_force: "exampleTransMassForce",
  trans_sensor_control: "exampleTransSensorControl",
  rot_spring_damper: "exampleRotSpringDamper",
  trans_spring_damper: "exampleTransSpringDamper",
};

export function exampleTitle(ex: ExampleInfo): string {
  const key = TITLE_KEY[ex.id];
  return key ? t(key) : ex.title;
}

export function ExamplesMenu({
  onOpen,
  showLabel,
  showTip,
  embedded = false,
}: {
  onOpen: (id: string) => void;
  showLabel: boolean;
  showTip: boolean;
  embedded?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [items, setItems] = useState<ExampleInfo[]>([]);
  const rootRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    void listExamples()
      .then(setItems)
      .catch(() => setItems([]));
  }, []);

  useEffect(() => {
    if (!open || embedded) return;
    const onDoc = (ev: MouseEvent) => {
      if (!rootRef.current?.contains(ev.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open, embedded]);

  const list = (
    <div className={embedded ? "examples-embedded" : "examples-popover"} role="menu">
      {items.length === 0 ? (
        <div className="examples-empty">{t("noInstances")}</div>
      ) : (
        groupExamples(items).map((group) => (
          <div key={group.domain} className="examples-group">
            <div className="examples-group-label">{t(exampleDomainLabelKey(group.domain))}</div>
            {group.items.map((ex) => (
              <button
                key={ex.id}
                type="button"
                className="examples-item"
                role="menuitem"
                onClick={() => {
                  setOpen(false);
                  onOpen(ex.id);
                }}
              >
                <span className="examples-item-title">{exampleTitle(ex)}</span>
                <span className="examples-item-meta">{ex.modelName}</span>
              </button>
            ))}
          </div>
        ))
      )}
    </div>
  );

  if (embedded) return list;

  return (
    <div className="examples-menu" ref={rootRef}>
      <IconButton
        icon={BookOpen}
        labelKey="examples"
        tipKey="tipExamples"
        onClick={() => setOpen((v) => !v)}
        showLabel={showLabel}
        showTip={showTip}
      />
      {open ? list : null}
    </div>
  );
}
