/** Persist catalog favorites in localStorage (P355). */

const KEY = "modai-studio.catalogFavorites";

export function loadFavorites(): string[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((x): x is string => typeof x === "string");
  } catch {
    return [];
  }
}

export function saveFavorites(ids: string[]): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(ids));
  } catch {
    /* ignore quota */
  }
}

export function toggleFavorite(id: string, current: string[]): string[] {
  const set = new Set(current);
  if (set.has(id)) set.delete(id);
  else set.add(id);
  const next = [...set];
  saveFavorites(next);
  return next;
}
