/**
 * Development-only stand-in for the Tauri backend, so the interface can be
 * opened in a regular browser (`npm run dev`) for design and layout work.
 * It is only called when `import.meta.env.DEV` is true and is dropped from
 * production builds.
 */

type Handler = (args: Record<string, unknown>) => unknown;

type MockWindow = Window & { __TAURI_INTERNALS__?: unknown };

const STRATEGIES = [
  "general.bat",
  "general (ALT).bat",
  "general (ALT2).bat",
  "general (ALT3).bat",
  "general (ALT4).bat",
  "general (ALT5).bat",
  "general (FAKE TLS AUTO).bat",
  "general (FAKE TLS AUTO ALT).bat",
  "general (SIMPLE FAKE).bat",
  "general (SIMPLE FAKE ALT).bat",
];

const state = {
  zapretRunning: true,
  tgRunning: false,
  warpConnected: false,
  warpMode: "warp",
  autopilotEnabled: true,
  testsRunning: false,
  settings: {
    language: "ru",
    theme: "system",
    selectedStrategy: "general (ALT5).bat",
    zapretVersion: "1.9.7",
    tgVersion: "1.4.2",
    checkUpdatesOnStart: true,
    autopilot: {
      enabled: true,
      intervalMinutes: 15,
      policy: "stability",
      switchMode: "rotate_policy",
      allowedStrategies: [] as string[],
      maxTestStrategies: 6,
      autoSwitchStrategy: true,
      minHealthPercent: 60,
      probeDiscord: true,
      probeYoutube: true,
      probeCloudflare: true,
      probeGoogle: false,
      probeTelegram: true,
      deepCheck: true,
      autoEnableWarp: false,
      notifyOnSwitch: true,
      notifyOnDegraded: true,
      maxSwitchesPerHour: 3,
      onlyWhenZapretRunning: true,
    },
    autostart: {
      launchAtLogin: true,
      autoStartZapret: false,
      autoStartWarp: false,
      autoStartTg: false,
      startMinimized: false,
    },
    lastSeenChangelogVersion: "0.6.0",
  } as Record<string, unknown>,
};

const serviceHealth = (id: string, ok: number, total: number, avgMs: number | null, failed: string[] = []) => ({
  id,
  ok,
  total,
  avgMs,
  bulkBlocked: false,
  failed,
  state: ok === total ? "ok" : ok === 0 ? "down" : "partial",
});

function logText(name: string): string {
  const lines: Record<string, string[]> = {
    app: [
      "[2026-11-08 12:00:01] INFO  ==================== EasyZapret session ====================",
      "[2026-11-08 12:00:01] INFO  EasyZapret 0.6.0 · admin: yes",
      "[2026-11-08 12:00:02] INFO  system: Microsoft Windows [Version 10.0.26100.1]",
      "[2026-11-08 12:00:04] INFO  [ui] action: start_zapret {\"strategyFile\":\"general (ALT5).bat\"}",
      "[2026-11-08 12:00:09] WARN  winws.exe took 4.8s to initialise",
      "[2026-11-08 12:07:31] ERROR [ui] warp_connect failed: warp_not_installed",
      "[2026-11-08 12:07:40] INFO  settings saved: theme system → dark",
    ],
    autopilot: [
      "[2026-11-08 12:15:00] INFO  check: discord 5/5 · youtube 6/6 · cloudflare 3/3 → 100%",
      "[2026-11-08 12:30:00] WARN  check: youtube 2/6 → 66% (confirming in 15s)",
      "[2026-11-08 12:30:17] INFO  confirmed: youtube recovered on second check",
    ],
  };
  return (lines[name] ?? ["[2026-11-08 12:00:05] INFO  winws started"]).join("\n");
}

