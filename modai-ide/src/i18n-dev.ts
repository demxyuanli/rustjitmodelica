import { getMessageCatalog } from "./i18n";

function diffKeys(baseKeys: string[], compareKeys: string[]) {
  const compareSet = new Set(compareKeys);
  return baseKeys.filter((key) => !compareSet.has(key));
}

export function warnOnI18nMismatch() {
  if (!import.meta.env.DEV) return;

  const catalogs = getMessageCatalog();
  const baseLocale = "en";
  const baseKeys = Object.keys(catalogs[baseLocale] ?? {}).sort();

  for (const [locale, table] of Object.entries(catalogs)) {
    if (locale === baseLocale) continue;

    const localeKeys = Object.keys(table ?? {}).sort();
    const missing = diffKeys(baseKeys, localeKeys);
    const extra = diffKeys(localeKeys, baseKeys);

    if (missing.length === 0 && extra.length === 0) continue;

    console.warn(`[i18n] locale "${locale}" is out of sync`, {
      missing,
      extra,
    });
  }
}
