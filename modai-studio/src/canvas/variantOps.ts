import type { DiagramDocument, DiagramNode } from "../api/types";
import { buildPortsFromChildTerminals } from "./subsystemOps";

export type VariantChoice = {
  id: string;
  label: string;
  subsystemRef: string;
};

export function variantList(n: DiagramNode): VariantChoice[] {
  return (n.ui?.variants as VariantChoice[] | undefined) ?? [];
}

export function isVariantSubsystem(n: DiagramNode): boolean {
  return n.nodeKind === "subsystem" && variantList(n).length >= 1;
}

export function activeVariantId(n: DiagramNode): string | null {
  const list = variantList(n);
  if (!list.length) return null;
  const aid = n.ui?.activeVariantId;
  if (aid && list.some((v) => v.id === aid)) return aid;
  const byRef = list.find((v) => v.subsystemRef === n.subsystemRef);
  return byRef?.id ?? list[0]!.id;
}

export function activeVariantLabel(n: DiagramNode): string | null {
  const id = activeVariantId(n);
  if (!id) return null;
  return variantList(n).find((v) => v.id === id)?.label ?? null;
}

/** Ensure a plain subsystem has a single variant entry (idempotent). */
export function ensureVariantMeta(n: DiagramNode): DiagramNode {
  if (n.nodeKind !== "subsystem" || !n.subsystemRef) return n;
  const list = variantList(n);
  if (list.length) return n;
  const id = "v1";
  return {
    ...n,
    typeName: "VariantSubsystem",
    catalogId: "studio.variantSubsystem",
    ui: {
      ...n.ui,
      variants: [{ id, label: "Choice1", subsystemRef: n.subsystemRef }],
      activeVariantId: id,
    },
  };
}

function portSignature(child: DiagramDocument | undefined): string {
  if (!child) return "";
  const built = buildPortsFromChildTerminals(child);
  return built.ports.map((p) => `${p.direction}:${p.id}`).sort().join("|");
}

export function variantPortMismatch(
  n: DiagramNode,
  subsystems: Record<string, DiagramDocument>,
): { a: string; b: string } | null {
  const list = variantList(n);
  if (list.length < 2) return null;
  const sigs = list.map((v) => ({
    label: v.label,
    sig: portSignature(subsystems[v.subsystemRef]),
  }));
  const base = sigs[0]!.sig;
  const bad = sigs.find((s) => s.sig !== base);
  if (!bad) return null;
  return { a: sigs[0]!.label, b: bad.label };
}

export type VariantOpResult = {
  parent: DiagramDocument;
  subsystems: Record<string, DiagramDocument>;
  nodeId: string;
};

/** Clone active child into a new variant choice; keep current active. */
export function addVariantChoice(
  parent: DiagramDocument,
  subsystems: Record<string, DiagramDocument>,
  nodeId: string,
): VariantOpResult | null {
  const raw = parent.nodes.find((n) => n.id === nodeId);
  if (!raw || raw.nodeKind !== "subsystem" || !raw.subsystemRef) return null;
  const node = ensureVariantMeta(raw);
  const list = [...variantList(node)];
  const activeId = activeVariantId(node)!;
  const active = list.find((v) => v.id === activeId)!;
  const src = subsystems[active.subsystemRef];
  if (!src) return null;

  const stamp = Date.now();
  const newRef = `sub_${stamp}_v${list.length + 1}`;
  const label = `Choice${list.length + 1}`;
  const newId = `v${list.length + 1}_${stamp}`;
  const child: DiagramDocument = {
    ...structuredClone(src),
    diagramId: newRef,
    modelName: `${src.modelName}_${label}`,
    parentDiagramId: parent.diagramId,
  };

  const nextNode: DiagramNode = {
    ...node,
    typeName: "VariantSubsystem",
    catalogId: "studio.variantSubsystem",
    ui: {
      ...node.ui,
      variants: [...list, { id: newId, label, subsystemRef: newRef }],
      activeVariantId: activeId,
    },
  };

  return {
    parent: {
      ...parent,
      nodes: parent.nodes.map((n) => (n.id === nodeId ? nextNode : n)),
    },
    subsystems: { ...subsystems, [newRef]: child },
    nodeId,
  };
}