const handlers: Record<string, Handler> = {
  get_app_info: () => ({ version: "0.6.0", dataDir: "C:\\EasyZapret", isAdmin: true, isWindows: true }),
  get_settings: () => state.settings,
  save_settings: (a) => {
    state.settings = a.settings as Record<string, unknown>;
  },
  get_components_state: () => ({
    zapretInstalled: true,
    zapretVersion: "1.9.7",
    tgInstalled: true,
    tgVersion: "1.4.2",
    dataDir: "C:\\EasyZapret",
  }),
  list_strategies: () => STRATEGIES,
  get_status: () => ({
    zapret: {
      running: state.zapretRunning,
      currentStrategy: state.zapretRunning ? state.settings.selectedStrategy : null,
      serviceInstalled: false,
      serviceState: null,
      serviceStrategy: null,
      windivertState: state.zapretRunning ? "RUNNING" : null,
      windivertSysPresent: true,
    },
    tg: {
      installed: true,
      running: state.tgRunning,
      host: "127.0.0.1",
      port: 1443,
      secret: "dd3f9c1a7be04d52a86c10f4e9d7b2a5c1",
      tgLink: "tg://proxy?server=127.0.0.1&port=1443&secret=dd3f9c1a7be04d52a86c10f4e9d7b2a5c1",
      shareLink: "https://t.me/proxy?server=127.0.0.1&port=1443&secret=dd3f9c1a7be04d52a86c10f4e9d7b2a5c1",
    },
    warp: {
      installed: true,
      connected: state.warpConnected,
      mode: state.warpMode,
      registered: true,
      detail: null,
    },
    autopilot: handlers.get_autopilot_status({}),
    testsRunning: state.testsRunning,
  }),
  get_autopilot_status: () => ({
    enabled: state.autopilotEnabled,
    checking: false,
    switching: false,
    lastCheckAt: "2026-11-08 12:30:17",
    lastHealthPercent: 92,
    lastMessage: "YouTube: 5 из 6 адресов отвечают, Discord и Cloudflare в порядке.",
    policy: "stability",
    switchMode: "rotate_policy",
    switchesThisHour: 1,
    services: [
      serviceHealth("discord", 5, 5, 84),
      serviceHealth("youtube", 5, 6, 132, ["i.ytimg.com"]),
      serviceHealth("telegram", 3, 3, 71),
      serviceHealth("cloudflare", 3, 3, 49),
    ],
    internetOk: true,
    nextCheckAt: "12:45",
    monitoringOnly: false,
    history: [
      { at: "12:30:17", kind: "recovered", message: "YouTube снова отвечает на повторной проверке — стратегия не менялась." },
      { at: "12:30:02", kind: "flaky", message: "YouTube: 2 из 6 адресов. Жду 15 секунд и проверю ещё раз." },
      { at: "12:15:01", kind: "check_ok", message: "Всё в порядке: Discord 5/5, YouTube 6/6, Cloudflare 3/3." },
      { at: "11:42:10", kind: "switch", message: "Discord не отвечал на двух проверках. Сменил general (ALT3) → general (ALT5), проверка прошла." },
      { at: "11:41:55", kind: "degraded", message: "Discord: 0 из 5 адресов, доступность 45%." },
    ],
  }),
  run_autopilot_check_now: () => handlers.get_autopilot_status({}),
  start_zapret: () => {
    state.zapretRunning = true;
  },
  stop_zapret: () => {
    state.zapretRunning = false;
  },
  zapret_status: () => ({}),
  get_service_settings: () => ({ gameFilterMode: "off", ipsetMode: "loaded", autoUpdateCheck: false }),
  get_active_fakes: () => ({
    files: ["quic_initial_www_google_com.bin", "stun.bin", "tls_clienthello_www_google_com.bin"],
    discordCurrent: "stun.bin",
    gameCurrent: "quic_initial_www_google_com.bin",
  }),
  check_hosts_file: () => ({ upToDate: true, tempFile: "", hostsFile: "" }),
  run_diagnostics: () => [
    { id: "bfe", status: "ok", detail: "" },
    { id: "proxy", status: "ok", detail: "" },
    { id: "tcp_timestamps", status: "warn", detail: "TCP timestamps выключены — включаю через netsh." },
    { id: "adguard", status: "ok", detail: "" },
    { id: "vpn", status: "fail", detail: "Найдены службы: WireGuardTunnel$office" },
    { id: "secure_dns", status: "ok", detail: "Cloudflare (DoH)" },
    { id: "conflicting_bypasses", status: "ok", detail: "" },
  ],
  clear_discord_cache: () => ["Discord", "Discord Canary"],
  check_services: (a) =>
    (a.ids as string[]).map((id) =>
      id === "telegram"
        ? { ...serviceHealth(id, 2, 3, 96, ["web.telegram.org"]), state: "partial" }
        : serviceHealth(id, id === "youtube" ? 6 : 5, id === "youtube" ? 6 : 5, 60 + id.length * 7),
    ),
  tg_status: () => (handlers.get_status({}) as { tg: unknown }).tg,
  start_tg: () => {
    state.tgRunning = true;
  },
  stop_tg: () => {
    state.tgRunning = false;
  },
  warp_details: () => ({
    installed: true,
    connected: state.warpConnected,
    mode: state.warpMode,
    registered: true,
    detail: null,
  }),
  warp_connect: () => {
    state.warpConnected = true;
  },
  warp_disconnect: () => {
    state.warpConnected = false;
  },
  warp_set_mode: (a) => {
    state.warpMode = a.mode as string;
    return state.warpMode;
  },
  warp_reset_keys: () => undefined,
  check_updates: () => [
    { component: "zapret", installed: "1.9.7", latest: "1.9.7", updateAvailable: false, releaseUrl: null, error: null },
    { component: "tgproxy", installed: "1.4.2", latest: "1.4.2", updateAvailable: false, releaseUrl: null, error: null },
  ],
  check_app_update: () => {
    // Open the preview with ?update to see the "update available" state.
    const available = new URLSearchParams(window.location.search).has("update");
    return {
      current: "0.6.0",
      latest: available ? "0.6.1" : "0.6.0",
      updateAvailable: available,
      releaseUrl: null,
      error: null,
    };
  },
  get_autostart_state: () => ({ launchAtLogin: true, loginEntryPresent: true }),
  read_user_lists: () => ({
    files: [
      { name: "list-general-user.txt", content: "example.org\nsubdomain.example.com\n", exists: true },
      { name: "list-exclude-user.txt", content: "", exists: false },
      { name: "ipset-exclude-user.txt", content: "203.0.113.0/24\n", exists: true },
    ],
  }),
  read_log: (a) => logText(String(a.name)),
  logs_dir_path: () => "C:\\EasyZapret\\logs",
  get_log_report: () =>
    "# EasyZapret report\n\n- Version: 0.6.0\n- System: Windows 11 (26100)\n- Admin: yes\n\n## Logs: app\n\n```\n" +
    logText("app") +
    "\n```\n",
  save_log_report: () => "C:\\EasyZapret\\logs\\easyzapret-report-20261108-123000.md",
};

