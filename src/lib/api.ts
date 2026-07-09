import { invoke } from "@tauri-apps/api/core";
import type {
  AutopilotStatus,
  AppInfo,
  AppUpdateStatus,
  ComponentsState,
  DiagItem,
  FullStatus,
  HostsCheck,
  ServiceSettings,
  Settings,
  TgStatus,
  UpdateStatus,
  UserListFile,
  WarpStatus,
  ZapretStatus,
} from "./types";

export const api = {
  // core
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  getStatus: () => invoke<FullStatus>("get_status"),
  refreshTray: () => invoke<void>("refresh_tray"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),

  // components / updates
  getComponentsState: () => invoke<ComponentsState>("get_components_state"),
  installComponent: (component: "zapret" | "tgproxy") =>
    invoke<string>("install_component", { component }),
  checkUpdates: () => invoke<UpdateStatus[]>("check_updates"),
  checkAppUpdate: () => invoke<AppUpdateStatus>("check_app_update"),

  // zapret
  listStrategies: () => invoke<string[]>("list_strategies"),
  startZapret: (strategyFile: string) => invoke<void>("start_zapret", { strategyFile }),
  stopZapret: () => invoke<void>("stop_zapret"),
  zapretStatus: () => invoke<ZapretStatus>("zapret_status"),
  installZapretService: (strategyFile: string) =>
    invoke<void>("install_zapret_service", { strategyFile }),
  removeZapretServices: () => invoke<void>("remove_zapret_services"),
  getServiceSettings: () => invoke<ServiceSettings>("get_service_settings"),
  setGameFilter: (mode: string) => invoke<void>("set_game_filter", { mode }),
  setIpsetMode: (mode: string) => invoke<void>("set_ipset_mode", { mode }),
  setAutoUpdateCheck: (enabled: boolean) => invoke<void>("set_auto_update_check", { enabled }),
  updateIpsetList: () => invoke<void>("update_ipset_list"),
  checkHostsFile: () => invoke<HostsCheck>("check_hosts_file"),
  runDiagnostics: () => invoke<DiagItem[]>("run_diagnostics"),
  removeConflictingServices: () => invoke<string[]>("remove_conflicting_services"),
  clearDiscordCache: () => invoke<string[]>("clear_discord_cache"),

  // tests
  runTests: (mode: "standard" | "dpi", configs: string[]) =>
    invoke<void>("run_tests", { mode, configs }),
  cancelTests: () => invoke<void>("cancel_tests"),

  // tg proxy
  tgStatus: () => invoke<TgStatus>("tg_status"),
  startTg: () => invoke<void>("start_tg"),
  stopTg: () => invoke<void>("stop_tg"),

  // cloudflare warp
  warpDetails: () => invoke<WarpStatus>("warp_details"),
  warpConnect: () => invoke<void>("warp_connect"),
  warpDisconnect: () => invoke<void>("warp_disconnect"),
  warpResetKeys: () => invoke<void>("warp_reset_keys"),
  warpSetMode: (mode: "warp" | "doh" | "warp+doh") =>
    invoke<void>("warp_set_mode", { mode }),

  getAutopilotStatus: () => invoke<AutopilotStatus>("get_autopilot_status"),
  runAutopilotCheckNow: () => invoke<AutopilotStatus>("run_autopilot_check_now"),

  getAutostartState: () => invoke<import("./types").AutostartState>("get_autostart_state"),
  setLaunchAtLogin: (enabled: boolean) =>
    invoke<import("./types").AutostartState>("set_launch_at_login", { enabled }),

  // lists
  readUserLists: () => invoke<{ files: UserListFile[] }>("read_user_lists"),
  saveUserList: (name: string, content: string) =>
    invoke<void>("save_user_list", { name, content }),

  // logs
  readLog: (name: string, maxLines = 500) => invoke<string>("read_log", { name, maxLines }),
  clearLog: (name: string) => invoke<void>("clear_log", { name }),
  logsDirPath: () => invoke<string>("logs_dir_path"),
};
