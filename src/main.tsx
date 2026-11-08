import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { installDevMock } from "./dev/mockTauri";
import { installGlobalErrorLogging } from "./lib/log";
import { applyTheme } from "./lib/store";
import "./i18n";
import "./styles.css";

// Lets the UI run in a plain browser (no Tauri backend) for design work.
if (import.meta.env.DEV) installDevMock();

try {
  applyTheme(localStorage.getItem("easyzapret-theme") || "system");
} catch {
  /* first paint */
}

installGlobalErrorLogging();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);
