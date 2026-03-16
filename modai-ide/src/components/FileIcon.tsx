interface FileIconProps {
  name: string;
  size?: number;
}

interface IconDef {
  label: string;
  fg: string;
  bg: string;
}

const EXT_MAP: Record<string, IconDef> = {
  mo:   { label: "M",  fg: "#fff",    bg: "#2b7bd6" },
  rs:   { label: "R",  fg: "#fff",    bg: "#c45a2c" },
  ts:   { label: "TS", fg: "#fff",    bg: "#3178c6" },
  tsx:  { label: "TS", fg: "#fff",    bg: "#3178c6" },
  js:   { label: "JS", fg: "#1a1a1a", bg: "#f0db4f" },
  jsx:  { label: "JS", fg: "#1a1a1a", bg: "#f0db4f" },
  json: { label: "{}",  fg: "#1a1a1a", bg: "#f0db4f" },
  h:    { label: "H",  fg: "#fff",    bg: "#9b59b6" },
  c:    { label: "C",  fg: "#fff",    bg: "#6c5ce7" },
  cpp:  { label: "C+", fg: "#fff",    bg: "#6c5ce7" },
  md:   { label: "M",  fg: "#fff",    bg: "#5b8fa8" },
  txt:  { label: "T",  fg: "#fff",    bg: "#7f8c8d" },
  toml: { label: "T",  fg: "#d4d4d4", bg: "#555" },
  yaml: { label: "Y",  fg: "#d4d4d4", bg: "#555" },
  yml:  { label: "Y",  fg: "#d4d4d4", bg: "#555" },
  css:  { label: "#",  fg: "#fff",    bg: "#8b5cf6" },
  html: { label: "H",  fg: "#fff",    bg: "#e44d26" },
  svg:  { label: "S",  fg: "#fff",    bg: "#e88e28" },
};

const DEFAULT_ICON: IconDef = { label: "F", fg: "#d4d4d4", bg: "#555" };

function getIconDef(name: string): IconDef {
  const ext = name.includes(".") ? name.split(".").pop()?.toLowerCase() ?? "" : "";
  return EXT_MAP[ext] ?? DEFAULT_ICON;
}

export function FileIcon({ name, size = 16 }: FileIconProps) {
  const def = getIconDef(name);
  const fontSize = def.label.length > 1 ? size * 0.45 : size * 0.6;
  const r = (size - 2) / 2;
  return (
    <svg
      width={size}
      height={size}
      viewBox={`0 0 ${size} ${size}`}
      style={{ flexShrink: 0 }}
      aria-hidden
    >
      <circle cx={size / 2} cy={size / 2} r={r} fill={def.bg} />
      <text
        x={size / 2}
        y={size / 2}
        dominantBaseline="central"
        textAnchor="middle"
        fill={def.fg}
        fontSize={fontSize}
        fontFamily="ui-monospace, Monaco, Consolas, monospace"
        fontWeight={700}
      >
        {def.label}
      </text>
    </svg>
  );
}
