import { invoke } from "@tauri-apps/api/core";
import type {
  ActiveFakesStatus,
  AppInfo,
  AppUpdateStatus,
  AutopilotStatus,
  AutostartState,
  ComponentsState,
  DiagItem,
  FullStatus,
  HostsCheck,
  ServiceHealth,
  ServiceSettings,
  Settings,
  TestMode,
  TgStatus,
  UpdateStatus,
  UserListFile,
  WarpStatus,
  ZapretStatus,
} from "./types";

/** Commands that run on a timer or on every page visit — not worth a log line. */
const QUIET_COMMANDS = new Set([
  "log_ui",
  "get_status",
  "get_autopilot_status",
  "get_app_info",
  "get_settings",
  "get_components_state",
  "get_service_settings",
  "get_active_fakes",
  "get_autostart_state",
  "zapret_status",
  "tg_status",
  "warp_details",
  "list_strategies",
  "read_user_lists",
  "read_log",
  "logs_dir_path",
  "refresh_tray",
  "check_updates",
  "check_app_update",
  "save_settings",
  "save_user_list",
  "get_log_report",
]);

export type UiLogLevel = "info" | "warn" | "error";

/** Writes a line to the app log. Never throws — logging must not break the UI. */
export function logUi(level: UiLogLevel, message: string): void {
  invoke("log_ui", { level, message: message.slice(0, 600) }).catch(() => {});
}

function describeArgs(args?: Record<string, unknown>): string {
  if (!args) return "";
  const text = JSON.stringify(args);
  return text.length > 160 ? ` ${text.slice(0, 160)}…` : ` ${text}`;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const quiet = QUIET_COMMANDS.has(command);
  if (!quiet) logUi("info", `action: ${command}${describeArgs(args)}`);
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    if (command !== "log_ui") logUi("error", `${command} failed: ${String(e)}`);
    throw e;
  }
}

export const api = {
  // core
  getAppInfo: () => call<AppInfo>("get_app_info"),
  getStatus: () => call<FullStatus>("get_status"),
  refreshTray: () => call<void>("refresh_tray"),
  getSettings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<void>("save_settings", { settings }),

  // components / updates
  getComponentsState: () => call<ComponentsState>("get_components_state"),
  installComponent: (component: "zapret" | "tgproxy") =>
    call<string>("install_component", { component }),
  checkUpdates: () => call<UpdateStatus[]>("check_updates"),
  checkAppUpdate: () => call<AppUpdateStatus>("check_app_update"),

  // zapret
  listStrategies: () => call<string[]>("list_strategies"),
  startZapret: (strategyFile: string) => call<void>("start_zapret", { strategyFile }),
  stopZapret: () => call<void>("stop_zapret"),
  zapretStatus: () => call<ZapretStatus>("zapret_status"),
  installZapretService: (strategyFile: string) =>
    call<void>("install_zapret_service", { strategyFile }),
  removeZapretServices: () => call<void>("remove_zapret_services"),
  getServiceSettings: () => call<ServiceSettings>("get_service_settings"),
  getActiveFakes: () => call<ActiveFakesStatus>("get_active_fakes"),
  setActiveFake: (kind: "discord" | "game", file: string) =>
    call<ActiveFakesStatus>("set_active_fake", { kind, file }),
  setGameFilter: (mode: string) => call<void>("set_game_filter", { mode }),
  setIpsetMode: (mode: string) => call<void>("set_ipset_mode", { mode }),
  setAutoUpdateCheck: (enabled: boolean) => call<void>("set_auto_update_check", { enabled }),
  updateIpsetList: () => call<void>("update_ipset_list"),
  checkHostsFile: () => call<HostsCheck>("check_hosts_file"),
  runDiagnostics: () => call<DiagItem[]>("run_diagnostics"),
  removeConflictingServices: () => call<string[]>("remove_conflicting_services"),
  clearDiscordCache: () => call<string[]>("clear_discord_cache"),

  // service reachability
  checkServices: (ids: string[]) => call<ServiceHealth[]>("check_services", { ids }),

  // tests
  runTests: (mode: TestMode, configs: string[], services: string[] = []) =>
    call<void>("run_tests", { mode, configs, services }),
  cancelTests: () => call<void>("cancel_tests"),

  // tg proxy
  tgStatus: () => call<TgStatus>("tg_status"),
  startTg: () => call<void>("start_tg"),
  stopTg: () => call<void>("stop_tg"),

  // cloudflare warp
  warpDetails: () => call<WarpStatus>("warp_details"),
  warpConnect: () => call<void>("warp_connect"),
  warpDisconnect: () => call<void>("warp_disconnect"),
  warpResetKeys: () => call<void>("warp_reset_keys"),
  /** Resolves with the mode warp-cli actually reports after the change. */
  warpSetMode: (mode: "warp" | "doh" | "warp+doh") => call<string>("warp_set_mode", { mode }),

  // autopilot
  getAutopilotStatus: () => call<AutopilotStatus>("get_autopilot_status"),
  runAutopilotCheckNow: () => call<AutopilotStatus>("run_autopilot_check_now"),

  // autostart
  getAutostartState: () => call<AutostartState>("get_autostart_state"),
  setLaunchAtLogin: (enabled: boolean) =>
    call<AutostartState>("set_launch_at_login", { enabled }),

  // lists
  readUserLists: () => call<{ files: UserListFile[] }>("read_user_lists"),
  saveUserList: (name: string, content: string) =>
    call<void>("save_user_list", { name, content }),

  // logs
  readLog: (name: string, maxLines = 600) => call<string>("read_log", { name, maxLines }),
  clearLog: (name: string) => call<void>("clear_log", { name }),
  logsDirPath: () => call<string>("logs_dir_path"),
  getLogReport: (linesPerLog = 250) =>
    call<string>("get_log_report", { linesPerLog }),
  saveLogReport: () => call<string>("save_log_report"),
};
