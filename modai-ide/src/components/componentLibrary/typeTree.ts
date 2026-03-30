import type { InstantiableClass } from "../../types";

export interface TypeTreeNode {
  segment: string;
  fullPath: string;
  children: Record<string, TypeTreeNode>;
  items: InstantiableClass[];
}

export function buildTypeTree(items: InstantiableClass[]): TypeTreeNode {
  const root: TypeTreeNode = { segment: "", fullPath: "", children: {}, items: [] };
  for (const item of items) {
    const parts = item.qualifiedName.split(".");
    let current = root;
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      const fullPath = parts.slice(0, i + 1).join(".");
      if (!current.children[part]) {
        current.children[part] = { segment: part, fullPath, children: {}, items: [] };
      }
      current = current.children[part];
    }
    current.items.push(item);
  }
  return root;
}

export function sortedChildEntries(node: TypeTreeNode): [string, TypeTreeNode][] {
  return Object.entries(node.children).sort(([a], [b]) => a.localeCompare(b, undefined, { sensitivity: "base" }));
}
