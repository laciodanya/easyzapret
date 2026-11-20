export type VpnMode = "all" | "only_apps" | "bypass_apps" | "proxy";

export interface VpnRule {
  kind: string;
  values: string[];
  outbound: "proxy" | "direct" | "reject";
}

export interface VpnSettings {
  mode: VpnMode;
  apps: string[];
  bypassLan: boolean;
  dnsThroughVpn: boolean;
  dnsServer: string;
  mtu: number;
  stack: "system" | "gvisor" | "mixed";
  ipv6: boolean;
  localPort: number;
  sniff: boolean;
  rules: VpnRule[];
  autoConnect: boolean;
}

export interface VpnStatus {
  installed: boolean;
  running: boolean;
  mode: VpnMode;
  activeProfile: string | null;
  error: string | null;
}

export interface VpnProfile {
  id: string;
  name: string;
  protocol: string;
  server: string;
  subscriptionId: string | null;
}

export interface VpnDetails {
  status: VpnStatus;
  settings: VpnSettings;
  selectedProfile: string | null;
  profiles: VpnProfile[];
  subscriptions: { id: string; name: string; host: string; updatedAt: string }[];
}
