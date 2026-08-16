import { Ban, Languages } from "lucide-react";
import { t } from "../i18n";
import { IconButton } from "./IconButton";

export function StatusBar({
  status,
  projectLabel,
  busy,
  bgValidate,
  onCancelValidate,
  locale,
  onToggleLocale,
}: {
  status: string;
  projectLabel: string;
  busy: boolean;
  bgValidate?: boolean;
  onCancelValidate?: () => void;
  locale: "zh" | "en";
  onToggleLocale: () => void;
}) {
  return (
    <footer className="status-bar">
      <span className={`status-bar-item${busy || bgValidate ? " busy" : ""}`}>{status}</span>
      {bgValidate && onCancelValidate ? (
        <IconButton
          icon={Ban}
          labelKey="cancelValidate"
          className="status-bar-item"
          onClick={onCancelValidate}
        />
      ) : null}
      <span className="status-bar-item muted">{projectLabel}</span>
      <span className="status-bar-spacer" />
      <IconButton
        icon={Languages}
        labelKey="settingsLocale"
        title={locale === "zh" ? t("localeZh") : t("localeEn")}
        className="status-bar-item"
        onClick={onToggleLocale}
      />
    </footer>
  );
}
