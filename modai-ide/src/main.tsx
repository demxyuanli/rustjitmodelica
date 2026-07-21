import "./monacoWorkers";
import ReactDOM from "react-dom/client";
import { loader } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import App from "./App";
import { DiagramSchemeProvider } from "./contexts/DiagramSchemeContext";
import { warnOnI18nMismatch } from "./i18n-dev";

// Use bundled monaco-editor instead of jsDelivr CDN (avoids Tracking Prevention / storage warnings in WebView2).
loader.config({ monaco });

warnOnI18nMismatch();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  // JointJS Paper is imperative and not StrictMode-safe (mount/cleanup/remount
  // races have crashed WebView2 with exit 0xcfffffff while opening diagrams).
  <DiagramSchemeProvider>
    <App />
  </DiagramSchemeProvider>,
);
