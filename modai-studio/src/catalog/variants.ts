/** Compatible type switches for Variant-like reconfiguration. */
export const VARIANT_GROUPS: Record<string, { typeName: string; catalogId: string; label: string }[]> = {
  continuous: [
    {
      typeName: "Modelica.Blocks.Continuous.Integrator",
      catalogId: "blocks.continuous.integrator",
      label: "Integrator",
    },
    {
      typeName: "Modelica.Blocks.Continuous.FirstOrder",
      catalogId: "blocks.continuous.firstOrder",
      label: "FirstOrder",
    },
    {
      typeName: "Modelica.Blocks.Continuous.Derivative",
      catalogId: "blocks.continuous.derivative",
      label: "Derivative",
    },
    {
      typeName: "Modelica.Blocks.Continuous.TransferFunction",
      catalogId: "blocks.continuous.transferFunction",
      label: "TransferFunction",
    },
    {
      typeName: "Modelica.Blocks.Continuous.PI",
      catalogId: "blocks.continuous.pi",
      label: "PI",
    },
    {
      typeName: "Modelica.Blocks.Continuous.PID",
      catalogId: "blocks.continuous.pid",
      label: "PID",
    },
  ],
  sources: [
    {
      typeName: "Modelica.Blocks.Sources.Step",
      catalogId: "blocks.sources.step",
      label: "Step",
    },
    {
      typeName: "Modelica.Blocks.Sources.Sine",
      catalogId: "blocks.sources.sine",
      label: "Sine",
    },
    {
      typeName: "Modelica.Blocks.Sources.Constant",
      catalogId: "blocks.sources.constant",
      label: "Constant",
    },
    {
      typeName: "Modelica.Blocks.Sources.Ramp",
      catalogId: "blocks.sources.ramp",
      label: "Ramp",
    },
    {
      typeName: "Modelica.Blocks.Sources.Pulse",
      catalogId: "blocks.sources.pulse",
      label: "Pulse",
    },
    {
      typeName: "Modelica.Blocks.Sources.ExpSine",
      catalogId: "blocks.sources.expSine",
      label: "ExpSine",
    },
  ],
  math: [
    {
      typeName: "Modelica.Blocks.Math.Gain",
      catalogId: "blocks.math.gain",
      label: "Gain",
    },
    {
      typeName: "Modelica.Blocks.Math.Add",
      catalogId: "blocks.math.add",
      label: "Add",
    },
    {
      typeName: "Modelica.Blocks.Math.Product",
      catalogId: "blocks.math.product",
      label: "Product",
    },
    {
      typeName: "Modelica.Blocks.Math.Division",
      catalogId: "blocks.math.division",
      label: "Division",
    },
    {
      typeName: "Modelica.Blocks.Math.Abs",
      catalogId: "blocks.math.abs",
      label: "Abs",
    },
    {
      typeName: "Modelica.Blocks.Math.Sqrt",
      catalogId: "blocks.math.sqrt",
      label: "Sqrt",
    },
    {
      typeName: "Modelica.Blocks.Math.Sign",
      catalogId: "blocks.math.sign",
      label: "Sign",
    },
    {
      typeName: "Modelica.Blocks.Math.Sin",
      catalogId: "blocks.math.sin",
      label: "Sin",
    },
    {
      typeName: "Modelica.Blocks.Math.Cos",
      catalogId: "blocks.math.cos",
      label: "Cos",
    },
  ],
  nonlinear: [
    {
      typeName: "Modelica.Blocks.Nonlinear.Limiter",
      catalogId: "blocks.nonlinear.limiter",
      label: "Limiter",
    },
    {
      typeName: "Modelica.Blocks.Nonlinear.DeadZone",
      catalogId: "blocks.nonlinear.deadZone",
      label: "DeadZone",
    },
  ],
  mux: [
    {
      typeName: "Modelica.Blocks.Routing.Multiplex2",
      catalogId: "blocks.routing.mux2",
      label: "Mux 2",
    },
    {
      typeName: "Modelica.Blocks.Routing.Multiplex3",
      catalogId: "blocks.routing.mux3",
      label: "Mux 3",
    },
    {
      typeName: "Modelica.Blocks.Routing.Multiplex4",
      catalogId: "blocks.routing.mux4",
      label: "Mux 4",
    },
  ],
  demux: [
    {
      typeName: "Modelica.Blocks.Routing.DeMultiplex2",
      catalogId: "blocks.routing.demux2",
      label: "Demux 2",
    },
    {
      typeName: "Modelica.Blocks.Routing.DeMultiplex3",
      catalogId: "blocks.routing.demux3",
      label: "Demux 3",
    },
    {
      typeName: "Modelica.Blocks.Routing.DeMultiplex4",
      catalogId: "blocks.routing.demux4",
      label: "Demux 4",
    },
  ],
  bus: [
    {
      typeName: "Modelica.Blocks.Routing.Multiplex2",
      catalogId: "blocks.routing.bus2",
      label: "Bus 2",
    },
    {
      typeName: "Modelica.Blocks.Routing.Multiplex3",
      catalogId: "blocks.routing.bus3",
      label: "Bus 3",
    },
  ],
  discrete: [
    {
      typeName: "Modelica.Blocks.Discrete.Sampler",
      catalogId: "blocks.discrete.sampler",
      label: "Sampler",
    },
    {
      typeName: "Modelica.Blocks.Discrete.ZeroOrderHold",
      catalogId: "blocks.discrete.zeroOrderHold",
      label: "ZeroOrderHold",
    },
    {
      typeName: "Modelica.Blocks.Discrete.FirstOrderHold",
      catalogId: "blocks.discrete.firstOrderHold",
      label: "FirstOrderHold",
    },
    {
      typeName: "Modelica.Blocks.Discrete.UnitDelay",
      catalogId: "blocks.discrete.unitDelay",
      label: "UnitDelay",
    },
    {
      typeName: "Modelica.Blocks.Discrete.TransferFunction",
      catalogId: "blocks.discrete.transferFunction",
      label: "Discrete TF",
    },
    {
      typeName: "Modelica.Blocks.Discrete.ZeroOrderHold",
      catalogId: "blocks.discrete.rateTransition",
      label: "Rate Transition",
    },
    {
      typeName: "Modelica.Blocks.Discrete.UnitDelay",
      catalogId: "blocks.discrete.rateTransitionUd",
      label: "Rate Transition (UD)",
    },
  ],
  tables: [
    {
      typeName: "Modelica.Blocks.Tables.CombiTable1Ds",
      catalogId: "blocks.tables.lookup1d",
      label: "Lookup1D",
    },
    {
      typeName: "Modelica.Blocks.Tables.CombiTable2Ds",
      catalogId: "blocks.tables.lookup2d",
      label: "Lookup2D",
    },
  ],
  booleanSources: [
    {
      typeName: "Modelica.Blocks.Sources.BooleanStep",
      catalogId: "blocks.sources.booleanStep",
      label: "BooleanStep",
    },
    {
      typeName: "Modelica.Blocks.Sources.BooleanPulse",
      catalogId: "blocks.sources.booleanPulse",
      label: "BooleanPulse",
    },
    {
      typeName: "Modelica.Blocks.Sources.BooleanConstant",
      catalogId: "blocks.sources.booleanConstant",
      label: "BooleanConstant",
    },
  ],
  logical: [
    {
      typeName: "Modelica.Blocks.Logical.And",
      catalogId: "blocks.logical.and",
      label: "And",
    },
    {
      typeName: "Modelica.Blocks.Logical.Or",
      catalogId: "blocks.logical.or",
      label: "Or",
    },
    {
      typeName: "Modelica.Blocks.Logical.Not",
      catalogId: "blocks.logical.not",
      label: "Not",
    },
    {
      typeName: "Modelica.Blocks.Logical.Switch",
      catalogId: "blocks.logical.switch",
      label: "Switch",
    },
  ],
  translational: [
    {
      typeName: "Modelica.Mechanics.Translational.Components.Mass",
      catalogId: "mechanics.translational.mass",
      label: "Mass",
    },
    {
      typeName: "Modelica.Mechanics.Translational.Components.Spring",
      catalogId: "mechanics.translational.spring",
      label: "Spring",
    },
    {
      typeName: "Modelica.Mechanics.Translational.Components.Damper",
      catalogId: "mechanics.translational.damper",
      label: "Damper",
    },
    {
      typeName: "Modelica.Mechanics.Translational.Components.SpringDamper",
      catalogId: "mechanics.translational.springDamper",
      label: "SpringDamper",
    },
  ],
  multibodyJoints: [
    {
      typeName: "Modelica.Mechanics.MultiBody.Joints.Revolute",
      catalogId: "multibody.revolute",
      label: "Revolute",
    },
    {
      typeName: "Modelica.Mechanics.MultiBody.Joints.Prismatic",
      catalogId: "multibody.prismatic",
      label: "Prismatic",
    },
    {
      typeName: "Modelica.Mechanics.MultiBody.Joints.Spherical",
      catalogId: "multibody.spherical",
      label: "Spherical",
    },
    {
      typeName: "Modelica.Mechanics.MultiBody.Joints.Universal",
      catalogId: "multibody.universal",
      label: "Universal",
    },
    {
      typeName: "Modelica.Mechanics.MultiBody.Joints.FreeMotion",
      catalogId: "multibody.freeMotion",
      label: "FreeMotion",
    },
  ],
};

