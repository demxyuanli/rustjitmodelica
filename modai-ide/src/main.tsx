import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { DiagramSchemeProvider } from "./contexts/DiagramSchemeContext";
import { warnOnI18nMismatch } from "./i18n-dev";

warnOnI18nMismatch();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <DiagramSchemeProvider>
      <App />
    </DiagramSchemeProvider>
  </React.StrictMode>,
);
