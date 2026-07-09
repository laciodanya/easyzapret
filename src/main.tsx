import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyTheme } from "./lib/store";
import "./i18n";
import "./styles.css";

try {
  applyTheme(localStorage.getItem("easyzapret-theme") || "system");
} catch {
  /* first paint */
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