export function variantOptionsFor(typeName: string, catalogId?: string) {
  const cid = (catalogId ?? "").toLowerCase();
  if (cid.includes("routing.bus")) return VARIANT_GROUPS.bus;
  if (cid.includes("routing.demux")) return VARIANT_GROUPS.demux;
  if (cid.includes("routing.mux")) return VARIANT_GROUPS.mux;
  if (cid.includes("nonlinear.")) return VARIANT_GROUPS.nonlinear;
  if (cid.includes("discrete.") || typeName.includes("Blocks.Discrete")) return VARIANT_GROUPS.discrete;
  if (cid.includes("tables.") || typeName.includes("Blocks.Tables")) return VARIANT_GROUPS.tables;
  if (cid.includes("logical.") || typeName.includes("Blocks.Logical")) return VARIANT_GROUPS.logical;
  if (cid.includes("boolean") || typeName.includes("Boolean")) return VARIANT_GROUPS.booleanSources;
  if (cid.includes("translational.") && (cid.includes("mass") || cid.includes("spring") || cid.includes("damper"))) {
    return VARIANT_GROUPS.translational;
  }
  if (
    cid.includes("multibody.") &&
    (cid.includes("revolute") ||
      cid.includes("prismatic") ||
      cid.includes("spherical") ||
      cid.includes("universal") ||
      cid.includes("freemotion"))
  ) {
    // Prefer plain joint variants (not actuated) for Configurable
    if (!cid.includes("actuated")) return VARIANT_GROUPS.multibodyJoints;
  }
  for (const group of Object.values(VARIANT_GROUPS)) {
    if (group.some((g) => g.typeName === typeName || g.catalogId === catalogId)) return group;
  }
  return null;
}

/** Apply a catalog variant to a node, preserving overlapping params and edges-friendly ports. */
export function applyVariantToNode(
  node: import("../api/types").DiagramNode,
  next: { typeName: string; catalogId: string },
  entry: import("../api/types").CatalogEntry | undefined,
): import("../api/types").DiagramNode {
  const ports = entry ? entry.ports.map((p) => ({ ...p })) : node.ports;
  return {
    ...node,
    typeName: next.typeName,
    catalogId: next.catalogId,
    params: entry
      ? entry.defaultParams.map((p) => {
          const prev = node.params.find((x) => x.name === p.name);
          return { name: p.name, value: prev?.value ?? p.value };
        })
      : node.params,
    ports,
    plotHints: entry ? [...entry.plotHints] : node.plotHints,
    domain: entry?.domain ?? node.domain,
  };
}
