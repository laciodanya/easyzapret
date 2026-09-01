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
  "warp_vpn_exclusive",
  "vpn_warp_exclusive",
  "vpn_core_not_installed",
  "vpn_no_node",
  "vpn_empty_subscription",
  "vpn_empty_url",
  "vpn_invalid_link",
  "vpn_tun_unavailable",
  "vpn_core_exited",
  "hysteria2_needs_singbox",
];

export function errText(t: TFunction, e: unknown): string {
  const msg = String(e);
  if (KNOWN_ERRORS.includes(msg)) return t(`errors.${msg}`);
  if (msg.startsWith("unsupported_protocol:")) {
    return t("errors.generic", { message: msg });
  }
  if (msg.toLowerCase().includes("network")) return t("errors.network");
  return t("errors.generic", { message: msg });
}
