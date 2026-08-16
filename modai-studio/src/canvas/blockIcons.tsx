import type { DiagramNode } from "../api/types";

/** Resolve a compact Simulink-like glyph key from catalog / type. */
export function blockIconKey(n: DiagramNode): string {
  const id = n.catalogId.toLowerCase();
  const ty = n.typeName.toLowerCase();
  if (n.nodeKind === "subsystem") return "subsystem";
  if (n.nodeKind === "fmu") return "fmu";
  if (id.includes("modelref") || id.startsWith("userlib.") || (n.ui?.moAsset && n.ui.moAsset.length > 0))
    return "modelRef";
  if (id.includes("inport") || id.includes("terminals.in")) return "inport";
  if (id.includes("outport") || id.includes("terminals.out")) return "outport";
  if (id.includes("routing.goto") || id.endsWith(".goto")) return "goto";
  if (id.includes("routing.from") || id.endsWith(".from")) return "from";
  if (id.includes("routing.bus") || (id.includes("bus") && id.includes("routing"))) return "bus";
  if (id.includes("demux")) return "demux";
  if (id.includes("mux")) return "mux";
  if (id.includes("product") || ty.endsWith(".product")) return "product";
  if (id.includes("division") || ty.endsWith(".division")) return "division";
  if (id.includes("abs") && id.includes("math")) return "abs";
  if (id.includes("sqrt")) return "sqrt";
  if (id.includes("sign") && id.includes("math")) return "sign";
  if (id.includes("math.sin") || ty.endsWith(".sin")) return "sin";
  if (id.includes("math.cos") || ty.endsWith(".cos")) return "cos";
  if (id.includes("limiter") || id.includes("saturation")) return "limiter";
  if (id.includes("deadzone")) return "deadZone";
  if (id.includes("ramp")) return "ramp";
  if (id.includes("fromfile") || id.includes("from_file")) return "fromFile";
  if (id.includes("clock") && (id.includes("source") || id.endsWith(".clock"))) return "clock";
  if (id.includes("booleantoreal") || id.includes("booltoreal")) return "boolToReal";
  if (id.includes("realtoboolean") || id.includes("realtobool")) return "realToBool";
  if (id.includes("logical.and") || (id.includes("logical") && id.endsWith(".and"))) return "and";
  if (id.includes("logical.or") || (id.includes("logical") && id.endsWith(".or"))) return "or";
  if (id.includes("logical.not") || (id.includes("logical") && id.endsWith(".not"))) return "not";
  if (id.includes("logical.switch")) return "switch";
  if (id.includes("boolean") || id.includes("boolstep") || id.includes("boolpulse") || id.includes("boolconst"))
    return "bool";
  if (id.includes("idealdiode") || id.endsWith(".diode")) return "diode";
  if (id.includes("closingswitch") || id.includes("openingswitch")) return "elecSwitch";
  if (id.includes("constantcurrent")) return "isource";
  if (id.includes("sinevoltage")) return "vsine";
  if (id.includes("pulsevoltage")) return "vpulse";
  if (id.includes("idealgearr2t") || id.includes("gearr2t")) return "gearR2T";
  if (id.includes("idealgear")) return "gear";
  if (id.includes("springdamper")) return "springDamper";
  if (id.includes("translational.mass")) return "mass";
  if (id.includes("translational.spring") && !id.includes("damper")) return "tspring";
  if (id.includes("translational.damper")) return "tdamper";
  if (id.includes("translational.fixed") || id.includes("rotational.fixed")) return "fixedFlange";
  if (id.includes("translational.force") && !id.includes("sensor")) return "force";
  if (id.includes("translational.speed") || id.includes("rotational.speed")) return "speed";
  if (id.includes("translational.position") && !id.includes("sensor")) return "tposition";
  if (
    id.includes("sensor") ||
    id.includes("positionsensor") ||
    id.includes("forcesensor") ||
    id.includes("anglesensor") ||
    id.includes("torquesensor") ||
    id.includes("speedsensor") ||
    id.includes("accsensor") ||
    id.includes("potentialsensor") ||
    id.includes("powersensor")
  )
    return "sensor";
  if (id.includes("prismatic")) return "prismatic";
  if (id.includes("spherical")) return "spherical";
  if (id.includes("universal")) return "universal";
  if (id.includes("freemotion")) return "freeMotion";
  if (id.includes("fixedrotation")) return "fixedRotation";
  if (id.includes("bodycylinder")) return "bodyCylinder";
  if (id.includes("pointmass")) return "pointMass";
  if (id.includes("revolute")) return "revolute";
  if (id.includes("inertia")) return "inertia";
  if (id.includes("pulse")) return "pulse";
  if (id.includes("lookup2d") || id.includes("combitable2")) return "lookup2d";
  if (id.includes("lookup") || id.includes("combitable")) return "lookup";
  if (id.includes("ratetransitionud") || id.endsWith("ratetransitionud")) return "rateTransitionUd";
  if (id.includes("ratetransition")) return "rateTransition";
  if (id.includes("unitdelay")) return "unitDelay";
  if (id.includes("zeroorderhold") || id.includes("zeroorder")) return "zoh";
  if (id.includes("firstorderhold")) return "foh";
  if (id.includes("sampler") && id.includes("discrete")) return "sampler";
  if (id.includes("discrete.transfer") || (id.includes("discrete") && id.includes("transfer")))
    return "discreteTf";
  if (id.includes("discrete.")) return "discrete";
  if (id.includes("transferfunction") || id.includes("transfer")) return "transfer";
  if (id.includes("derivative") || (id.includes("continuous") && id.includes("der"))) return "derivative";
  if (id.includes(".pid") || id.endsWith("pid")) return "pid";
  if (id.includes(".pi") || id.endsWith(".pi") || id.includes("continuous.pi")) return "pi";
  if (id.includes("display") || id.includes("sinks.display")) return "display";
  if (id.includes("gain") || ty.endsWith(".gain")) return "gain";
  if (id.includes("add") || ty.endsWith(".add")) return "add";
  if (id.includes("feedback") || ty.endsWith(".feedback")) return "feedback";
  if (id.includes("product") || ty.endsWith(".product")) return "product";
  if (id.includes("integrator") || ty.includes("integrator")) return "integrator";
  if (id.includes("secondorder") || ty.includes("secondorder")) return "secondOrder";
  if (id.includes("firstorder") || ty.includes("firstorder")) return "firstOrder";
  if ((id.includes("math.min") || ty.endsWith(".min")) && !id.includes("min_")) return "min";
  if ((id.includes("math.max") || ty.endsWith(".max")) && !id.includes("max_")) return "max";
  if (id.includes("noise") || ty.includes("noise")) return "noise";
  if (id.includes("statespace") || ty.includes("statespace")) return "stateSpace";
  if (id.includes("step") || ty.endsWith(".step")) return "step";
  if (id.includes("sine") || ty.endsWith(".sine")) return "sine";
  if (id.includes("constant") && n.domain === "signal") return "constant";
  if (id.includes("ground")) return "ground";
  if (id.includes("resistor")) return "resistor";
  if (id.includes("capacitor")) return "capacitor";
  if (id.includes("inductor")) return "inductor";
  if (id.includes("voltage") && id.includes("source")) return "vsource";
  if (id.includes("voltagesensor") || id.includes("currentsensor")) return "sensor";
  // wrap / MSL without Mask: stable domain placeholder (not empty rect)
  if (id.startsWith("wrap.") || id.startsWith("userlib.") || ty.startsWith("modelica.")) {
    if (ty.includes("electrical") || n.domain === "electrical") return "electrical";
    if (ty.includes("mechanics.rotational") || ty.includes("mechanics.translational") || n.domain === "mechanical")
      return "mechanical";
    if (ty.includes("mechanics.multi") || n.domain === "multibody") return "multibody";
    if (n.domain === "signal" || ty.includes("blocks.")) return "signal";
  }
  if (n.domain === "multibody") return "multibody";
  if (n.domain === "mechanical") return "mechanical";
  if (n.domain === "electrical") return "electrical";
  if (n.domain === "signal") return "signal";
  return "generic";
}

