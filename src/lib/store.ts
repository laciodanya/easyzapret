import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api } from "./api";
import i18n, { systemLanguage } from "../i18n";
import { toast } from "./toast";
import { getStatus } from "./status";
import type { TFunction } from "i18next";
import type {
  AppInfo,
  AppUpdateStatus,
  AutopilotEvent,
  AutopilotSettings,
  AutostartSettings,
  ComponentsState,
  FullStatus,
  Settings,
  UpdateStatus,
} from "./types";

export type Page =
  | "home"
  | "strategies"
  | "zapret"
  | "autopilot"
  | "telegram"
  | "warp"
  | "logs"
  | "settings";

export type ZapretTab = "service" | "tests" | "lists";

const THEME_KEY = "easyzapret-theme";

interface AppStore {
  page: Page;
  zapretTab: ZapretTab;
  ready: boolean;
  initError: boolean;
  appInfo: AppInfo | null;
  settings: Settings | null;
  components: ComponentsState | null;
  status: FullStatus | null;
  strategies: string[];
  updates: UpdateStatus[] | null;
  appUpdate: AppUpdateStatus | null;
  updatesCheckedAt: Date | null;
  updatesError: boolean;
  showSetup: boolean;
  showUpdatesModal: boolean;
  showWhatsNew: boolean;

  setPage: (page: Page) => void;
  setZapretTab: (tab: ZapretTab) => void;
  init: () => Promise<void>;
  refreshStatus: () => Promise<void>;
  refreshComponents: () => Promise<void>;
  refreshStrategies: () => Promise<void>;
  updateSettings: (patch: Partial<Settings>) => Promise<void>;
  updateAutostart: (patch: Partial<AutostartSettings>) => Promise<void>;
  checkUpdates: (opts?: { silent?: boolean }) => Promise<void>;
  dismissSetup: () => void;
  dismissUpdatesModal: () => void;
  dismissWhatsNew: () => Promise<void>;
}

function resolveTheme(theme: string): "light" | "purple" {
  if (theme === "light") return "light";
  if (theme === "purple" || theme === "dark") return "purple";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "purple" : "light";
}

export function applyTheme(theme: string) {
  const root = document.documentElement;
  root.classList.remove("dark", "theme-purple");
  try {
    localStorage.setItem(THEME_KEY, theme);
  } catch {
    /* private mode */
  }
  if (resolveTheme(theme) === "purple") {
    root.classList.add("dark", "theme-purple");
  }
}

window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
  const theme = useStore.getState().settings?.theme ?? "system";
  if (theme === "system") applyTheme("system");
});

function defaultAutopilot(): AutopilotSettings {
  return {
    enabled: false,
    intervalMinutes: 15,
    policy: "stability",
    switchMode: "rotate_policy",
    allowedStrategies: [],
    maxTestStrategies: 6,
    autoSwitchStrategy: true,
    minHealthPercent: 60,
    probeDiscord: true,
    probeYoutube: true,
    probeCloudflare: true,
    probeGoogle: false,
    autoEnableWarp: false,
    notifyOnSwitch: true,
    notifyOnDegraded: true,
    maxSwitchesPerHour: 3,
    onlyWhenZapretRunning: true,
  };
}

function defaultAutostart(): AutostartSettings {
  return {
    launchAtLogin: false,
    autoStartZapret: false,
    autoStartWarp: false,
    autoStartTg: false,
    startMinimized: false,
  };
}

function mergeSettings(raw: Settings): Settings {
  const autostart = { ...defaultAutostart(), ...raw.autostart };
  if (autostart.autoStartWarp) {
    autostart.autoStartZapret = true;
  }
  return {
    ...raw,
    autopilot: { ...defaultAutopilot(), ...raw.autopilot },
    autostart,
  };
}

