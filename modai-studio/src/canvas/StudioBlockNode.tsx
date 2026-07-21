import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import type { DiagramNode } from "../api/types";

export type StudioFlowNode = Node<{ diagram: DiagramNode }, "studio">;

export function StudioBlockNode({ data, selected }: NodeProps<StudioFlowNode>) {
  const n = data.diagram;
  const signalIns = n.ports.filter((p) => p.kind === "signal" && p.direction === "in");
  const signalOuts = n.ports.filter((p) => p.kind === "signal" && p.direction === "out");
  const cons = n.ports.filter((p) => p.kind === "connector");

  const left: { id: string; type: "source" | "target" }[] = [
    ...signalIns.map((p) => ({ id: p.id, type: "target" as const })),
    ...(cons.length <= 1
      ? cons.map((p) => ({ id: p.id, type: "target" as const }))
      : cons.slice(0, -1).map((p) => ({ id: p.id, type: "target" as const }))),
  ];
  const right: { id: string; type: "source" | "target" }[] = [
    ...signalOuts.map((p) => ({ id: p.id, type: "source" as const })),
    ...(cons.length > 1
      ? cons.slice(-1).map((p) => ({ id: p.id, type: "source" as const }))
      : []),
  ];

  const isSub = n.nodeKind === "subsystem";
  const isFmu = n.nodeKind === "fmu";
  const bg = isSub
    ? "#2a3a2a"
    : isFmu
      ? "#2a2a3a"
      : n.domain === "signal"
        ? "#243041"
        : "#3a2f24";

  return (
    <div
      style={{
        minWidth: 110,
        padding: "8px 10px",
        borderRadius: 6,
        border: `1px solid ${
          selected
            ? "var(--accent)"
            : isSub
              ? "#5a9a5a"
              : isFmu
                ? "#7a7ad0"
                : "var(--border)"
        }`,
        background: bg,
        fontSize: 12,
      }}
    >
      <div style={{ fontWeight: 600 }}>
        {isSub ? `[Sub] ${n.instanceName}` : isFmu ? `[FMU] ${n.instanceName}` : n.instanceName}
      </div>
      <div style={{ color: "var(--muted)", fontSize: 10 }}>
        {isSub
          ? "double-click to drill"
          : isFmu
            ? n.ui?.fmuModelIdentifier ?? "FMI2 CS"
            : n.typeName.split(".").slice(-2).join(".")}
      </div>
      {left.map((p, i) => (
        <Handle
          key={`L-${p.id}`}
          id={p.id}
          type={p.type}
          position={Position.Left}
          style={{ top: 28 + i * 14 }}
        />
      ))}
      {right.map((p, i) => (
        <Handle
          key={`R-${p.id}`}
          id={p.id}
          type={p.type}
          position={Position.Right}
          style={{ top: 28 + i * 14 }}
        />
      ))}
    </div>
  );
}