function stroke(): string {
  return "var(--text)";
}

function accent(): string {
  return "var(--signal)";
}

/** Inline SVG glyphs (viewBox 0 0 48 32). */
export function BlockIcon({ kind, label }: { kind: string; label?: string }) {
  const s = stroke();
  const a = accent();
  switch (kind) {
    case "inport":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="4" y="6" width="28" height="20" rx="2" fill="var(--inport)" stroke="var(--inport)" strokeWidth="1" />
          <text x="10" y="20" fontSize="11" fill="#fff" fontFamily="inherit">
            In
          </text>
          <polygon points="34,10 44,16 34,22" fill="var(--inport)" />
        </svg>
      );
    case "outport":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polygon points="4,10 14,16 4,22" fill="var(--outport)" />
          <rect x="14" y="6" width="28" height="20" rx="2" fill="var(--outport)" stroke="var(--outport)" strokeWidth="1" />
          <text x="20" y="20" fontSize="10" fill="#fff" fontFamily="inherit">
            Out
          </text>
        </svg>
      );
    case "goto":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="4" y="6" width="32" height="20" rx="2" fill="var(--goto)" stroke="var(--goto)" strokeWidth="1" />
          <text x="10" y="20" fontSize="9" fill="#fff" fontFamily="inherit">
            Goto
          </text>
          <polygon points="38,10 46,16 38,22" fill="var(--goto)" />
        </svg>
      );
    case "from":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polygon points="2,10 10,16 2,22" fill="var(--from)" />
          <rect x="10" y="6" width="34" height="20" rx="2" fill="var(--from)" stroke="var(--from)" strokeWidth="1" />
          <text x="16" y="20" fontSize="9" fill="#fff" fontFamily="inherit">
            From
          </text>
        </svg>
      );
    case "mux":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="20" y="4" width="8" height="24" fill="var(--busbar)" />
          <line x1="6" y1="8" x2="20" y2="12" stroke={a} strokeWidth="1.4" />
          <line x1="6" y1="16" x2="20" y2="16" stroke={a} strokeWidth="1.4" />
          <line x1="6" y1="24" x2="20" y2="20" stroke={a} strokeWidth="1.4" />
          <line x1="28" y1="16" x2="42" y2="16" stroke={a} strokeWidth="1.6" />
        </svg>
      );
    case "demux":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="20" y="4" width="8" height="24" fill="var(--busbar)" />
          <line x1="6" y1="16" x2="20" y2="16" stroke={a} strokeWidth="1.6" />
          <line x1="28" y1="12" x2="42" y2="8" stroke={a} strokeWidth="1.4" />
          <line x1="28" y1="16" x2="42" y2="16" stroke={a} strokeWidth="1.4" />
          <line x1="28" y1="20" x2="42" y2="24" stroke={a} strokeWidth="1.4" />
        </svg>
      );
    case "bus":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="18" y="3" width="12" height="26" fill="var(--busbar)" stroke={s} strokeWidth="0.8" />
          <line x1="4" y1="8" x2="18" y2="10" stroke={a} strokeWidth="1.3" />
          <line x1="4" y1="16" x2="18" y2="16" stroke={a} strokeWidth="1.3" />
          <line x1="4" y1="24" x2="18" y2="22" stroke={a} strokeWidth="1.3" />
          <line x1="30" y1="16" x2="44" y2="16" stroke={a} strokeWidth="2" />
        </svg>
      );
    case "gain":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polygon points="6,4 42,16 6,28" fill="color-mix(in srgb, var(--signal) 18%, var(--panel))" stroke={a} strokeWidth="1.5" />
          <text x="18" y="19" fontSize="9" fill={s} fontFamily="inherit">
            {label && label.length <= 5 ? label : "k"}
          </text>
        </svg>
      );
    case "product":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="8" y="6" width="32" height="20" fill="transparent" stroke={a} strokeWidth="1.5" />
          <text x="20" y="20" fontSize="12" fill={s}>
            {"\u00d7"}
          </text>
        </svg>
      );
    case "division":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="8" y="6" width="32" height="20" fill="transparent" stroke={a} strokeWidth="1.5" />
          <text x="20" y="20" fontSize="14" fill={s}>
            {"\u00f7"}
          </text>
        </svg>
      );
    case "abs":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="12" y="21" fontSize="12" fill={s}>
            |u|
          </text>
        </svg>
      );
    case "sqrt":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="10" y="21" fontSize="11" fill={s}>
            {"\u221a"}u
          </text>
        </svg>
      );
    case "sign":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="10" y="21" fontSize="10" fill={s}>
            sign
          </text>
        </svg>
      );
    case "sin":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <path d="M6 16 Q14 4 22 16 T38 16" fill="none" stroke={a} strokeWidth="1.6" />
        </svg>
      );
    case "cos":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <path d="M6 10 Q14 22 22 10 T38 10" fill="none" stroke={a} strokeWidth="1.6" />
        </svg>
      );
    case "limiter":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="6,26 14,26 22,6 34,6 42,14" fill="none" stroke={a} strokeWidth="1.6" />
        </svg>
      );
    case "deadZone":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="6,26 16,26 20,16 28,16 32,6 42,6" fill="none" stroke={a} strokeWidth="1.6" />
        </svg>
      );
    case "ramp":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="6,26 18,26 38,8" fill="none" stroke={a} strokeWidth="1.8" />
        </svg>
      );
    case "pulse":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="4,24 12,24 12,8 24,8 24,24 36,24 36,8 44,8" fill="none" stroke={a} strokeWidth="1.5" />
        </svg>
      );
    case "lookup":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="6,26 14,18 22,22 30,10 42,14" fill="none" stroke={a} strokeWidth="1.6" />
          <circle cx="14" cy="18" r="1.5" fill={s} />
          <circle cx="22" cy="22" r="1.5" fill={s} />
          <circle cx="30" cy="10" r="1.5" fill={s} />
        </svg>
      );
    case "lookup2d":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="8" y="6" width="20" height="20" fill="transparent" stroke={a} strokeWidth="1.3" />
          <line x1="8" y1="12" x2="28" y2="12" stroke={s} strokeWidth="0.8" />
          <line x1="8" y1="18" x2="28" y2="18" stroke={s} strokeWidth="0.8" />
          <line x1="14" y1="6" x2="14" y2="26" stroke={s} strokeWidth="0.8" />
          <line x1="20" y1="6" x2="20" y2="26" stroke={s} strokeWidth="0.8" />
          <text x="32" y="20" fontSize="8" fill={s}>
            2D
          </text>
        </svg>
      );
    case "sampler":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="4,24 12,24 12,8 20,8 20,24 28,24 28,8 36,8 36,24 44,24" fill="none" stroke={a} strokeWidth="1.5" />
        </svg>
      );
    case "zoh":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="4,24 14,24 14,10 26,10 26,20 38,20 38,8 44,8" fill="none" stroke={a} strokeWidth="1.6" />
        </svg>
      );
    case "foh":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="4,24 14,24 26,10 38,18 44,8" fill="none" stroke={a} strokeWidth="1.6" />
        </svg>
      );
    case "unitDelay":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="10" y="6" width="28" height="20" fill="transparent" stroke={a} strokeWidth="1.5" />
          <text x="16" y="20" fontSize="10" fill={s}>
            z^-1
          </text>
        </svg>
      );
    case "rateTransition":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="4,22 12,22 12,10 22,10" fill="none" stroke={a} strokeWidth="1.5" />
          <polyline points="26,22 34,22 34,10 44,10" fill="none" stroke={s} strokeWidth="1.5" />
          <path d="M22 16 L26 16" stroke={a} strokeWidth="1.6" markerEnd="none" />
          <text x="18" y="28" fontSize="7" fill={s}>
            RT
          </text>
        </svg>
      );
    case "rateTransitionUd":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="4,22 12,22 12,10 20,10" fill="none" stroke={a} strokeWidth="1.4" />
          <rect x="22" y="8" width="14" height="16" fill="transparent" stroke={a} strokeWidth="1.3" />
          <text x="24" y="19" fontSize="8" fill={s}>
            z-1
          </text>
          <polyline points="36,16 44,16" fill="none" stroke={s} strokeWidth="1.4" />
        </svg>
      );
    case "discreteTf":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="6" y="20" fontSize="9" fill={s}>
            b(z)/a(z)
          </text>
        </svg>
      );
    case "discrete":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="8" y="6" width="32" height="20" fill="transparent" stroke={a} strokeWidth="1.4" strokeDasharray="3 2" />
          <text x="14" y="20" fontSize="9" fill={s}>
            Ts
          </text>
        </svg>
      );
    case "transfer":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="8" y="20" fontSize="9" fill={s}>
            b/a
          </text>
        </svg>
      );
    case "derivative":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="12" y="21" fontSize="12" fill={s} fontStyle="italic">
            d/dt
          </text>
        </svg>
      );
    case "pi":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="16" y="21" fontSize="12" fill={s}>
            PI
          </text>
        </svg>
      );
    case "pid":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="12" y="21" fontSize="12" fill={s}>
            PID
          </text>
        </svg>
      );
    case "display":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="6" y="6" width="36" height="20" rx="2" fill="transparent" stroke={a} strokeWidth="1.4" />
          <text x="14" y="20" fontSize="9" fill={s}>
            0.00
          </text>
        </svg>
      );
    case "fromFile":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="8" y="6" width="24" height="20" rx="1" fill="transparent" stroke={a} strokeWidth="1.4" />
          <path d="M14 12 h12 M14 16 h12 M14 20 h8" stroke={s} strokeWidth="1.2" />
          <polygon points="34,10 44,16 34,22" fill={a} />
        </svg>
      );
    case "clock":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="24" cy="16" r="10" fill="transparent" stroke={a} strokeWidth="1.5" />
          <line x1="24" y1="16" x2="24" y2="9" stroke={s} strokeWidth="1.4" />
          <line x1="24" y1="16" x2="30" y2="18" stroke={s} strokeWidth="1.4" />
        </svg>
      );
    case "bool":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="6,22 18,22 18,10 42,10" fill="none" stroke={a} strokeWidth="1.8" />
          <text x="28" y="26" fontSize="8" fill={s}>
            bool
          </text>
        </svg>
      );
    case "and":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="14" y="21" fontSize="12" fill={s}>
            AND
          </text>
        </svg>
      );
    case "or":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="16" y="21" fontSize="12" fill={s}>
            OR
          </text>
        </svg>
      );
    case "not":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="14" y="21" fontSize="12" fill={s}>
            NOT
          </text>
        </svg>
      );
    case "switch":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <line x1="6" y1="8" x2="20" y2="8" stroke={a} strokeWidth="1.4" />
          <line x1="6" y1="24" x2="20" y2="24" stroke={a} strokeWidth="1.4" />
          <line x1="20" y1="8" x2="34" y2="16" stroke={s} strokeWidth="1.6" />
          <line x1="34" y1="16" x2="44" y2="16" stroke={a} strokeWidth="1.6" />
          <circle cx="24" cy="26" r="2" fill={s} />
        </svg>
      );
    case "boolToReal":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="6" y="20" fontSize="9" fill={s}>
            B{"\u2192"}R
          </text>
        </svg>
      );
    case "realToBool":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="6" y="20" fontSize="9" fill={s}>
            R{"\u2192"}B
          </text>
        </svg>
      );
    case "diode":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <line x1="4" y1="16" x2="16" y2="16" stroke={s} strokeWidth="1.4" />
          <polygon points="16,8 30,16 16,24" fill="transparent" stroke={a} strokeWidth="1.5" />
          <line x1="30" y1="8" x2="30" y2="24" stroke={a} strokeWidth="1.6" />
          <line x1="30" y1="16" x2="44" y2="16" stroke={s} strokeWidth="1.4" />
        </svg>
      );
    case "elecSwitch":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <line x1="4" y1="20" x2="18" y2="20" stroke={s} strokeWidth="1.4" />
          <line x1="18" y1="20" x2="32" y2="10" stroke={a} strokeWidth="1.6" />
          <line x1="32" y1="20" x2="44" y2="20" stroke={s} strokeWidth="1.4" />
          <circle cx="18" cy="20" r="2" fill={s} />
          <circle cx="32" cy="20" r="2" fill={s} />
        </svg>
      );
    case "isource":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="24" cy="16" r="11" fill="transparent" stroke={a} strokeWidth="1.5" />
          <text x="20" y="20" fontSize="11" fill={s}>
            I
          </text>
        </svg>
      );
    case "vsine":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="24" cy="16" r="11" fill="transparent" stroke={a} strokeWidth="1.5" />
          <path d="M14 16 Q18 8 24 16 T34 16" fill="none" stroke={s} strokeWidth="1.4" />
        </svg>
      );
    case "vpulse":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="24" cy="16" r="11" fill="transparent" stroke={a} strokeWidth="1.5" />
          <polyline points="14,22 20,22 20,10 28,10 28,22 34,22" fill="none" stroke={s} strokeWidth="1.3" />
        </svg>
      );
    case "gear":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="18" cy="16" r="7" fill="transparent" stroke={a} strokeWidth="1.4" />
          <circle cx="32" cy="16" r="5" fill="transparent" stroke={a} strokeWidth="1.4" />
        </svg>
      );
    case "springDamper":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="4,10 10,10 13,4 19,16 25,4 31,16 37,10 44,10" fill="none" stroke={a} strokeWidth="1.3" />
          <line x1="4" y1="22" x2="44" y2="22" stroke={s} strokeWidth="1.2" />
          <line x1="24" y1="18" x2="24" y2="26" stroke={s} strokeWidth="1.2" />
        </svg>
      );
    case "fixedFlange":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <line x1="8" y1="6" x2="8" y2="26" stroke={s} strokeWidth="2" />
          <line x1="8" y1="16" x2="40" y2="16" stroke={a} strokeWidth="1.5" />
        </svg>
      );
    case "speed":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="12" y="21" fontSize="11" fill={s}>
            w
          </text>
        </svg>
      );
    case "gearR2T":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="14" cy="16" r="6" fill="transparent" stroke={a} strokeWidth="1.3" />
          <line x1="22" y1="16" x2="42" y2="16" stroke={s} strokeWidth="1.5" />
          <text x="26" y="12" fontSize="7" fill={s}>
            R2T
          </text>
        </svg>
      );
    case "mass":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="14" y="8" width="20" height="16" fill="transparent" stroke={a} strokeWidth="1.5" />
          <text x="18" y="20" fontSize="9" fill={s}>
            m
          </text>
        </svg>
      );
    case "tspring":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="4,16 10,16 13,8 19,24 25,8 31,24 37,16 44,16" fill="none" stroke={a} strokeWidth="1.4" />
        </svg>
      );
    case "tdamper":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <line x1="4" y1="16" x2="16" y2="16" stroke={s} strokeWidth="1.3" />
          <rect x="16" y="10" width="10" height="12" fill="transparent" stroke={a} strokeWidth="1.3" />
          <line x1="21" y1="10" x2="21" y2="22" stroke={a} strokeWidth="1.2" />
          <line x1="26" y1="16" x2="44" y2="16" stroke={s} strokeWidth="1.3" />
        </svg>
      );
    case "force":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <line x1="6" y1="16" x2="34" y2="16" stroke={a} strokeWidth="1.6" />
          <polygon points="34,10 44,16 34,22" fill={a} />
        </svg>
      );
    case "tposition":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="16" y="21" fontSize="11" fill={s}>
            s
          </text>
        </svg>
      );
    case "prismatic":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="8" y="12" width="32" height="8" fill="transparent" stroke={a} strokeWidth="1.4" />
          <line x1="18" y1="8" x2="18" y2="24" stroke={s} strokeWidth="1.2" />
          <line x1="30" y1="8" x2="30" y2="24" stroke={s} strokeWidth="1.2" />
        </svg>
      );
    case "spherical":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="24" cy="16" r="9" fill="transparent" stroke={a} strokeWidth="1.5" />
          <circle cx="24" cy="16" r="2" fill={s} />
        </svg>
      );
    case "universal":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <line x1="6" y1="16" x2="20" y2="16" stroke={a} strokeWidth="1.5" />
          <line x1="28" y1="8" x2="28" y2="24" stroke={a} strokeWidth="1.5" />
          <circle cx="24" cy="16" r="4" fill="transparent" stroke={s} strokeWidth="1.3" />
          <line x1="32" y1="16" x2="42" y2="16" stroke={a} strokeWidth="1.5" />
        </svg>
      );
    case "freeMotion":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="16" cy="16" r="5" fill="transparent" stroke={a} strokeWidth="1.3" />
          <circle cx="32" cy="16" r="5" fill="transparent" stroke={a} strokeWidth="1.3" />
          <path d="M21 16 Q24 8 27 16" fill="none" stroke={s} strokeWidth="1.2" />
        </svg>
      );
    case "fixedRotation":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="8" y="10" width="14" height="12" fill="transparent" stroke={a} strokeWidth="1.3" />
          <rect x="26" y="10" width="14" height="12" fill="transparent" stroke={a} strokeWidth="1.3" />
          <path d="M22 16 A6 6 0 0 1 26 16" fill="none" stroke={s} strokeWidth="1.3" />
        </svg>
      );
    case "bodyCylinder":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <ellipse cx="12" cy="16" rx="4" ry="8" fill="transparent" stroke={a} strokeWidth="1.3" />
          <line x1="12" y1="8" x2="36" y2="8" stroke={a} strokeWidth="1.3" />
          <line x1="12" y1="24" x2="36" y2="24" stroke={a} strokeWidth="1.3" />
          <ellipse cx="36" cy="16" rx="4" ry="8" fill="transparent" stroke={a} strokeWidth="1.3" />
        </svg>
      );
    case "pointMass":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="24" cy="16" r="5" fill={a} />
          <line x1="8" y1="16" x2="18" y2="16" stroke={s} strokeWidth="1.2" />
        </svg>
      );
    case "revolute":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="24" cy="16" r="8" fill="transparent" stroke={a} strokeWidth="1.5" />
          <line x1="24" y1="8" x2="24" y2="24" stroke={s} strokeWidth="1.3" />
        </svg>
      );
    case "inertia":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="24" cy="16" r="9" fill="transparent" stroke={a} strokeWidth="1.5" />
          <text x="20" y="20" fontSize="9" fill={s}>
            J
          </text>
        </svg>
      );
    case "add":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="24" cy="16" r="11" fill="transparent" stroke={a} strokeWidth="1.5" />
          <path d="M24 9 v14 M17 16 h14" stroke={s} strokeWidth="1.6" />
        </svg>
      );
    case "feedback":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="24" cy="16" r="11" fill="transparent" stroke={a} strokeWidth="1.5" />
          <text x="16" y="20" fontSize="11" fill={s}>
            {"\u2211"}
          </text>
        </svg>
      );
    case "integrator":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="6" y="4" width="36" height="24" fill="transparent" stroke={a} strokeWidth="1.5" />
          <text x="18" y="21" fontSize="14" fill={s} fontStyle="italic">
            {"\u222b"}
          </text>
        </svg>
      );
    case "firstOrder":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="6" y="4" width="36" height="24" fill="transparent" stroke={a} strokeWidth="1.5" />
          <text x="10" y="20" fontSize="9" fill={s}>
            1/(Ts+1)
          </text>
        </svg>
      );
    case "secondOrder":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="6" y="4" width="36" height="24" fill="transparent" stroke={a} strokeWidth="1.5" />
          <text x="8" y="20" fontSize="8" fill={s}>
            2nd
          </text>
        </svg>
      );
    case "min":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="14" y="21" fontSize="12" fill={s}>
            min
          </text>
        </svg>
      );
    case "max":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="14" y="21" fontSize="12" fill={s}>
            max
          </text>
        </svg>
      );
    case "noise":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="4,20 10,12 16,22 22,8 28,18 34,10 40,16 44,14" fill="none" stroke={a} strokeWidth="1.4" />
        </svg>
      );
    case "stateSpace":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <text x="10" y="20" fontSize="10" fill={s}>
            dx=Ax
          </text>
        </svg>
      );
    case "step":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="6,24 22,24 22,8 42,8" fill="none" stroke={a} strokeWidth="1.8" />
        </svg>
      );
    case "sine":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <path
            d="M4 16 Q12 4 20 16 T36 16 T44 16"
            fill="none"
            stroke={a}
            strokeWidth="1.8"
          />
        </svg>
      );
    case "constant":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <line x1="8" y1="16" x2="40" y2="16" stroke={a} strokeWidth="1.8" />
          <text x="18" y="12" fontSize="8" fill={s}>
            const
          </text>
        </svg>
      );
    case "ground":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <line x1="24" y1="4" x2="24" y2="14" stroke={s} strokeWidth="1.5" />
          <line x1="12" y1="14" x2="36" y2="14" stroke={s} strokeWidth="1.5" />
          <line x1="16" y1="19" x2="32" y2="19" stroke={s} strokeWidth="1.4" />
          <line x1="20" y1="24" x2="28" y2="24" stroke={s} strokeWidth="1.3" />
        </svg>
      );
    case "resistor":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline
            points="4,16 10,16 13,8 19,24 25,8 31,24 37,16 44,16"
            fill="none"
            stroke={a}
            strokeWidth="1.5"
          />
        </svg>
      );
    case "capacitor":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <line x1="6" y1="16" x2="20" y2="16" stroke={s} strokeWidth="1.5" />
          <line x1="20" y1="6" x2="20" y2="26" stroke={a} strokeWidth="2" />
          <line x1="28" y1="6" x2="28" y2="26" stroke={a} strokeWidth="2" />
          <line x1="28" y1="16" x2="42" y2="16" stroke={s} strokeWidth="1.5" />
        </svg>
      );
    case "inductor":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <path
            d="M4 18 Q8 6 12 18 Q16 6 20 18 Q24 6 28 18 Q32 6 36 18 Q40 6 44 18"
            fill="none"
            stroke={a}
            strokeWidth="1.5"
          />
        </svg>
      );
    case "vsource":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="24" cy="16" r="11" fill="transparent" stroke={a} strokeWidth="1.5" />
          <text x="19" y="20" fontSize="11" fill={s}>
            V
          </text>
        </svg>
      );
    case "sensor":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <circle cx="24" cy="16" r="10" fill="transparent" stroke={a} strokeWidth="1.5" />
          <path d="M18 20 L24 8 L30 20" fill="none" stroke={s} strokeWidth="1.4" />
        </svg>
      );
    case "subsystem":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="6" y="5" width="36" height="22" fill="transparent" stroke="var(--block-sub-border)" strokeWidth="2" />
          <rect x="14" y="11" width="20" height="10" fill="transparent" stroke={s} strokeWidth="1.2" />
        </svg>
      );
    case "variant":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="6" y="5" width="36" height="22" fill="transparent" stroke="var(--block-sub-border)" strokeWidth="2" />
          <path d="M16 22 L24 10 L32 22" fill="none" stroke={s} strokeWidth="1.4" />
          <text x="20" y="26" fontSize="7" fill={s}>
            V
          </text>
        </svg>
      );
    case "modelRef":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="4" y="6" width="28" height="20" rx="2" fill="transparent" stroke={a} strokeWidth="1.5" />
          <path d="M34 10 h8 v12 h-8" fill="none" stroke={s} strokeWidth="1.4" />
          <text x="8" y="20" fontSize="8" fill={s}>
            Ref
          </text>
        </svg>
      );
    case "fmu":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="6" y="5" width="36" height="22" fill="transparent" stroke="var(--block-fmu-border)" strokeWidth="1.5" />
          <text x="14" y="20" fontSize="10" fill={s}>
            FMU
          </text>
        </svg>
      );
    case "mechanical":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="10" y="10" width="28" height="12" fill="transparent" stroke={a} strokeWidth="1.4" />
          <circle cx="16" cy="16" r="3" fill="transparent" stroke={s} strokeWidth="1.2" />
          <circle cx="32" cy="16" r="3" fill="transparent" stroke={s} strokeWidth="1.2" />
        </svg>
      );
    case "multibody":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polygon points="24,6 38,26 10,26" fill="transparent" stroke={a} strokeWidth="1.4" />
        </svg>
      );
    case "electrical":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <polyline points="8,16 16,16 20,8 28,24 32,16 40,16" fill="none" stroke={a} strokeWidth="1.5" />
        </svg>
      );
    case "signal":
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="8" y="6" width="32" height="20" fill="transparent" stroke={a} strokeWidth="1.3" />
          <polyline points="12,22 20,10 28,18 36,12" fill="none" stroke={s} strokeWidth="1.4" />
        </svg>
      );
    default:
      return (
        <svg className="slx-block-icon" viewBox="0 0 48 32" aria-hidden>
          <rect x="8" y="6" width="32" height="20" fill="transparent" stroke={a} strokeWidth="1.3" />
          {label ? (
            <text x="24" y="20" fontSize="8" fill={s} textAnchor="middle">
              {label.length > 6 ? label.slice(0, 6) : label}
            </text>
          ) : null}
        </svg>
      );
  }
}
