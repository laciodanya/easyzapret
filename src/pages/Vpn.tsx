import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore, type VpnTab } from "../lib/store";
import { waitForStatus } from "../lib/status";
import { countryFromAddress, countryFromName, displayServerName } from "../lib/serverMeta";
import { FlagMark } from "../components/FlagMark";
import type { VpnDetails, VpnNode, VpnSettings, VpnSubscription } from "../lib/types";
import {
  Badge,
  Button,
  Card,
  FieldRow,
  Modal,
  Note,
  PageHeader,
  Segmented,
  Spinner,
  Switch,
  cn,
} from "../components/ui";

function formatBytes(n?: number | null): string {
  if (n == null || n <= 0) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

function formatExpire(ts?: number | null): string {
  if (!ts) return "—";
  try {
    return new Date(ts * 1000).toLocaleString();
  } catch {
    return "—";
  }
}

function allNodes(details: VpnDetails | null): VpnNode[] {
  if (!details) return [];
  const out = [...details.state.manualNodes];
  for (const s of details.state.subscriptions) out.push(...s.nodes);
  return out;
}

function paramStr(node: VpnNode, key: string): string | null {
  if (!node.params || typeof node.params !== "object") return null;
  const v = (node.params as Record<string, unknown>)[key];
  return typeof v === "string" && v ? v : null;
}

function transportLabel(node: VpnNode): string | null {
  const type = (paramStr(node, "type") || paramStr(node, "net") || "tcp").toLowerCase();
  const security = (paramStr(node, "security") || "").toLowerCase();
  const bits = [type === "tcp" ? null : type.toUpperCase(), security && security !== "none" ? security.toUpperCase() : null].filter(
    Boolean,
  );
  return bits.length ? bits.join(" + ") : null;
}

export function VpnPage() {
  const { t } = useTranslation();
  const { status, refreshStatus, vpnTab, setVpnTab, components, refreshComponents } = useStore();
  const [details, setDetails] = useState<VpnDetails | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [subUrl, setSubUrl] = useState("");
  const [nodeLink, setNodeLink] = useState("");
  const [addOpen, setAddOpen] = useState<"sub" | "node" | null>(null);
  const [settingsDraft, setSettingsDraft] = useState<VpnSettings | null>(null);

  const vpn = status?.vpn;
  const connected = vpn?.connected ?? details?.status.connected ?? false;
  const coreInstalled = components?.vpnCoreInstalled ?? vpn?.coreInstalled ?? false;
  const warpOn = !!status?.warp.connected;

  const load = useCallback(async () => {
    try {
      const d = await api.vpnDetails();
      setDetails(d);
      setSettingsDraft(d.state.settings);
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load, status?.vpn.connected, status?.vpn.selectedNodeId]);

  const nodes = useMemo(() => allNodes(details), [details]);
  const selectedId = details?.state.selectedNodeId ?? vpn?.selectedNodeId ?? null;

  const tabs: { value: VpnTab; label: string }[] = [
    { value: "connection", label: t("vpn.tabs.connection") },
    { value: "subscriptions", label: t("vpn.tabs.subscriptions") },
    { value: "settings", label: t("vpn.tabs.settings") },
  ];

  async function installCore() {
    setBusy("install");
    try {
      await api.installComponent("vpncore");
      await refreshComponents();
      await load();
      toast(t("vpn.coreInstalled"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setBusy(null);
    }
  }

  async function toggleConnect(on: boolean) {
    if (on && warpOn) {
      toast(t("errors.vpn_warp_exclusive"), "fail");
      return;
    }
    if (on && !coreInstalled) {
      toast(t("errors.vpn_core_not_installed"), "fail");
      return;
    }
    setBusy(on ? "connect" : "disconnect");
    try {
      if (on) await api.vpnConnect(selectedId);
      else await api.vpnDisconnect();
      await waitForStatus(
        (next) => next.vpn.connected === on,
        (next) => useStore.setState({ status: next }),
        25_000,
      );
      await load();
      await refreshStatus();
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setBusy(null);
    }
  }

  async function selectAndMaybeReconnect(id: string) {
    try {
      await api.vpnSelectNode(id);
      await load();
      if (connected) {
        setBusy("connect");
        await api.vpnConnect(id);
        await refreshStatus();
        await load();
      }
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setBusy(null);
    }
  }

  async function pingAll() {
    setBusy("ping");
    try {
      await api.vpnPingNodes([]);
      await load();
      toast(t("vpn.pingDone"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setBusy(null);
    }
  }

  async function addSubscription() {
    setBusy("add-sub");
    try {
      await api.vpnAddSubscription(subUrl.trim());
      setSubUrl("");
      setAddOpen(null);
      await load();
      toast(t("vpn.subAdded"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setBusy(null);
    }
  }

  async function addNode() {
    setBusy("add-node");
    try {
      await api.vpnAddNode(nodeLink.trim());
      setNodeLink("");
      setAddOpen(null);
      await load();
      toast(t("vpn.nodeAdded"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setBusy(null);
    }
  }

  async function updateSub(id: string) {
    setBusy(`upd-${id}`);
    try {
      await api.vpnUpdateSubscription(id);
      await load();
      toast(t("vpn.subUpdated"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setBusy(null);
    }
  }

  async function removeSub(id: string) {
    setBusy(`rm-${id}`);
    try {
      await api.vpnRemoveSubscription(id);
      await load();
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setBusy(null);
    }
  }

  async function removeNode(id: string) {
    try {
      await api.vpnRemoveNode(id);
      await load();
    } catch (e) {
      toast(errText(t, e), "fail");
    }
  }

  async function saveSettings() {
    if (!settingsDraft) return;
    setBusy("settings");
    try {
      const next = await api.vpnSaveSettings(settingsDraft);
      setSettingsDraft(next);
      await load();
      toast(t("common.saved"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setBusy(null);
    }
  }

  function patchSettings(patch: Partial<VpnSettings>) {
    setSettingsDraft((prev) => (prev ? { ...prev, ...patch } : prev));
  }

  return (
    <div className="ez-fade-up mx-auto flex h-full max-w-4xl flex-col">
      <PageHeader title={t("vpn.title")} description={t("vpn.description")} />
      <div className="mb-5">
        <Segmented value={vpnTab} options={tabs} onChange={setVpnTab} />
      </div>

      {!coreInstalled && (
        <Card className="mb-5">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <div className="text-sm font-semibold text-[rgb(var(--text))]">{t("vpn.needCoreTitle")}</div>
              <p className="mt-1 text-sm text-[rgb(var(--text-secondary))]">{t("vpn.needCoreDesc")}</p>
            </div>
            <Button variant="primary" disabled={busy !== null} onClick={() => void installCore()}>
              {busy === "install" ? <Spinner /> : null}
              {t("vpn.installCore")}
            </Button>
          </div>
        </Card>
      )}

      {warpOn && (
        <div className="mb-5">
          <Note tone="warn">{t("vpn.warpExclusive")}</Note>
        </div>
      )}

      {vpnTab === "connection" && (
        <div className="space-y-5">
          <Card>
            <div className="flex flex-col gap-5 sm:flex-row sm:items-center sm:justify-between">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <h3 className="text-lg font-bold tracking-tight text-[rgb(var(--text))]">
                    {connected ? t("vpn.connected") : t("vpn.disconnected")}
                  </h3>
                  <Badge tone={connected ? "ok" : "off"}>
                    {connected ? t("common.on") : t("common.off")}
                  </Badge>
                </div>
                <p className="mt-1 truncate text-sm text-[rgb(var(--text-secondary))]">
                  {displayServerName(vpn?.selectedNodeName || details?.status.selectedNodeName || "") || t("vpn.noServer")}
                </p>
                <p className="mt-1 text-xs text-[rgb(var(--muted))]">
                  {t("vpn.proxyPorts", {
                    http: vpn?.httpPort ?? 10809,
                    socks: vpn?.socksPort ?? 10808,
                  })}
                </p>
              </div>
              <div className="flex items-center gap-3">
                <Switch
                  checked={connected}
                  disabled={busy !== null || !coreInstalled || (warpOn && !connected)}
                  onChange={(v) => void toggleConnect(v)}
                />
                {(busy === "connect" || busy === "disconnect") && <Spinner />}
              </div>
            </div>
            <Note tone="info">{t("vpn.zapretCompatible")}</Note>
          </Card>

          <Card>
            <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
              <h3 className="text-sm font-bold uppercase tracking-wide text-slate-400">
                {t("vpn.servers")} ({nodes.length})
              </h3>
              <div className="flex flex-wrap gap-2">
                <Button variant="secondary" disabled={busy !== null || nodes.length === 0} onClick={() => void pingAll()}>
                  {busy === "ping" ? <Spinner /> : null}
                  {t("vpn.pingAll")}
                </Button>
                <Button variant="secondary" onClick={() => setAddOpen("node")}>
                  {t("vpn.addNode")}
                </Button>
              </div>
            </div>
            {nodes.length === 0 ? (
              <p className="py-8 text-center text-sm text-[rgb(var(--text-secondary))]">{t("vpn.emptyServers")}</p>
            ) : (
              <ul className="divide-y divide-[rgb(var(--border)/0.45)]">
                {nodes.map((n) => {
                  const active = selectedId === n.id;
                  const cc = countryFromName(n.name) || countryFromAddress(n.address);
                  const title = displayServerName(n.name) || n.address;
                  const transport = transportLabel(n);
                  return (
                    <li key={n.id} className="flex items-center gap-2">
                      <button
                        type="button"
                        onClick={() => void selectAndMaybeReconnect(n.id)}
                        className={cn(
                          "flex min-w-0 flex-1 items-center gap-3 rounded-xl px-2 py-2.5 text-left transition-colors",
                          active ? "bg-accent-soft/70 ring-1 ring-[rgb(var(--accent)/0.28)]" : "hover:bg-[rgb(var(--accent)/0.06)]",
                        )}
                      >
                        {cc ? (
                          <FlagMark code={cc} />
                        ) : (
                          <span className="flex h-5 w-7 shrink-0 items-center justify-center rounded-[4px] bg-[rgb(var(--accent)/0.18)] text-[9px] font-bold text-accent">
                            {n.protocol.slice(0, 2).toUpperCase()}
                          </span>
                        )}
                        <div className="min-w-0 flex-1">
                          <div className="truncate text-sm font-semibold text-[rgb(var(--text))]">{title}</div>
                          <div className="truncate text-xs text-[rgb(var(--text-secondary))]">
                            {n.protocol.toUpperCase()}
                            {transport ? ` · ${transport}` : ""} · {n.address}:{n.port}
                          </div>
                        </div>
                        <span className="shrink-0 text-xs tabular-nums text-[rgb(var(--muted))]">
                          {n.latencyMs != null ? `${n.latencyMs} ms` : "—"}
                        </span>
                      </button>
                      {!n.subscriptionId && (
                        <Button variant="ghost" onClick={() => void removeNode(n.id)}>
                          {t("common.remove")}
                        </Button>
                      )}
                    </li>
                  );
                })}
              </ul>
            )}
          </Card>
        </div>
      )}

      {vpnTab === "subscriptions" && (
        <div className="space-y-5">
          <div className="flex justify-end">
            <Button variant="primary" onClick={() => setAddOpen("sub")}>
              {t("vpn.addSubscription")}
            </Button>
          </div>
          {(details?.state.subscriptions.length ?? 0) === 0 ? (
            <Card>
              <p className="py-10 text-center text-sm text-[rgb(var(--text-secondary))]">{t("vpn.emptySubs")}</p>
            </Card>
          ) : (
            details!.state.subscriptions.map((sub) => (
              <SubscriptionCard
                key={sub.id}
                sub={sub}
                busy={busy}
                onUpdate={() => void updateSub(sub.id)}
                onRemove={() => void removeSub(sub.id)}
                onOpen={(url) => openUrl(url).catch(() => {})}
                t={t}
              />
            ))
          )}
        </div>
      )}

      {vpnTab === "settings" && settingsDraft && (
        <div className="space-y-4">
          <Card>
            <h3 className="mb-3 text-sm font-bold uppercase tracking-wide text-slate-400">{t("vpn.settings.general")}</h3>
            <FieldRow title={t("vpn.settings.mode")} description={t("vpn.settings.modeDesc")}>
              <Segmented
                value={settingsDraft.mode === "tun" ? "tun" : "system-proxy"}
                options={[
                  { value: "system-proxy", label: t("vpn.settings.modeProxy") },
                  { value: "tun", label: t("vpn.settings.modeTun") },
                ]}
                onChange={(v) => patchSettings({ mode: v })}
              />
            </FieldRow>
            {settingsDraft.mode === "tun" && (
              <Note tone="warn">{t("vpn.settings.tunNote")}</Note>
            )}
            <FieldRow title={t("vpn.settings.httpPort")}>
              <input
                type="number"
                className="w-24 rounded-lg border border-[rgb(var(--border))] bg-transparent px-2 py-1.5 text-sm"
                value={settingsDraft.httpPort}
                onChange={(e) => patchSettings({ httpPort: Number(e.target.value) || 10809 })}
              />
            </FieldRow>
            <FieldRow title={t("vpn.settings.socksPort")}>
              <input
                type="number"
                className="w-24 rounded-lg border border-[rgb(var(--border))] bg-transparent px-2 py-1.5 text-sm"
                value={settingsDraft.socksPort}
                onChange={(e) => patchSettings({ socksPort: Number(e.target.value) || 10808 })}
              />
            </FieldRow>
            <FieldRow title={t("vpn.settings.dns")}>
              <input
                className="w-40 rounded-lg border border-[rgb(var(--border))] bg-transparent px-2 py-1.5 text-sm"
                value={settingsDraft.dns}
                onChange={(e) => patchSettings({ dns: e.target.value })}
              />
            </FieldRow>
          </Card>

          <Card>
            <h3 className="mb-3 text-sm font-bold uppercase tracking-wide text-slate-400">{t("vpn.settings.routing")}</h3>
            <FieldRow title={t("vpn.settings.routingEnabled")} description={t("vpn.settings.routingDesc")}>
              <Switch checked={settingsDraft.routingEnabled} onChange={(v) => patchSettings({ routingEnabled: v })} />
            </FieldRow>
            <FieldRow title={t("vpn.settings.bypassLan")}>
              <Switch checked={settingsDraft.bypassLan} onChange={(v) => patchSettings({ bypassLan: v })} />
            </FieldRow>
            <FieldRow title={t("vpn.settings.bypassPrivate")}>
              <Switch checked={settingsDraft.bypassPrivate} onChange={(v) => patchSettings({ bypassPrivate: v })} />
            </FieldRow>
            <FieldRow title={t("vpn.settings.sniffing")}>
              <Switch checked={settingsDraft.sniffing} onChange={(v) => patchSettings({ sniffing: v })} />
            </FieldRow>
            <FieldRow title={t("vpn.settings.allowInsecure")} description={t("vpn.settings.allowInsecureDesc")}>
              <Switch checked={settingsDraft.allowInsecure} onChange={(v) => patchSettings({ allowInsecure: v })} />
            </FieldRow>
          </Card>

          <Card>
            <h3 className="mb-3 text-sm font-bold uppercase tracking-wide text-slate-400">{t("vpn.settings.advanced")}</h3>
            <FieldRow title={t("vpn.settings.mux")} description={t("vpn.settings.muxDesc")}>
              <Switch checked={settingsDraft.muxEnabled} onChange={(v) => patchSettings({ muxEnabled: v })} />
            </FieldRow>
            <FieldRow title={t("vpn.settings.fragmentation")} description={t("vpn.settings.fragmentationDesc")}>
              <Switch checked={settingsDraft.fragmentation} onChange={(v) => patchSettings({ fragmentation: v })} />
            </FieldRow>
            <FieldRow title={t("vpn.settings.autoConnect")}>
              <Switch checked={settingsDraft.autoConnect} onChange={(v) => patchSettings({ autoConnect: v })} />
            </FieldRow>
            <FieldRow title={t("vpn.settings.autoconnectType")}>
              <Segmented
                value={settingsDraft.autoconnectType}
                options={[
                  { value: "lastused", label: t("vpn.settings.lastUsed") },
                  { value: "lowestdelay", label: t("vpn.settings.lowestDelay") },
                ]}
                onChange={(v) => patchSettings({ autoconnectType: v })}
              />
            </FieldRow>
            <FieldRow title={t("vpn.settings.autoUpdateSubs")}>
              <Switch checked={settingsDraft.autoUpdateSubs} onChange={(v) => patchSettings({ autoUpdateSubs: v })} />
            </FieldRow>
            <FieldRow title={t("vpn.settings.updateOnOpen")}>
              <Switch checked={settingsDraft.updateOnOpen} onChange={(v) => patchSettings({ updateOnOpen: v })} />
            </FieldRow>
          </Card>

          <div className="flex justify-end">
            <Button variant="primary" disabled={busy !== null} onClick={() => void saveSettings()}>
              {busy === "settings" ? <Spinner /> : null}
              {t("common.save")}
            </Button>
          </div>
        </div>
      )}

      <Modal
        open={addOpen === "sub"}
        title={t("vpn.addSubscription")}
        onClose={() => setAddOpen(null)}
        footer={
          <>
            <Button onClick={() => setAddOpen(null)}>{t("common.cancel")}</Button>
            <Button variant="primary" disabled={!subUrl.trim() || busy !== null} onClick={() => void addSubscription()}>
              {busy === "add-sub" ? <Spinner /> : null}
              {t("common.apply")}
            </Button>
          </>
        }
      >
        <p className="mb-3 text-sm text-[rgb(var(--text-secondary))]">{t("vpn.addSubHint")}</p>
        <input
          autoFocus
          className="w-full rounded-xl border border-[rgb(var(--border))] bg-transparent px-3 py-2.5 text-sm"
          placeholder="https://…"
          value={subUrl}
          onChange={(e) => setSubUrl(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void addSubscription()}
        />
      </Modal>

      <Modal
        open={addOpen === "node"}
        title={t("vpn.addNode")}
        onClose={() => setAddOpen(null)}
        footer={
          <>
            <Button onClick={() => setAddOpen(null)}>{t("common.cancel")}</Button>
            <Button variant="primary" disabled={!nodeLink.trim() || busy !== null} onClick={() => void addNode()}>
              {busy === "add-node" ? <Spinner /> : null}
              {t("common.apply")}
            </Button>
          </>
        }
      >
        <p className="mb-3 text-sm text-[rgb(var(--text-secondary))]">{t("vpn.addNodeHint")}</p>
        <textarea
          autoFocus
          rows={4}
          className="w-full rounded-xl border border-[rgb(var(--border))] bg-transparent px-3 py-2.5 font-mono text-xs"
          placeholder="vless://… / vmess://… / trojan://… / ss://"
          value={nodeLink}
          onChange={(e) => setNodeLink(e.target.value)}
        />
      </Modal>
    </div>
  );
}

function SubscriptionCard({
  sub,
  busy,
  onUpdate,
  onRemove,
  onOpen,
  t,
}: {
  sub: VpnSubscription;
  busy: string | null;
  onUpdate: () => void;
  onRemove: () => void;
  onOpen: (url: string) => void;
  t: (key: string, opts?: Record<string, unknown>) => string;
}) {
  const used = (sub.userinfo?.upload ?? 0) + (sub.userinfo?.download ?? 0);
  return (
    <Card>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h3 className="truncate text-base font-bold text-[rgb(var(--text))]">{sub.name}</h3>
            <Badge tone="info">{sub.nodes.length}</Badge>
          </div>
          <p className="mt-1 truncate text-xs text-[rgb(var(--muted))]">{sub.url}</p>
          {sub.announce && (
            <p className="mt-2 text-sm text-[rgb(var(--text-secondary))]">{sub.announce}</p>
          )}
          <div className="mt-2 flex flex-wrap gap-3 text-xs text-[rgb(var(--text-secondary))]">
            <span>
              {t("vpn.traffic")}: {formatBytes(used)}
              {sub.userinfo?.total ? ` / ${formatBytes(sub.userinfo.total)}` : ""}
            </span>
            <span>
              {t("vpn.expires")}: {formatExpire(sub.userinfo?.expire)}
            </span>
            {sub.updatedAt && (
              <span>
                {t("vpn.updated")}: {new Date(sub.updatedAt).toLocaleString()}
              </span>
            )}
          </div>
        </div>
        <div className="flex flex-wrap gap-2">
          {sub.webPageUrl && (
            <Button variant="ghost" onClick={() => onOpen(sub.webPageUrl!)}>
              {t("vpn.website")}
            </Button>
          )}
          {sub.supportUrl && (
            <Button variant="ghost" onClick={() => onOpen(sub.supportUrl!)}>
              {t("vpn.support")}
            </Button>
          )}
          <Button variant="secondary" disabled={busy !== null} onClick={onUpdate}>
            {busy === `upd-${sub.id}` ? <Spinner /> : null}
            {t("common.refresh")}
          </Button>
          <Button variant="secondary" disabled={busy !== null} onClick={onRemove}>
            {t("common.remove")}
          </Button>
        </div>
      </div>
    </Card>
  );
}
