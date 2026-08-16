import type { CatalogEntry } from "../api/types";
import { getI18nLocale } from "../i18n";

type LocaleName = { en: string; zh: string };

const BY_ID: [string, LocaleName][] = [
  ["inport", { en: "Inport", zh: "输入口" }],
  ["outport", { en: "Outport", zh: "输出口" }],
  ["routing.goto", { en: "Goto", zh: "Goto" }],
  ["routing.from", { en: "From", zh: "From" }],
  ["routing.mux", { en: "Mux", zh: "多路复用" }],
  ["routing.demux", { en: "Demux", zh: "多路分解" }],
  ["routing.bus", { en: "Bus", zh: "总线" }],
  ["sinks.display", { en: "Display", zh: "显示" }],
  ["sinks.scope", { en: "Scope", zh: "示波器" }],
  ["lookup2d", { en: "Lookup2D", zh: "二维查表" }],
  ["lookup", { en: "Lookup1D", zh: "一维查表" }],
  ["discrete.rateTransitionUd", { en: "Rate Transition (UD)", zh: "速率转换 (UD)" }],
  ["discrete.rateTransition", { en: "Rate Transition", zh: "速率转换" }],
  ["discrete.sampler", { en: "Sampler", zh: "采样器" }],
  ["discrete.zeroOrderHold", { en: "ZOH", zh: "零阶保持" }],
  ["discrete.firstOrderHold", { en: "FOH", zh: "一阶保持" }],
  ["discrete.unitDelay", { en: "UnitDelay", zh: "单位延迟" }],
  ["discrete.transferFunction", { en: "Discrete TF", zh: "离散传递函数" }],
  ["logical.and", { en: "And", zh: "与" }],
  ["logical.or", { en: "Or", zh: "或" }],
  ["logical.not", { en: "Not", zh: "非" }],
  ["logical.switch", { en: "Switch", zh: "开关" }],
  ["booleanToReal", { en: "BoolToReal", zh: "布尔转实数" }],
  ["realToBoolean", { en: "RealToBool", zh: "实数转布尔" }],
  ["idealDiode", { en: "Diode", zh: "二极管" }],
  ["idealClosingSwitch", { en: "ClosingSwitch", zh: "闭合开关" }],
  ["idealOpeningSwitch", { en: "OpeningSwitch", zh: "断开开关" }],
  ["sineVoltage", { en: "SineVoltage", zh: "正弦电压" }],
  ["pulseVoltage", { en: "PulseVoltage", zh: "脉冲电压" }],
  ["constantCurrent", { en: "CurrentSource", zh: "电流源" }],
  ["rotational.fixed", { en: "Fixed", zh: "固定端" }],
  ["springDamper", { en: "SpringDamper", zh: "弹簧阻尼" }],
  ["idealGear", { en: "IdealGear", zh: "理想齿轮" }],
  ["rotational.speed", { en: "Speed", zh: "转速源" }],
  ["angleSensor", { en: "AngleSensor", zh: "角度传感器" }],
  ["torqueSensor", { en: "TorqueSensor", zh: "转矩传感器" }],
  ["translational.mass", { en: "Mass", zh: "质量" }],
  ["translational.springDamper", { en: "SpringDamper", zh: "弹簧阻尼" }],
  ["translational.spring", { en: "Spring", zh: "弹簧" }],
  ["translational.damper", { en: "Damper", zh: "阻尼" }],
  ["translational.fixed", { en: "Fixed", zh: "固定端" }],
  ["translational.forceSensor", { en: "ForceSensor", zh: "力传感器" }],
  ["translational.force", { en: "Force", zh: "力源" }],
  ["translational.speed", { en: "Speed", zh: "速度源" }],
  ["translational.positionSensor", { en: "PosSensor", zh: "位移传感器" }],
  ["translational.position", { en: "Position", zh: "位移源" }],
  ["idealGearR2T", { en: "GearR2T", zh: "转动-平动齿轮" }],
  ["prismaticActuated", { en: "PrismaticA", zh: "驱动移动副" }],
  ["prismatic", { en: "Prismatic", zh: "移动副" }],
  ["spherical", { en: "Spherical", zh: "球铰" }],
  ["universal", { en: "Universal", zh: "万向节" }],
  ["freeMotion", { en: "FreeMotion", zh: "自由运动" }],
  ["fixedRotation", { en: "FixedRotation", zh: "固定转动" }],
  ["bodyCylinder", { en: "BodyCylinder", zh: "圆柱体" }],
  ["pointMass", { en: "PointMass", zh: "质点" }],
  ["revoluteActuated", { en: "RevoluteA", zh: "驱动转动副" }],
  ["revolute", { en: "Revolute", zh: "转动副" }],
  ["stategraph.initialStep", { en: "InitialStep", zh: "初始步" }],
  ["stategraph.stepWithSignal", { en: "StepWithSignal", zh: "步(信号)" }],
  ["stategraph.transitionWithSignal", { en: "TransitionWithSignal", zh: "变迁(信号)" }],
  ["stategraph.transition", { en: "Transition", zh: "变迁" }],
  ["stategraph.step", { en: "Step", zh: "步" }],
  ["sources.step", { en: "Step", zh: "阶跃" }],
  ["sources.sine", { en: "Sine", zh: "正弦" }],
  ["sources.constant", { en: "Constant", zh: "常量" }],
  ["sources.ramp", { en: "Ramp", zh: "斜坡" }],
  ["math.gain", { en: "Gain", zh: "增益" }],
  ["math.add", { en: "Add", zh: "加法" }],
  ["math.feedback", { en: "Feedback", zh: "反馈" }],
  ["continuous.integrator", { en: "Integrator", zh: "积分" }],
  ["continuous.firstOrder", { en: "FirstOrder", zh: "一阶" }],
  ["analog.basic.ground", { en: "Ground", zh: "地" }],
  ["analog.basic.resistor", { en: "Resistor", zh: "电阻" }],
  ["analog.basic.capacitor", { en: "Capacitor", zh: "电容" }],
  ["analog.basic.inductor", { en: "Inductor", zh: "电感" }],
];

