import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import type { DiagramNode } from "../api/types";
import { BlockIcon, blockIconKey } from "./blockIcons";
import { asIconAnnotation, IconFace } from "./IconFace";
import {
  isInportNode,
  isOutportNode,
  isTerminalNode,
  terminalPortNumber,
} from "./terminalNodes";
import {
  busBarHeight,
  busBarKind,
  busDimensionBadge,
  isBusBarNode,
} from "./busBarNodes";
import { isFromNode, isGotoNode, isJumpNode, jumpTag } from "./jumpNodes";
import { samplePeriodLabel, sampleTimeClass } from "./sampleTime";
import { activeVariantLabel, isVariantSubsystem } from "./variantOps";
import {
  layoutPorts,
  normalizeRotation,
  portsByRotatedSide,
  type PortSide,
  type SidePort,
} from "./portGeometry";

export type StudioFlowNode = Node<
  { diagram: DiagramNode; displayValue?: string; magnetPortIds?: string[] },
  "studio" | "studioDraw"
>;

const HANDLE_POS: Record<PortSide, Position> = {
  left: Position.Left,
  right: Position.Right,
  top: Position.Top,
  bottom: Position.Bottom,
};

function PortHandle({
  port,
  position,
  className,
  magnet,
}: {
  port: SidePort;
  position: Position;
  className?: string;
  magnet?: boolean;
}) {
  const types: Array<"source" | "target"> =
    port.kind === "connector" ? ["target", "source"] : [port.type];
  const base = className ?? `slx-handle ${port.kind}`;
  const cls = magnet ? `${base} is-magnet` : base;
  const handles = types.map((type) => (
    <Handle
      key={`${port.id}-${type}`}
      id={port.id}
      type={type}
      position={position}
      className={cls}
      title={port.tip}
    />
  ));
  if (port.kind === "connector") {
    return <span className="slx-handle-stack">{handles}</span>;
  }
  return <>{handles}</>;
}

function PortBands({
  node,
  magnet,
  handleClass,
  showLabels = true,
}: {
  node: DiagramNode;
  magnet: Set<string>;
  handleClass?: (p: SidePort) => string | undefined;
  showLabels?: boolean;
}) {
  const sides = portsByRotatedSide(node);
  return (
    <>
      {(["left", "right", "top", "bottom"] as const).flatMap((side) => {
        const ports = sides[side];
        const along = side === "left" || side === "right" ? "top" : "left";
        const labelAfter = side === "left" || side === "top";
        return ports.map((p, i) => {
          const pct = ((i + 1) / (ports.length + 1)) * 100;
          const signalClass =
            p.kind === "signal" ? (p.type === "target" ? " inport" : " outport") : "";
          const label =
            showLabels && p.label ? (
              <span className={`slx-port-label${signalClass}`}>{p.label}</span>
            ) : null;
          const handle = (
            <PortHandle
              port={p}
              position={HANDLE_POS[side]}
              className={handleClass?.(p)}
              magnet={magnet.has(p.id)}
            />
          );
          return (
            <div
              key={`${side}-${p.id}`}
              className={`slx-port-row ${side}`}
              style={{ [along]: `${pct}%` }}
              title={p.tip}
            >
              {labelAfter ? (
                <>
                  {handle}
                  {label}
                </>
              ) : (
                <>
                  {label}
                  {handle}
                </>
              )}
            </div>
          );
        });
      })}
    </>
  );
}

function shortType(typeName: string): string {
  const parts = typeName.split(".");
  return parts.slice(-2).join(".");
}

function domainClass(n: DiagramNode): string {
  if (isInportNode(n)) return "inport";
  if (isOutportNode(n)) return "outport";
  if (isGotoNode(n)) return "goto";
  if (isFromNode(n)) return "from";
  if (isBusBarNode(n)) return busBarKind(n);
  if (n.nodeKind === "subsystem") return "subsystem";
  if (n.nodeKind === "fmu") return "fmu";
  const st = sampleTimeClass(n);
  if (st === "discrete") return "discrete";
  if (n.domain === "signal") return "signal";
  if (n.domain === "multibody") return "multibody";
  if (n.domain === "mechanical") return "mechanical";
  if (n.domain === "electrical") return "electrical";
  return "generic";
}

function blockHeight(leftCount: number, rightCount: number, base = 64): number {
  const rows = Math.max(leftCount, rightCount, 1);
  return Math.max(base, 36 + rows * 18);
}

function gainLabel(n: DiagramNode): string | undefined {
  return n.params.find((p) => p.name === "k")?.value;
}

