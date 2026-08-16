import type { DiagramDocument, DiagramNode, StudioDiagnostic } from "../api/types";
import { isDrawNode } from "../canvas/drawParams";
import { isRateTransitionNode, samplePeriodOf } from "../canvas/sampleTime";
import { inferPortUnit } from "../canvas/sensorMeta";
import { variantPortMismatch } from "../canvas/variantOps";

export type AdvisorSeverity = "error" | "warning" | "info";

export interface AdvisorFinding {
  id: string;
  rule: string;
  severity: AdvisorSeverity;
  /** Node id for focus, if any. */
  nodeId?: string;
  params?: Record<string, string | number>;
}

function simNodes(diagram: DiagramDocument): DiagramNode[] {
  return diagram.nodes.filter((n) => !isDrawNode(n) && n.nodeKind !== "annotation");
}

function portUsed(
  diagram: DiagramDocument,
  nodeId: string,
  portId: string,
  as: "source" | "target",
): boolean {
  return diagram.edges.some((e) =>
    as === "source"
      ? e.sourceNodeId === nodeId && e.sourcePortId === portId
      : e.targetNodeId === nodeId && e.targetPortId === portId,
  );
}

function isGoto(n: DiagramNode): boolean {
  const id = n.catalogId.toLowerCase();
  return id.includes("routing.goto") || id.endsWith(".goto");
}

function isFrom(n: DiagramNode): boolean {
  const id = n.catalogId.toLowerCase();
  return id.includes("routing.from") || id.endsWith(".from");
}

function paramOf(n: DiagramNode, name: string): string {
  return n.params.find((p) => p.name === name)?.value.trim() ?? "";
}