/** Switch active variant and sync face ports from that child. */
export function setActiveVariant(
  parent: DiagramDocument,
  subsystems: Record<string, DiagramDocument>,
  nodeId: string,
  variantId: string,
): VariantOpResult | null {
  const raw = parent.nodes.find((n) => n.id === nodeId);
  if (!raw || raw.nodeKind !== "subsystem") return null;
  const node = ensureVariantMeta(raw);
  const list = variantList(node);
  const choice = list.find((v) => v.id === variantId);
  if (!choice) return null;
  const child = subsystems[choice.subsystemRef];
  if (!child) return null;
  const built = buildPortsFromChildTerminals(child);
  const nextNode: DiagramNode = {
    ...node,
    subsystemRef: choice.subsystemRef,
    typeName: "VariantSubsystem",
    catalogId: "studio.variantSubsystem",
    ports: built.ports,
    ui: {
      ...node.ui,
      variants: list,
      activeVariantId: variantId,
      portBindings: built.portBindings,
      width: built.width,
      height: built.height,
    },
  };
  return {
    parent: {
      ...parent,
      nodes: parent.nodes.map((n) => (n.id === nodeId ? nextNode : n)),
    },
    subsystems,
    nodeId,
  };
}

export function renameVariantChoice(
  parent: DiagramDocument,
  nodeId: string,
  variantId: string,
  label: string,
): DiagramDocument | null {
  const raw = parent.nodes.find((n) => n.id === nodeId);
  if (!raw) return null;
  const node = ensureVariantMeta(raw);
  const list = variantList(node).map((v) =>
    v.id === variantId ? { ...v, label: label.trim() || v.label } : v,
  );
  const nextNode: DiagramNode = {
    ...node,
    ui: { ...node.ui, variants: list },
  };
  return {
    ...parent,
    nodes: parent.nodes.map((n) => (n.id === nodeId ? nextNode : n)),
  };
}

/** Remove a non-last variant; drops unused child diagram. */
export function removeVariantChoice(
  parent: DiagramDocument,
  subsystems: Record<string, DiagramDocument>,
  nodeId: string,
  variantId: string,
): VariantOpResult | null {
  const raw = parent.nodes.find((n) => n.id === nodeId);
  if (!raw) return null;
  const node = ensureVariantMeta(raw);
  const list = variantList(node);
  if (list.length <= 1) return null;
  const doomed = list.find((v) => v.id === variantId);
  if (!doomed) return null;
  const nextList = list.filter((v) => v.id !== variantId);
  let activeId = activeVariantId(node)!;
  let subsystemRef = node.subsystemRef;
  let ports = node.ports;
  let ui = { ...node.ui, variants: nextList, activeVariantId: activeId };

  if (activeId === variantId) {
    const fallback = nextList[0]!;
    activeId = fallback.id;
    subsystemRef = fallback.subsystemRef;
    const child = subsystems[fallback.subsystemRef];
    if (child) {
      const built = buildPortsFromChildTerminals(child);
      ports = built.ports;
      ui = {
        ...ui,
        activeVariantId: activeId,
        portBindings: built.portBindings,
        width: built.width,
        height: built.height,
      };
    } else {
      ui = { ...ui, activeVariantId: activeId };
    }
  }

  const nextNode: DiagramNode = {
    ...node,
    subsystemRef,
    ports,
    ui,
  };
  const nextSubs = { ...subsystems };
  delete nextSubs[doomed.subsystemRef];

  return {
    parent: {
      ...parent,
      nodes: parent.nodes.map((n) => (n.id === nodeId ? nextNode : n)),
    },
    subsystems: nextSubs,
    nodeId,
  };
}
