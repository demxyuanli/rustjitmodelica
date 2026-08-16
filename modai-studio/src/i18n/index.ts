import { messagesEn, type MessageKey } from "./messagesEn";
import { messagesZh } from "./messagesZh";
import type { Locale } from "../settings/studioSettings";

const catalogs: Record<Locale, Record<MessageKey, string>> = {
  en: messagesEn,
  zh: messagesZh,
};

let locale: Locale = "zh";

export function setI18nLocale(l: Locale) {
  locale = l;
}

export function getI18nLocale(): Locale {
  return locale;
}

export function t(key: MessageKey): string {
  return catalogs[locale]?.[key] ?? catalogs.en[key] ?? key;
}

export function tf(key: MessageKey, values: Record<string, string | number>): string {
  let s = t(key);
  for (const [k, v] of Object.entries(values)) {
    s = s.split(`{${k}}`).join(String(v));
  }
  return s;
}

export type { MessageKey };
