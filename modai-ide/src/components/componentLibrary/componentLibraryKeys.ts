import type { InstantiableClass } from "../../types";

export function componentKey(item: Pick<InstantiableClass, "qualifiedName" | "libraryId">) {
  return `${item.libraryId}::${item.qualifiedName}`;
}
