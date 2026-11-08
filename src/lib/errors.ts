import type { TFunction } from "i18next";

const KNOWN_ERRORS = [
  "already_running",
  "service_running",
  "not_installed",
  "no_backup",
  "tests_already_running",
  "service_installed",
  "dpi_suite_unavailable",
  "zapret_required",
  "warp_not_installed",
  "autopilot_busy",
  "no_services",
  "no_configs",
];

export function errText(t: TFunction, e: unknown): string {
  const msg = String(e);
  if (KNOWN_ERRORS.includes(msg)) return t(`errors.${msg}`);
  if (msg.toLowerCase().includes("network")) return t("errors.network");
  return t("errors.generic", { message: msg });
}