/** Client-side Model Advisor checklist (F80) + structure hints (F81). */
export function runStaticChecks(
  diagram: DiagramDocument,
  subsystems: Record<string, DiagramDocument> = {},
): AdvisorFinding[] {
  const findings: AdvisorFinding[] = [];
  const nodes = simNodes(diagram);
  const connected = new Set<string>();
  for (const e of diagram.edges) {
    connected.add(e.sourceNodeId);
    connected.add(e.targetNodeId);
  }

  // Duplicate instance names
  const nameCount = new Map<string, string[]>();
  for (const n of nodes) {
    const list = nameCount.get(n.instanceName) ?? [];
    list.push(n.id);
    nameCount.set(n.instanceName, list);
  }
  for (const [name, ids] of nameCount) {
    if (ids.length > 1) {
      findings.push({
        id: `dup_${name}`,
        rule: "duplicateName",
        severity: "error",
        params: { name, n: ids.length },
        nodeId: ids[0],
      });
    }
  }

  // Unconnected ports (signal/electrical) — skip subsystem face if no ports expected
  for (const n of nodes) {
    for (const p of n.ports) {
      if (!p.id || p.id === "self") {
        // connector self: require at least one edge
        const used =
          diagram.edges.some((e) => e.sourceNodeId === n.id) ||
          diagram.edges.some((e) => e.targetNodeId === n.id);
        if (!used && (p.direction === "in" || p.direction === "out" || n.nodeKind === "component")) {
          // RealInput/RealOutput with self port
          if (n.catalogId.toLowerCase().includes("terminals") || p.typeName.includes("Input") || p.typeName.includes("Output")) {
            if (!used) {
              findings.push({
                id: `port_${n.id}_self`,
                rule: "unconnectedTerminal",
                severity: "warning",
                params: { name: n.instanceName },
                nodeId: n.id,
              });
            }
          }
        }
        continue;
      }
      const needSource = p.direction === "out" || p.direction === "inout";
      const needTarget = p.direction === "in" || p.direction === "inout";
      if (needSource && !portUsed(diagram, n.id, p.id, "source")) {
        // out ports may fan-out unused — only warn if never used as source
          findings.push({
            id: `port_${n.id}_${p.id}_out`,
            rule: "unconnectedOut",
            severity: "warning",
            params: { name: n.instanceName, port: p.id },
            nodeId: n.id,
          });
      }
      if (needTarget && !portUsed(diagram, n.id, p.id, "target")) {
          findings.push({
            id: `port_${n.id}_${p.id}_in`,
            rule: "unconnectedIn",
            severity: "warning",
            params: { name: n.instanceName, port: p.id },
            nodeId: n.id,
          });
      }
    }
  }

  // Orphan blocks (no edges)
  for (const n of nodes) {
    if (n.nodeKind === "subsystem") continue;
    if (!connected.has(n.id) && n.ports.length > 0) {
      findings.push({
        id: `orphan_${n.id}`,
        rule: "orphanBlock",
        severity: "warning",
        params: { name: n.instanceName },
        nodeId: n.id,
      });
    }
  }

  // Electrical domain: ground recommended
  const electrical = nodes.filter((n) => n.domain === "electrical");
  if (electrical.length > 0) {
    const hasGround = electrical.some(
      (n) =>
        n.catalogId.toLowerCase().includes("ground") ||
        n.typeName.toLowerCase().includes("ground"),
    );
    if (!hasGround) {
      findings.push({
        id: "elec_ground",
        rule: "missingGround",
        severity: "warning",
        nodeId: electrical[0]?.id,
      });
    }
  }

  // Goto / From tag matching
  const gotos = nodes.filter(isGoto);
  const froms = nodes.filter(isFrom);
  const gotoTags = new Map<string, string>();
  for (const g of gotos) {
    const tag = paramOf(g, "tag");
    if (!tag) {
      findings.push({
        id: `goto_empty_${g.id}`,
        rule: "gotoEmpty",
        severity: "warning",
        params: { name: g.instanceName },
        nodeId: g.id,
      });
      continue;
    }
    if (gotoTags.has(tag)) {
      findings.push({
        id: `goto_dup_${tag}`,
        rule: "gotoDup",
        severity: "error",
        params: { tag },
        nodeId: g.id,
      });
    }
    gotoTags.set(tag, g.id);
  }
  for (const f of froms) {
    const tag = paramOf(f, "tag");
    if (!tag) {
      findings.push({
        id: `from_empty_${f.id}`,
        rule: "fromEmpty",
        severity: "warning",
        params: { name: f.instanceName },
        nodeId: f.id,
      });
      continue;
    }
    if (!gotoTags.has(tag)) {
      findings.push({
        id: `from_miss_${f.id}`,
        rule: "fromMiss",
        severity: "error",
        params: { name: f.instanceName, tag },
        nodeId: f.id,
      });
    }
  }
  for (const [tag, gid] of gotoTags) {
    if (!froms.some((f) => paramOf(f, "tag") === tag)) {
      findings.push({
        id: `goto_unused_${tag}`,
        rule: "gotoUnused",
        severity: "info",
        params: { tag },
        nodeId: gid,
      });
    }
  }

  // Empty subsystems + variant port mismatch
  for (const n of nodes.filter((x) => x.nodeKind === "subsystem")) {
    const ref = n.subsystemRef;
    if (!ref) {
      findings.push({
        id: `sub_noref_${n.id}`,
        rule: "missingSubsystem",
        severity: "warning",
        params: { name: n.instanceName },
        nodeId: n.id,
      });
      continue;
    }
    const child = subsystems[ref];
    if (!child || simNodes(child).length === 0) {
      findings.push({
        id: `sub_empty_${n.id}`,
        rule: "emptySubsystem",
        severity: "warning",
        params: { name: n.instanceName },
        nodeId: n.id,
      });
    }
    const mismatch = variantPortMismatch(n, subsystems);
    if (mismatch) {
      findings.push({
        id: `variant_ports_${n.id}`,
        rule: "variantPorts",
        severity: "warning",
        params: { name: n.instanceName, a: mismatch.a, b: mismatch.b },
        nodeId: n.id,
      });
    }
  }

  // Signal feedback loops (structure hint for algebraic / causal cycles)
  const cycle = findSignalCycle(diagram, nodes);
  if (cycle) {
    findings.push({
      id: "signal_cycle",
        rule: "signalCycle",
        severity: "info",
        params: { cycle },
        nodeId: nodes.find((n) => cycle.includes(n.instanceName))?.id,
    });
  }

  // F72 / Q4-B: domain mismatch on edges + unit conflicts
  const byId = new Map(nodes.map((n) => [n.id, n]));
  for (const e of diagram.edges) {
    const sn = byId.get(e.sourceNodeId);
    const tn = byId.get(e.targetNodeId);
    if (!sn || !tn) continue;
    const sd = sn.domain;
    const td = tn.domain;
    const physical = new Set(["electrical", "mechanical", "multibody"]);
    if (
      physical.has(sd) &&
      physical.has(td) &&
      sd !== td &&
      sn.nodeKind === "component" &&
      tn.nodeKind === "component"
    ) {
      findings.push({
        id: `domain_${e.id}`,
        rule: "domainMismatch",
        severity: "warning",
        params: {
          src: `${sn.instanceName}.${e.sourcePortId}`,
          dst: `${tn.instanceName}.${e.targetPortId}`,
          a: sd,
          b: td,
        },
        nodeId: sn.id,
      });
    }
    const inferred =
      inferPortUnit(sn, e.sourcePortId) ?? inferPortUnit(tn, e.targetPortId);
    const annotated = e.unit?.trim() || null;
    if (inferred && annotated && normalizeUnit(inferred) !== normalizeUnit(annotated)) {
      findings.push({
        id: `unit_${e.id}`,
        rule: "unitConflict",
        severity: "info",
        params: {
          annotated,
          inferred,
          port: `${sn.instanceName}.${e.sourcePortId}`,
        },
        nodeId: sn.id,
      });
    }

    // Multi-rate: discrete Ts mismatch without Rate Transition bridge
    const tsA = samplePeriodOf(sn);
    const tsB = samplePeriodOf(tn);
    if (
      tsA != null &&
      tsB != null &&
      Math.abs(tsA - tsB) > 1e-12 &&
      !isRateTransitionNode(sn) &&
      !isRateTransitionNode(tn)
    ) {
      findings.push({
        id: `ts_${e.id}`,
        rule: "sampleRateMismatch",
        severity: "warning",
        params: { a: tsA, b: tsB, src: sn.instanceName, dst: tn.instanceName },
        nodeId: sn.id,
      });
    }
  }

  return findings;
}

