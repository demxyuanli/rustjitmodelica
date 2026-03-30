import type { ComponentProps } from "react";
import { SimulationRunView } from "./SimulationRunView";

export type SimulationResultsTabProps = ComponentProps<typeof SimulationRunView>;

export function SimulationResultsTab(props: SimulationResultsTabProps) {
  return (
    <div className="flex min-h-0 min-w-0 flex-1">
      <SimulationRunView {...props} />
    </div>
  );
}