export const useStore = create<AppStore>((set, get) => ({
  page: "home",
  zapretTab: "service",
  ready: false,
  initError: false,
  appInfo: null,
  settings: null,
  components: null,
  status: null,
  strategies: [],
  updates: null,
  appUpdate: null,
  updatesCheckedAt: null,
  updatesError: false,
  showSetup: false,
  showUpdatesModal: false,
  showWhatsNew: false,

  setPage: (page) => set({ page }),
  setZapretTab: (zapretTab) => set({ zapretTab, page: "zapret" }),

  init: async () => {
    try {
      const [appInfo, rawSettings, components] = await Promise.all([
        api.getAppInfo(),
        api.getSettings(),
        api.getComponentsState(),
      ]);
      const settings = mergeSettings(rawSettings);
      const lang = settings.language ?? systemLanguage();
      if (i18n.language !== lang) await i18n.changeLanguage(lang);
      applyTheme(settings.theme);

      const showWhatsNew = settings.lastSeenChangelogVersion !== appInfo.version;

      set({
        appInfo,
        settings,
        components,
        showSetup: !components.zapretInstalled || !components.tgInstalled,
        showWhatsNew,
        ready: true,
        initError: false,
      });

      void get().refreshStatus();
      void get().refreshStrategies();
      if (settings.checkUpdatesOnStart) {
        window.setTimeout(() => get().checkUpdates({ silent: true }).catch(() => {}), 4000);
      }
    } catch {
      set({ ready: true, initError: true });
    }
  },

  refreshStatus: async () => {
    try {
      const status = await getStatus();
      set({ status });
    } catch {
      /* backend busy */
    }
  },

  refreshComponents: async () => {
    const components = await api.getComponentsState();
    set({ components });
  },

  refreshStrategies: async () => {
    try {
      const strategies = await api.listStrategies();
      set({ strategies });
      const { settings } = get();
      if (settings && !settings.selectedStrategy && strategies.length > 0) {
        const general = strategies.find((s) => s.toLowerCase() === "general.bat");
        await get().updateSettings({ selectedStrategy: general ?? strategies[0] });
      }
    } catch {
      set({ strategies: [] });
    }
  },

  updateSettings: async (patch) => {
    const current = get().settings;
    if (!current) return;
    const settings = mergeSettings({ ...current, ...patch });
    set({ settings });
    await api.saveSettings(settings);
    if (patch.theme) applyTheme(patch.theme);
    if (patch.language !== undefined) {
      await i18n.changeLanguage(patch.language ?? systemLanguage());
      api.refreshTray().catch(() => {});
    }
  },

  updateAutostart: async (patch) => {
    const current = get().settings;
    if (!current) return;
    const next = { ...current.autostart, ...patch };
    if (next.autoStartWarp) next.autoStartZapret = true;
    const settings = mergeSettings({ ...current, autostart: next });
    set({ settings });
    if (patch.launchAtLogin !== undefined) {
      await api.setLaunchAtLogin(patch.launchAtLogin);
    }
    await api.saveSettings(settings);
  },

  checkUpdates: async (opts) => {
    const silent = opts?.silent ?? false;
    try {
      const [updates, appUpdate] = await Promise.all([
        api.checkUpdates(),
        api.checkAppUpdate().catch(() => null),
      ]);
      const hasErrors = updates.every((u) => u.error) && (!appUpdate || !!appUpdate.error);
      const anyAvailable =
        updates.some((u) => u.updateAvailable) || !!appUpdate?.updateAvailable;
      set({
        updates,
        appUpdate,
        updatesCheckedAt: new Date(),
        updatesError: hasErrors,
        ...(silent ? {} : { showUpdatesModal: anyAvailable }),
      });
    } catch {
      set({ updatesCheckedAt: new Date(), updatesError: true });
    }
  },

  dismissSetup: () => set({ showSetup: false }),
  dismissUpdatesModal: () => set({ showUpdatesModal: false }),

  dismissWhatsNew: async () => {
    const { appInfo } = get();
    if (appInfo) {
      await get().updateSettings({ lastSeenChangelogVersion: appInfo.version });
    }
    set({ showWhatsNew: false });
  },
}));

/** Autopilot toast notifications — wired once at app mount. */
export function setupAutopilotListener(t: TFunction) {
  let unlisten: UnlistenFn | null = null;
  listen<AutopilotEvent>("autopilot-event", (e) => {
    const ev = e.payload;
    if (ev.kind === "strategy_switched" && ev.fromStrategy && ev.toStrategy) {
      const from = ev.fromStrategy.replace(/\.bat$/i, "");
      const to = ev.toStrategy.replace(/\.bat$/i, "");
      toast(t("autopilot.toastSwitch", { from, to }), "info");
    } else if (ev.kind === "health_degraded") {
      toast(
        t("autopilot.toastDegraded", { percent: ev.healthPercent ?? 0 }),
        "fail",
      );
    }
  }).then((u) => {
    unlisten = u;
  });
  return () => {
    unlisten?.();
  };
}