export function StudioBlockNode({ data, selected }: NodeProps<StudioFlowNode>) {
  const n = data.diagram;
  const magnet = new Set(data.magnetPortIds ?? []);
  const { left, right } = layoutPorts(n);
  const terminal = isTerminalNode(n);
  const busBar = isBusBarNode(n);
  const jump = isJumpNode(n);
  const goto = isGotoNode(n);
  const from = isFromNode(n);
  const inport = isInportNode(n);
  const outport = isOutportNode(n);
  const barKind = busBar ? busBarKind(n) : null;
  const moIcon = terminal || busBar || jump ? null : asIconAnnotation(n.ui?.icon);
  const labelBelow = !!n.ui?.labelBelow;
  const height = blockHeight(
    left.length,
    right.length,
    n.ui?.height ??
      (terminal
        ? 36
        : jump
          ? 40
          : busBar
            ? busBarHeight(Math.max(left.length, right.length))
            : moIcon
              ? 72
              : 64),
  );
  const width =
    n.ui?.width ??
    (terminal
      ? 72
      : jump
        ? 64
        : busBar
          ? barKind === "bus"
            ? 36
            : 30
          : moIcon
            ? 140
            : 120);
  const isSub = n.nodeKind === "subsystem";
  const isFmu = n.nodeKind === "fmu";
  const isDisplay = (n.catalogId ?? "").toLowerCase().includes("sinks.display");
  const isScope = (n.catalogId ?? "").toLowerCase().includes("sinks.scope");
  const displayValue = data.displayValue;
  const rot = normalizeRotation(n.rotation);
  const boxW = rot === 90 || rot === 270 ? height : width;
  const boxH = rot === 90 || rot === 270 ? width : height;
  const title = n.ui?.label ?? n.instanceName;
  const iconKey = blockIconKey(n);
  const iconLbl =
    iconKey === "gain"
      ? gainLabel(n)
      : iconKey === "generic"
        ? shortType(n.typeName)
        : undefined;
  const faceW = Math.min(72, Math.max(40, width - 52));
  const faceH = Math.min(56, Math.max(28, height - 16));
  const portNum = terminalPortNumber(n);
  const tag = jump ? jumpTag(n) : "";
  const tsBadge = samplePeriodLabel(n);

  if (terminal) {
    return (
      <div
        className={`slx-block slx-terminal domain-${domainClass(n)}${selected ? " selected" : ""}${
          inport ? " is-inport" : ""
        }${outport ? " is-outport" : ""}`}
        style={{ width: boxW, height: boxH }}
        title={`${title} (${inport ? "Inport" : "Outport"})`}
      >
        <div className="slx-terminal-tag">
          <span className="slx-terminal-kind">{inport ? "In" : "Out"}</span>
          <span className="slx-terminal-num">{portNum}</span>
        </div>
        <div className="slx-terminal-name" title={title}>
          {title}
        </div>
        <PortBands node={n} magnet={magnet} showLabels={false} />
      </div>
    );
  }

  if (jump) {
    return (
      <div
        className={`slx-block slx-jump domain-${domainClass(n)}${selected ? " selected" : ""}${
          goto ? " is-goto" : ""
        }${from ? " is-from" : ""}${labelBelow ? " label-below" : ""}`}
        style={{ width: boxW, height: boxH }}
        title={`${title} (${goto ? "Goto" : "From"} ${tag})`}
      >
        <div className="slx-jump-tag">
          <span className="slx-jump-kind">{goto ? "Goto" : "From"}</span>
          <span className="slx-jump-name">{tag}</span>
        </div>
        {!labelBelow ? (
          <div className="slx-jump-inst" title={title}>
            {title}
          </div>
        ) : null}
        <PortBands node={n} magnet={magnet} showLabels={false} />
        {labelBelow ? <div className="slx-block-label-below">{title}</div> : null}
      </div>
    );
  }

  if (busBar && barKind) {
    const kindLabel = barKind === "bus" ? "Bus" : barKind === "demux" ? "Demux" : "Mux";
    const dimBadge = busDimensionBadge(left.length, right.length);
    return (
      <div
        className={`slx-block slx-busbar is-${barKind} domain-${domainClass(n)}${
          selected ? " selected" : ""
        }`}
        style={{ width: boxW, height: boxH }}
        title={`${title} (${kindLabel} ${dimBadge})`}
      >
        <div className="slx-busbar-rail" aria-hidden />
        <div className="slx-busbar-caption">{kindLabel}</div>
        <div className="slx-busbar-dim" title={dimBadge}>
          {dimBadge}
        </div>
        <PortBands node={n} magnet={magnet} />
      </div>
    );
  }

  if (isSub) {
    const subH0 = Math.max(
      height,
      blockHeight(left.length, right.length, n.ui?.height ?? 72),
    );
    const subW0 = n.ui?.width ?? Math.max(width, 160);
    const subW = rot === 90 || rot === 270 ? subH0 : subW0;
    const subH = rot === 90 || rot === 270 ? subW0 : subH0;
    const variant = isVariantSubsystem(n);
    const vLabel = activeVariantLabel(n);
    return (
      <div
        className={`slx-block slx-subsystem domain-subsystem${selected ? " selected" : ""}${
          labelBelow ? " label-below" : ""
        }${variant ? " is-variant" : ""}`}
        style={{ width: subW, height: subH }}
        title={
          variant
            ? `${title} (Variant: ${vLabel ?? "?"}) — double-click to open active`
            : `${title} (Subsystem) — double-click to open`
        }
      >
        <div className="slx-subsystem-frame" aria-hidden />
        <div className="slx-subsystem-body">
          <BlockIcon kind={variant ? "variant" : "subsystem"} />
          {!labelBelow ? (
            <div className="slx-block-caption">
              <div className="slx-block-title" title={title}>
                {title}
              </div>
              <div className="slx-block-type">
                {variant
                  ? `V: ${vLabel ?? "?"} · ${left.length}/${right.length}`
                  : `${left.length} in / ${right.length} out`}
              </div>
            </div>
          ) : (
            <div className="slx-block-caption compact">
              <div className="slx-block-type">
                {variant
                  ? `V: ${vLabel ?? "?"}`
                  : `${left.length} in / ${right.length} out`}
              </div>
            </div>
          )}
        </div>
        <PortBands
          node={n}
          magnet={magnet}
          handleClass={(p) =>
            p.kind === "signal"
              ? p.type === "target"
                ? "slx-handle signal inport"
                : "slx-handle signal outport"
              : undefined
          }
        />
        {labelBelow ? <div className="slx-block-label-below">{title}</div> : null}
      </div>
    );
  }

  return (
    <div
      className={`slx-block has-icon domain-${domainClass(n)}${selected ? " selected" : ""}${
        isFmu ? " is-fmu" : ""
      }${iconKey === "gain" ? " shape-gain" : ""}${moIcon ? " has-mo-icon" : ""}${
        labelBelow ? " label-below" : ""
      }${isDisplay ? " is-display" : ""}${isScope ? " is-scope" : ""}${rot ? ` rot-${rot}` : ""}`}
      style={{ width: boxW, height: boxH }}
      title={
        isDisplay
          ? `${title} (Display${displayValue != null ? `: ${displayValue}` : ""})`
          : isScope
            ? `${title} (Scope)`
            : title
      }
    >
      <div className="slx-block-stripe" aria-hidden />
      <div className="slx-block-body">
        <div
          className={`slx-block-icon-wrap${moIcon ? " mo-icon" : ""}`}
          style={rot ? { transform: `rotate(${rot}deg)` } : undefined}
        >
          {moIcon ? (
            <IconFace
              icon={moIcon}
              width={faceW}
              height={faceH}
              instanceName={n.instanceName}
            />
          ) : (
            <BlockIcon kind={iconKey} label={iconLbl} />
          )}
        </div>
        {!labelBelow ? (
          <div className="slx-block-caption">
            <div className="slx-block-title" title={title}>
              {title}
            </div>
            <div className="slx-block-type" title={n.typeName}>
              {isDisplay
                ? displayValue != null
                  ? displayValue
                  : "Display"
                : isScope
                  ? "Scope"
                  : isFmu
                    ? n.ui?.fmuModelIdentifier ?? "FMI2 CS"
                    : shortType(n.typeName)}
            </div>
            {tsBadge ? <div className="slx-sample-badge">{tsBadge}</div> : null}
          </div>
        ) : (
          <div className="slx-block-caption compact">
            <div className="slx-block-type" title={n.typeName}>
              {isDisplay
                ? displayValue != null
                  ? displayValue
                  : "Display"
                : isScope
                  ? "Scope"
                  : isFmu
                    ? n.ui?.fmuModelIdentifier ?? "FMI2 CS"
                    : shortType(n.typeName)}
            </div>
            {tsBadge ? <div className="slx-sample-badge">{tsBadge}</div> : null}
          </div>
        )}
      </div>
      <PortBands node={n} magnet={magnet} />
      {labelBelow ? <div className="slx-block-label-below">{title}</div> : null}
    </div>
  );
}
