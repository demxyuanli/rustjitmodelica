import type { DiagramNode } from "../api/types";

/** Pre-run dashboard knobs (not in-sim online tuning — engine has no mid-run param API). */
export function DashboardPanel({
  nodes,
  onChangeParam,
}: {
  nodes: DiagramNode[];
  onChangeParam: (nodeId: string, paramName: string, value: string) => void;
}) {
  const knobs = nodes.flatMap((n) =>
    n.params
      .filter((p) => /^-?\d+(\.\d+)?([eE][-+]?\d+)?$/.test(p.value.trim()))
      .map((p) => ({ node: n, param: p })),
  );

  return (
    <div className="panel right" style={{ borderLeft: "none", borderTop: "1px solid var(--border)" }}>
      <h3>Dashboard</h3>
      <div style={{ padding: "4px 10px", fontSize: 11, color: "var(--muted)" }}>
        Pre-run knobs only (no live Pause/online tune)
      </div>
      {!knobs.length ? (
        <div style={{ padding: 10, fontSize: 12, color: "var(--muted)" }}>No numeric params</div>
      ) : (
        knobs.map(({ node, param }) => {
          const num = Number(param.value);
          const lo = Number.isFinite(num) ? Math.min(0, num * 0.2) : 0;
          const hi = Number.isFinite(num) ? Math.max(num * 2, 1) : 10;
          return (
            <div key={`${node.id}_${param.name}`} style={{ padding: "6px 10px" }}>
              <div style={{ fontSize: 12 }}>
                {node.instanceName}.{param.name} = {param.value}
              </div>
              <input
                type="range"
                min={lo}
                max={hi || 1}
                step={(hi - lo) / 100 || 0.01}
                value={Number.isFinite(num) ? num : 0}
                style={{ width: "100%" }}
                onChange={(ev) => onChangeParam(node.id, param.name, ev.target.value)}
              />
            </div>
          );
        })
      )}
    </div>
  );
}