const listeners = new Map<string, Set<number>>();
const callbacks = new Map<number, (event: unknown) => void>();
let nextCallbackId = 1;

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((id) => callbacks.get(id)?.({ event, id, payload }));
}

function simulateServiceTests(configs: string[], services: string[]) {
  state.testsRunning = true;
  emit("test-event", {
    kind: "started",
    mode: "service",
    configs,
    services,
    targetsPerConfig: services.length * 3,
  });
  let delay = 300;
  configs.forEach((config, index) => {
    window.setTimeout(() => emit("test-event", { kind: "config-start", config, index, total: configs.length }), delay);
    services.forEach((group) => {
      for (let n = 0; n < 3; n += 1) {
        delay += 160;
        const ok = (index + group.length + n) % 5 !== 0;
        window.setTimeout(
          () =>
            emit("test-event", {
              kind: "target-result",
              result: {
                config,
                group,
                name: `${group}-${n + 1}.example`,
                status: ok ? "ok" : "fail",
                detail: ok ? "HTTP 200 · 84 ms" : "timeout after 5000 ms",
              },
            }),
          delay,
        );
      }
    });
    delay += 200;
    window.setTimeout(() => emit("test-event", { kind: "config-done", config, ok: 0, fail: 0 }), delay);
  });
  window.setTimeout(() => {
    state.testsRunning = false;
    emit("test-event", { kind: "finished", best: configs[0] ?? null, summary: [] });
  }, delay + 300);
}

export function installDevMock() {
  const win = window as MockWindow;
  if (win.__TAURI_INTERNALS__) return;

  Object.assign(win, {
    __TAURI_EVENT_PLUGIN_INTERNALS__: {
      unregisterListener: (event: string, id: number) => listeners.get(event)?.delete(id),
    },
  });

  win.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback: (callback: (event: unknown) => void) => {
      const id = nextCallbackId++;
      callbacks.set(id, callback);
      return id;
    },
    unregisterCallback: (id: number) => callbacks.delete(id),
    invoke: async (command: string, args: Record<string, unknown> = {}) => {
      if (command === "plugin:event|listen") {
        const event = String(args.event);
        const id = Number(args.handler);
        if (!listeners.has(event)) listeners.set(event, new Set());
        listeners.get(event)!.add(id);
        return id;
      }
      if (command.startsWith("plugin:")) {
        if (command === "plugin:window|is_maximized") return false;
        return null;
      }
      if (command === "run_tests") {
        simulateServiceTests(args.configs as string[], (args.services as string[]) ?? []);
        return null;
      }
      const handler = handlers[command];
      if (!handler) return null;
      return handler(args);
    },
  };
}