function normalizeUnit(u: string): string {
  return u.replace(/\s+/g, "").toLowerCase();
}

function findSignalCycle(diagram: DiagramDocument, nodes: DiagramNode[]): string | null {
  const idToName = new Map(nodes.map((n) => [n.id, n.instanceName]));
  const adj = new Map<string, string[]>();
  for (const n of nodes) adj.set(n.id, []);
  for (const e of diagram.edges) {
    if (!adj.has(e.sourceNodeId) || !adj.has(e.targetNodeId)) continue;
    adj.get(e.sourceNodeId)!.push(e.targetNodeId);
  }
  const visited = new Set<string>();
  const stack = new Set<string>();
  const path: string[] = [];

  const dfs = (id: string): string | null => {
    visited.add(id);
    stack.add(id);
    path.push(idToName.get(id) ?? id);
    for (const nxt of adj.get(id) ?? []) {
      if (!visited.has(nxt)) {
        const hit = dfs(nxt);
        if (hit) return hit;
      } else if (stack.has(nxt)) {
        const name = idToName.get(nxt) ?? nxt;
        const i = path.indexOf(name);
        return (i >= 0 ? path.slice(i) : path).concat(name).join(" -> ");
      }
    }
    path.pop();
    stack.delete(id);
    return null;
  };

  for (const n of nodes) {
    if (!visited.has(n.id)) {
      const hit = dfs(n.id);
      if (hit) return hit;
    }
  }
  return null;
}

/** Classify compiler messages that look like algebraic / structural issues (F81). */
export function classifyStructureDiagnostics(
  diags: StudioDiagnostic[],
): StudioDiagnostic[] {
  return diags.map((d) => {
    const m = d.message.toLowerCase();
    if (
      m.includes("algebraic") ||
      m.includes("singular") ||
      m.includes("structurally singular") ||
      m.includes("underdetermined") ||
      m.includes("overdetermined") ||
      m.includes("matching failed") ||
      m.includes("no matching")
    ) {
      return {
        ...d,
        source: d.source ?? "structure",
        rule: d.rule ?? "structure",
        message: d.message.startsWith("[structure]") ? d.message : `[structure] ${d.message}`,
      };
    }
    return d;
  });
}

export function findingsToDiagnostics(findings: AdvisorFinding[]): {
  errors: StudioDiagnostic[];
  warnings: StudioDiagnostic[];
} {
  const errors: StudioDiagnostic[] = [];
  const warnings: StudioDiagnostic[] = [];
  for (const f of findings) {
    const diag: StudioDiagnostic = {
      message: `[advisor:${f.rule}]`,
      source: "advisor",
      rule: f.rule,
      nodeId: f.nodeId,
      severity: f.severity,
      params: f.params,
    };
    if (f.severity === "error") errors.push(diag);
    else warnings.push(diag);
  }
  return { errors, warnings };
}
