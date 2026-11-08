import { logUi } from "./api";

/** Sends uncaught frontend errors to the app log so they show up in bug reports. */
export function installGlobalErrorLogging() {
  window.addEventListener("error", (e) => {
    const where = e.filename ? ` (${e.filename.split("/").pop()}:${e.lineno})` : "";
    logUi("error", `uncaught: ${e.message}${where}`);
  });
  window.addEventListener("unhandledrejection", (e) => {
    const reason = e.reason instanceof Error ? e.reason.message : String(e.reason);
    logUi("error", `unhandled rejection: ${reason}`);
  });
}
