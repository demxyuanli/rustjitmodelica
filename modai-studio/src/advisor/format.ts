import type { DiagnosticSource, StudioDiagnostic } from "../api/types";
import { t, tf, type MessageKey } from "../i18n";

export type { DiagnosticSource };

const ADV_MSG: Record<string, MessageKey> = {
  duplicateName: "advDupName",
  unconnectedTerminal: "advUnconnectedTerminal",
  unconnectedOut: "advUnconnectedOut",
  unconnectedIn: "advUnconnectedIn",
  orphanBlock: "advOrphan",
  missingGround: "advMissingGround",
  gotoEmpty: "advGotoEmpty",
  gotoDup: "advGotoDup",
  fromEmpty: "advFromEmpty",
  fromMiss: "advFromMiss",
  gotoUnused: "advGotoUnused",
  missingSubsystem: "advSubNoRef",
  emptySubsystem: "advSubEmpty",
  variantPorts: "advVariantPorts",
  signalCycle: "advSignalCycle",
  domainMismatch: "advDomainMismatch",
  unitConflict: "advUnitConflict",
  sampleRateMismatch: "advSampleRate",
};

const ADV_HINT: Record<string, MessageKey> = {
  duplicateName: "advHintDupName",
  unconnectedTerminal: "advHintUnconnected",
  unconnectedOut: "advHintUnconnected",
  unconnectedIn: "advHintUnconnected",
  orphanBlock: "advHintOrphan",
  missingGround: "advHintMissingGround",
  gotoEmpty: "advHintGotoTag",
  gotoDup: "advHintGotoDup",
  fromEmpty: "advHintGotoTag",
  fromMiss: "advHintFromMiss",
  gotoUnused: "advHintGotoUnused",
  missingSubsystem: "advHintSubsystem",
  emptySubsystem: "advHintSubsystem",
  variantPorts: "advHintVariantPorts",
  signalCycle: "advHintSignalCycle",
  domainMismatch: "advHintDomain",
  unitConflict: "advHintUnit",
  sampleRateMismatch: "advHintSampleRate",
};

const CONN_MSG: Record<string, MessageKey> = {
  "missing port": "connMissingPort",
  missing_port: "connMissingPort",
  "missing node": "connMissingNode",
  missing_node: "connMissingNode",
  "domain mismatch": "connDomainMismatch",
  domain_mismatch: "connDomainMismatch",
  "signal must be out -> in": "connSignalDirection",
  signal_direction: "connSignalDirection",
  "incompatible port kinds": "connIncompatible",
  incompatible_kinds: "connIncompatible",
};

export function diagnosticSource(d: StudioDiagnostic): DiagnosticSource {
  if (d.source === "advisor" || d.source === "connection" || d.source === "structure" || d.source === "compiler") {
    return d.source;
  }
  const m = d.message;
  if (m.includes("[advisor:")) return "advisor";
  if (m.startsWith("Invalid edge") || m.includes("invalid connection")) return "connection";
  if (m.includes("[structure]")) return "structure";
  return "compiler";
}

export function localizeConnectReason(reason?: string | null): string {
  if (!reason) return t("invalidConnection");
  const key = CONN_MSG[reason];
  return key ? t(key) : reason;
}

export function formatDiagnostic(d: StudioDiagnostic): {
  title: string;
  hint?: string;
  rule?: string;
  source: DiagnosticSource;
} {
  const source = diagnosticSource(d);
  if (source === "advisor" && d.rule) {
    const msgKey = ADV_MSG[d.rule];
    const hintKey = ADV_HINT[d.rule];
    const params = d.params ?? {};
    return {
      source,
      rule: d.rule,
      title: msgKey ? tf(msgKey, params) : d.message,
      hint: hintKey ? t(hintKey) : d.hint ?? undefined,
    };
  }
  if (source === "connection") {
    const reason = d.rule ?? d.message;
    return {
      source,
      rule: d.rule ?? "connection",
      title: localizeConnectReason(reason),
      hint: t("advHintConnection"),
    };
  }
  let title = d.message;
  if (title.startsWith("[structure] ")) title = title.slice("[structure] ".length);
  return {
    source,
    rule: d.rule ?? (source === "structure" ? "structure" : undefined),
    title,
    hint: d.hint ?? undefined,
  };
}