const CATEGORY: Record<string, LocaleName> = {
  Sources: { en: "Sources", zh: "源" },
  Math: { en: "Math", zh: "数学" },
  Continuous: { en: "Continuous", zh: "连续" },
  Discrete: { en: "Discrete", zh: "离散" },
  StateGraph: { en: "StateGraph", zh: "状态图" },
  Logical: { en: "Logical", zh: "逻辑" },
  Nonlinear: { en: "Nonlinear", zh: "非线性" },
  Tables: { en: "Tables", zh: "查表" },
  Sinks: { en: "Sinks", zh: "接收" },
  Routing: { en: "Routing", zh: "路由" },
  Terminals: { en: "Terminals", zh: "端子" },
  Analog: { en: "Analog", zh: "电气" },
  Rotational: { en: "Rotational", zh: "转动" },
  Translational: { en: "Translational", zh: "平动" },
  MultiBody: { en: "MultiBody", zh: "多体" },
  Sensors: { en: "Sensors", zh: "传感器" },
  Draw: { en: "Draw", zh: "绘图" },
  ModelRef: { en: "Model Reference", zh: "模型引用" },
  UserLib: { en: "User Library", zh: "用户库" },
  Favorites: { en: "Favorites", zh: "收藏" },
  Recents: { en: "Recent", zh: "最近使用" },
};

function pick(name: LocaleName): string {
  return getI18nLocale() === "zh" ? name.zh : name.en;
}

export function catalogLabel(e: CatalogEntry): string {
  const id = e.id.toLowerCase();
  for (const [needle, names] of BY_ID) {
    if (id.includes(needle.toLowerCase())) return pick(names);
  }
  if (e.category === "ModelRef" || e.id.startsWith("modelref.")) {
    return e.typeName.split(".").pop() ?? e.typeName;
  }
  if (e.category === "UserLib" || e.id.startsWith("userlib.")) {
    return e.typeName.split(".").pop() ?? e.typeName;
  }
  return e.defaultInstancePrefix;
}

export function catalogCategoryLabel(category: string): string {
  const hit = CATEGORY[category];
  return hit ? pick(hit) : category;
}

export function catalogDomainHint(e: CatalogEntry): string {
  if (e.domain === "draw") return "draw";
  if (e.category === "Terminals") return "terminal";
  if (e.category === "Routing") return "routing";
  if (e.category === "Discrete") return "discrete";
  if (e.category === "StateGraph") return "discrete";
  if (e.category === "Logical") return "logical";
  if (e.category === "Translational" || e.category === "Rotational") return "1d";
  if (e.category === "MultiBody") return "3d";
  if (e.category === "ModelRef") return "modelRef";
  if (e.category === "UserLib") return "userLib";
  return e.domain;
}
