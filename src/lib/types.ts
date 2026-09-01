export interface AppInfo {
  version: string;
  dataDir: string;
  isAdmin: boolean;
  isWindows: boolean;
}

export interface AutopilotSettings {
  enabled: boolean;
  intervalMinutes: number;
  policy: "stability" | "speed" | "gaming";
  switchMode: "rotate_policy" | "rotate_custom" | "best_by_tests";
  allowedStrategies: string[];
  maxTestStrategies: number;
  autoSwitchStrategy: boolean;
  minHealthPercent: number;
  probeDiscord: boolean;
  probeYoutube: boolean;
  probeCloudflare: boolean;
  probeGoogle: boolean;
  autoEnableWarp: boolean;
  notifyOnSwitch: boolean;
  notifyOnDegraded: boolean;
  maxSwitchesPerHour: number;
  onlyWhenZapretRunning: boolean;
}

export interface AutostartSettings {
  launchAtLogin: boolean;
  autoStartZapret: boolean;
  autoStartWarp: boolean;
  autoStartTg: boolean;
  startMinimized: boolean;
}

export interface AutostartState {
  launchAtLogin: boolean;
  loginEntryPresent: boolean;
}

export interface Settings {
  language: "ru" | "en" | null;
  theme: "light" | "purple" | "system";
  selectedStrategy: string | null;
  zapretVersion: string | null;
  tgVersion: string | null;
  vpnCoreVersion: string | null;
  checkUpdatesOnStart: boolean;
  autopilot: AutopilotSettings;
  autostart: AutostartSettings;
  lastSeenChangelogVersion: string | null;
}

export interface AutopilotStatus {
  enabled: boolean;
  checking: boolean;
  lastCheckAt: string | null;
  lastHealthPercent: number | null;
  lastMessage: string | null;
  policy: string;
  switchMode: string;
  switchesThisHour: number;
}

export interface AutopilotEvent {
  kind: string;
  message: string;
  fromStrategy: string | null;
  toStrategy: string | null;
  healthPercent: number | null;
}

export interface ComponentsState {
  zapretInstalled: boolean;
  zapretVersion: string | null;
  tgInstalled: boolean;
  tgVersion: string | null;
  vpnCoreInstalled: boolean;
  vpnCoreVersion: string | null;
  dataDir: string;
}

export interface ZapretStatus {
  running: boolean;
  currentStrategy: string | null;
  serviceInstalled: boolean;
  serviceState: string | null;
  serviceStrategy: string | null;
  windivertState: string | null;
  windivertSysPresent: boolean;
}

export interface TgStatus {
  installed: boolean;
  running: boolean;
  host: string;
  port: number;
  secret: string | null;
  tgLink: string | null;
  shareLink: string | null;
}

export interface WarpStatus {
  installed: boolean;
  connected: boolean;
  mode: string | null;
  registered: boolean;
  detail: string | null;
}

export interface VpnStatus {
  coreInstalled: boolean;
  connected: boolean;
  mode: string;
  selectedNodeId: string | null;
  selectedNodeName: string | null;
  socksPort: number;
  httpPort: number;
  nodeCount: number;
  subscriptionCount: number;
}

export interface VpnUserInfo {
  upload?: number | null;
  download?: number | null;
  total?: number | null;
  expire?: number | null;
}

export interface VpnNode {
  id: string;
  name: string;
  protocol: string;
  address: string;
  port: number;
  raw: string;
  latencyMs: number | null;
  subscriptionId: string | null;
  params: unknown;
}

export interface VpnSubscription {
  id: string;
  url: string;
  name: string;
  updatedAt: string | null;
  userinfo: VpnUserInfo | null;
  announce: string | null;
  supportUrl: string | null;
  webPageUrl: string | null;
  updateIntervalHours: number | null;
  nodes: VpnNode[];
}

export interface VpnSettings {
  mode: string;
  socksPort: number;
  httpPort: number;
  muxEnabled: boolean;
  muxConcurrency: number;
  sniffing: boolean;
  allowInsecure: boolean;
  dns: string;
  bypassLan: boolean;
  bypassPrivate: boolean;
  fragmentation: boolean;
  fragmentationPackets: string;
  fragmentationLength: string;
  fragmentationInterval: string;
  autoConnect: boolean;
  autoconnectType: string;
  autoUpdateSubs: boolean;
  updateOnOpen: boolean;
  routingEnabled: boolean;
  selectStrategy: string;
  schema?: number;
}

export interface VpnState {
  subscriptions: VpnSubscription[];
  manualNodes: VpnNode[];
  selectedNodeId: string | null;
  settings: VpnSettings;
}

export interface VpnDetails {
  status: VpnStatus;
  state: VpnState;
}

export interface FullStatus {
  zapret: ZapretStatus;
  tg: TgStatus;
  warp: WarpStatus;
  vpn: VpnStatus;
  autopilot: AutopilotStatus;
  testsRunning: boolean;
}

export interface ServiceSettings {
  gameFilterMode: "off" | "all" | "tcp" | "udp";
  ipsetMode: "none" | "loaded" | "any";
  autoUpdateCheck: boolean;
}

export interface ActiveFakesStatus {
  files: string[];
  discordCurrent: string | null;
  gameCurrent: string | null;
}

export interface UpdateStatus {
  component: "zapret" | "tgproxy" | "vpncore";
  installed: string | null;
  latest: string | null;
  updateAvailable: boolean;
  releaseUrl: string | null;
  error: string | null;
}

export interface AppUpdateStatus {
  current: string;
  latest: string | null;
  updateAvailable: boolean;
  releaseUrl: string | null;
  error: string | null;
}

export interface DiagItem {
  id: string;
  status: "ok" | "warn" | "fail";
  detail: string;
}

export interface HostsCheck {
  upToDate: boolean;
  tempFile: string;
  hostsFile: string;
}

export interface UserListFile {
  name: string;
  content: string;
  exists: boolean;
}

export interface InstallProgress {
  component: string;
  phase: "downloading" | "extracting" | "done" | "error";
  downloaded: number;
  total: number;
}

export interface TestTargetResult {
  config: string;
  name: string;
  status: "ok" | "warn" | "fail" | "timeout" | "skipped";
  detail: string;
}

export type TestEvent =
  | { kind: "started"; mode: string; configs: string[]; targetsPerConfig: number }
  | { kind: "config-start"; config: string; index: number; total: number }
  | { kind: "target-result"; result: TestTargetResult }
  | { kind: "config-done"; config: string; ok: number; fail: number; error?: string }
  | { kind: "finished"; best: string | null; summary: { config: string; ok: number; fail: number }[] }
  | { kind: "cancelled" }
  | { kind: "error"; message: string };
