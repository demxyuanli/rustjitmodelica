import { messagesEn } from "./i18n/messagesEn";
import { messagesZh } from "./i18n/messagesZh";

const messages: Record<"en" | "zh", Record<string, string>> = {
  en: messagesEn as Record<string, string>,
  zh: messagesZh as Record<string, string>,
};

let lang: "en" | "zh" = "zh";

export function setLang(l: "en" | "zh") {
  lang = l;
}

export type I18nKey = keyof typeof messagesEn;

export function t(key: I18nKey): string {
  return messages[lang]?.[key] ?? messages.en[key] ?? key;
}

export function tf(key: I18nKey, values: Record<string, string | number>): string {
  return Object.entries(values).reduce(
    (acc, [name, value]) => acc.split(`{${name}}`).join(String(value)),
    t(key),
  );
}

export function getMessageCatalog() {
  return messages;
}
