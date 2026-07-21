/** Compatible type switches for Variant-like reconfiguration (P3). */
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
  ],
};

export function variantOptionsFor(typeName: string) {
  for (const group of Object.values(VARIANT_GROUPS)) {
    if (group.some((g) => g.typeName === typeName)) return group;
  }
  return null;
}
