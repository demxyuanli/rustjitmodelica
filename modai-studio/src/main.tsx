import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { setI18nLocale } from "./i18n";
import { applySettingsToDom, loadSettings } from "./settings/studioSettings";
import "./styles/app.css";
import "./styles/simulinkBlocks.css";

const boot = loadSettings();
setI18nLocale(boot.locale);
applySettingsToDom(boot);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
