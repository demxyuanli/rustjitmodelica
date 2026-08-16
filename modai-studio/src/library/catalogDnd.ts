export const CATALOG_DND_MIME = "application/x-modai-catalog";

export function setCatalogDragData(dt: DataTransfer, catalogId: string): void {
  dt.setData(CATALOG_DND_MIME, catalogId);
  dt.setData("text/plain", catalogId);
  dt.effectAllowed = "copy";
}

export function readCatalogDragId(dt: DataTransfer): string | null {
  const id = dt.getData(CATALOG_DND_MIME) || dt.getData("text/plain");
  return id.trim() ? id.trim() : null;
}
