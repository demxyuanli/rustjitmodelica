import type { ExampleInfo } from "../api/types";
import type { MessageKey } from "../i18n";

const DOMAIN_ORDER = ["signal", "electrical", "mechanical", "multibody"] as const;

export type ExampleGroup = {
  domain: string;
  items: ExampleInfo[];
};

export function exampleDomainLabelKey(domain: string): MessageKey {
  switch (domain) {
    case "signal":
      return "exampleDomainSignal";
    case "electrical":
      return "exampleDomainElectrical";
    case "mechanical":
      return "exampleDomainMechanical";
    case "multibody":
      return "exampleDomainMultibody";
    default:
      return "exampleDomainOther";
  }
}

export function groupExamples(items: ExampleInfo[]): ExampleGroup[] {
  const buckets = new Map<string, ExampleInfo[]>();
  for (const ex of items) {
    const domain = ex.domain.trim() || "other";
    const list = buckets.get(domain);
    if (list) list.push(ex);
    else buckets.set(domain, [ex]);
  }
  const groups: ExampleGroup[] = [];
  for (const domain of DOMAIN_ORDER) {
    const list = buckets.get(domain);
    if (list?.length) groups.push({ domain, items: list });
    buckets.delete(domain);
  }
  for (const [domain, list] of buckets) {
    if (list.length) groups.push({ domain, items: list });
  }
  return groups;
}
