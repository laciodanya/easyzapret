import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import { useStore } from "../lib/store";
import { toast } from "../lib/toast";
import { vpnErrorText, type VpnDetails, type VpnProfile, type VpnRule, type VpnSettings } from "../lib/vpn";
import { VpnServerButton, VpnServerPicker } from "../components/VpnServerPicker";
import { Badge, Button, ConfirmModal, Empty, IconButton, Input, Modal, Note, PageHeader,
  Panel, Row, Select, Spinner, Switch, Tabs } from "../components/ui";

const TEXTAREA = "selectable w-full rounded-control border border-line-strong bg-sunken p-3 font-mono text-[13px] text-fg focus:border-accent";

export function VpnPage() {
  const { t } = useTranslation();
  const appInfo = useStore((s) => s.appInfo);
  const status = useStore((s) => s.status);
  const refreshStatus = useStore((s) => s.refreshStatus);
  const { vpnInstalling, vpnInstallProgress, vpnInstallError, installVpn, setPage } = useStore();
  const [pickerOpen, setPickerOpen] = useState(false);
  const [details, setDetails] = useState<VpnDetails | null>(null);
  const [draft, setDraft] = useState<VpnSettings | null>(null);
  const [tab, setTab] = useState("profiles");
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [importOpen, setImportOpen] = useState(false);
  const [input, setInput] = useState("");
  const [editor, setEditor] = useState<VpnProfile | null>(null);
  const [editorJson, setEditorJson] = useState("");
  const [editorName, setEditorName] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; kind: "profile" | "subscription" } | null>(null);
  const [appInput, setAppInput] = useState("");
  const [availableApps, setAvailableApps] = useState<string[]>([]);
  const [appsOpen, setAppsOpen] = useState(false);
  const [appSearch, setAppSearch] = useState("");
  const [ruleOpen, setRuleOpen] = useState(false);
  const [ruleKind, setRuleKind] = useState("domain_suffix");
  const [ruleValues, setRuleValues] = useState("");
  const [ruleOutbound, setRuleOutbound] = useState<VpnRule["outbound"]>("proxy");
  const [latency, setLatency] = useState<number | null>(null);

  const vpn = status?.vpn ?? details?.status;
  const running = vpn?.running ?? false;
  const installed = vpn?.installed ?? details?.status.installed ?? false;
  const dirty = !!draft && !!details && JSON.stringify(draft) !== JSON.stringify(details.settings);
  const selected = details?.profiles.find((p) => p.id === details.selectedProfile);
  const usesServices = !!(status?.zapret.serviceInstalled || status?.zapret.windivertState);
  const switching = !running && !!(status?.warp.connected || status?.zapret.running || usesServices);
  const unsupported = appInfo?.isWindows === false;
  const progress = vpnInstallProgress;
  const needsApps = !!draft && ["only_apps", "bypass_apps"].includes(draft.mode) && draft.apps.length === 0;

  function accept(next: VpnDetails) {
    setDetails(next);
    setDraft(next.settings);
    setLoadError(null);
  }

  async function perform(task: () => Promise<unknown>) {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    try {
      await task();
      await refreshStatus();
    } catch (error) {
      toast(vpnErrorText(t, error), "fail");
      await refreshStatus();
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }

  useEffect(() => {
    let active = true;
    api.vpnDetails().then((next) => { if (active) accept(next); })
      .catch((error) => { if (active) setLoadError(vpnErrorText(t, error)); });
    return () => { active = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function patch(patch: Partial<VpnSettings>) {
    setDraft((current) => current ? { ...current, ...patch } : current);
  }

  function addApp(raw: string) {
    const app = raw.trim().replace(/^"|"$/g, "");
    if (!app || !draft) return;
    if (!app.toLowerCase().endsWith(".exe")) {
      toast(t("vpn.errors.vpn_invalid_application"), "fail");
      return;
    }
    if (!draft.apps.some((p) => p.toLowerCase() === app.toLowerCase())) {
      patch({ apps: [...draft.apps, app] });
    }
    setAppInput("");
  }

  const saveButton = dirty && (
    <Button variant="primary" disabled={busy} onClick={() => void perform(async () => {
      if (!draft) return;
      accept(await api.vpnSaveSettings(draft));
      setLatency(null);
      toast(t("vpn.saved"), "ok");
    })}>{running ? t("vpn.applyReconnect") : t("common.save")}</Button>
  );

  return (
    <>
      <PageHeader title="VPN" description={t("vpn.description")} action={
        <Button variant={running ? "secondary" : "primary"} icon="power"
          disabled={busy || vpnInstalling || unsupported || !details || (!running && (!installed || !selected || dirty || needsApps))}
          onClick={() => void perform(async () => {
            if (running) await api.vpnDisconnect();
            else await api.vpnConnect();
            setLatency(null);
          })}>
          {busy && <Spinner size={14} />}
          {running ? t("vpn.disconnect") : t("vpn.connect")}
        </Button>
      } />
      <div className="mb-6 flex flex-wrap items-center gap-3 text-[13px] text-muted">
        <Badge tone={running ? "ok" : "off"}>{running ? t("vpn.connected") : t("vpn.disconnected")}</Badge>
        {selected && <span className="min-w-0 max-w-[34ch] truncate">{selected.name}</span>}
        {running && <Button size="sm" variant="ghost" icon="activity" disabled={busy}
          onClick={() => void perform(async () => setLatency(await api.vpnTestConnection()))}>
          {latency === null ? t("vpn.test") : t("vpn.latency", { ms: latency })}
        </Button>}
        {saveButton}
      </div>
      <div className="mb-6 space-y-3">
        {appInfo && !appInfo.isWindows && <Note>{t("vpn.windowsOnly")}</Note>}
        {loadError && <Note tone="fail" action={<Button size="sm" disabled={busy}
          onClick={() => void perform(async () => accept(await api.vpnDetails()))}>{t("vpn.retry")}</Button>}>{loadError}</Note>}
        {vpn?.error && <Note tone="fail">{vpnErrorText(t, vpn.error)}</Note>}
        {switching && <Note tone="warn">{t("vpn.switchToVpn")}</Note>}
        {installed && selected && needsApps && tab !== "routing" && <Note action={
          <Button size="sm" onClick={() => setTab("routing")}>{t("vpn.openRouting")}</Button>
        }>{t("vpn.errors.vpn_select_apps")}</Note>}
        {(!installed || vpnInstalling) && !unsupported && (
          <Note title={t("vpn.installTitle")} action={
            <Button icon="download" disabled={busy || vpnInstalling}
              onClick={() => void installVpn()}>
              {t(vpnInstalling ? "vpn.installing" : vpnInstallError ? "vpn.retry" : "vpn.install")}
            </Button>
          }>
            {vpnInstalling ? (
              <div role="status" className="space-y-2">
                <div className="flex items-center gap-2">
                  <Spinner size={13} />
                  {t(progress ? `vpn.installPhases.${progress.phase}` : "vpn.installPhases.preparing", { asset: progress?.asset })}
                </div>
                {progress?.phase === "downloading" && (
                  <div className="space-y-1">
                    <progress aria-label={t("vpn.downloadProgress")} max={progress.total} value={progress.downloaded}
                      className="block h-2 w-full min-w-36 accent-accent" />
                    <span className="tabular-nums">{t("vpn.downloadSize", {
                      current: (progress.downloaded / 1048576).toFixed(1),
                      total: (progress.total / 1048576).toFixed(1),
                    })}</span>
                  </div>
                )}
              </div>
            ) : t("vpn.installDescription")}
          </Note>
        )}
        {vpnInstallError && <Note tone="fail" action={
          <Button size="sm" onClick={() => setPage("logs")}>{t("vpn.openLogs")}</Button>
        }>{vpnInstallError}</Note>}
        {dirty && <Note>{running ? t("vpn.pendingReconnect") : t("vpn.pendingSave")}</Note>}
      </div>
      {details && draft && <>
        <Tabs value={tab} onChange={setTab} options={[
          { value: "profiles", label: t("vpn.profiles") },
          { value: "routing", label: t("vpn.routing") },
          { value: "settings", label: t("vpn.settings") },
        ]} />
        {tab === "profiles" && <div className="space-y-5">
          <Panel title={t("vpn.profiles")} action={<Button icon="plus" disabled={busy || dirty}
            onClick={() => setImportOpen(true)}>{t("vpn.add")}</Button>}>
            {details.profiles.length === 0 ? <Empty>{t("vpn.noProfiles")}</Empty> : (
              <div className="py-3">
                <VpnServerButton name={selected?.name} disabled={busy || dirty}
                  onClick={() => setPickerOpen(true)} />
              </div>
            )}
          </Panel>
          {details.subscriptions.length > 0 && <Panel title={t("vpn.subscriptions")}>
            {details.subscriptions.map((subscription) => <Row key={subscription.id} title={subscription.name}
              description={t("vpn.updated", { date: new Date(subscription.updatedAt).toLocaleString() })}>
              <IconButton icon="refresh" label={t("vpn.refreshSubscription")} disabled={busy || dirty}
                onClick={() => void perform(async () => accept(await api.vpnRefreshSubscription(subscription.id)))} />
              <IconButton icon="trash" label={t("vpn.removeSubscription")} disabled={busy || dirty}
                onClick={() => setDeleteTarget({ id: subscription.id, kind: "subscription" })} />
            </Row>)}
          </Panel>}
        </div>}
        {tab === "routing" && <div className="space-y-5">
          <Panel title={t("vpn.routingMode")}>
            <Row title={t("vpn.traffic")}>
              <Select aria-label={t("vpn.traffic")} value={draft.mode} disabled={busy}
                onChange={(e) => patch({ mode: e.target.value as VpnSettings["mode"] })}>
                {["only_apps", "bypass_apps", "all", "proxy"].map((mode) =>
                  <option key={mode} value={mode}>{t(`vpn.modes.${mode}`)}</option>)}
              </Select>
            </Row>
            <Row title={t("vpn.bypassLan")} description={t("vpn.bypassLanDescription")}>
              <Switch label={t("vpn.bypassLan")} checked={draft.bypassLan} disabled={busy}
                onChange={(value) => patch({ bypassLan: value })} />
            </Row>
          </Panel>
          {(draft.mode === "only_apps" || draft.mode === "bypass_apps") && <Panel title={t("vpn.applications")}
            description={t(`vpn.appDescriptions.${draft.mode}`)} padded>
            <form className="flex gap-2" onSubmit={(e) => { e.preventDefault(); addApp(appInput); }}>
              <Input aria-label={t("vpn.appPath")} placeholder="Discord.exe" value={appInput} disabled={busy}
                onChange={(e) => setAppInput(e.target.value)} className="min-w-0 flex-1" />
              <Button type="submit" disabled={busy || !appInput.trim()}>{t("vpn.addApp")}</Button>
              <IconButton icon="folder" label={t("vpn.browseApp")} disabled={busy}
                onClick={() => void perform(async () => { const path = await api.vpnPickApp(); if (path) addApp(path); })} />
            </form>
            <Button size="sm" variant="ghost" icon="refresh" disabled={busy} className="mt-3"
              onClick={() => void perform(async () => { setAvailableApps(await api.vpnListApps()); setAppsOpen(true); })}>
              {t("vpn.runningApps")}
            </Button>
            {draft.apps.length === 0 && <Empty>{t("vpn.noApps")}</Empty>}
            <div className="mt-2 divide-y divide-line">
              {draft.apps.map((app) => <div key={app} className="flex items-center gap-3 py-2">
                <span className="selectable min-w-0 flex-1 break-all text-[13px] text-fg">{app}</span>
                <IconButton icon="x" label={t("vpn.removeApp")} disabled={busy}
                  onClick={() => patch({ apps: draft.apps.filter((p) => p !== app) })} />
              </div>)}
            </div>
          </Panel>}
          {draft.mode === "proxy" && <Note>{t("vpn.proxyDescription", { port: draft.localPort })}</Note>}
          <Panel title={t("vpn.rules")} description={t("vpn.rulesDescription")} action={
            <Button icon="plus" disabled={busy} onClick={() => setRuleOpen(true)}>{t("vpn.addRule")}</Button>}>
            {draft.rules.length === 0 && <Empty>{t("vpn.noRules")}</Empty>}
            {draft.rules.map((rule, index) => <div key={index} className="flex items-center gap-3 py-3">
              <div className="min-w-0 flex-1">
                <div className="text-[13px] font-medium">{t(`vpn.ruleKinds.${rule.kind}`)} · {t(`vpn.outbounds.${rule.outbound}`)}</div>
                <div className="selectable break-all text-xs text-muted">{rule.values.join(", ")}</div>
              </div>
              <IconButton icon="trash" label={t("vpn.removeRule")} disabled={busy}
                onClick={() => patch({ rules: draft.rules.filter((_, i) => i !== index) })} />
            </div>)}
          </Panel>
          {saveButton}
        </div>}
        {tab === "settings" && <div className="space-y-5">
          <Panel title={t("vpn.dns")}>
            <Row title={t("vpn.dnsServer")} description={t("vpn.dnsDescription")}>
              <Input aria-label={t("vpn.dnsServer")} className="w-64" value={draft.dnsServer} disabled={busy}
                onChange={(e) => patch({ dnsServer: e.target.value })} />
            </Row>
            <Row title={t("vpn.dnsThroughVpn")} description={t("vpn.dnsThroughVpnDescription")}>
              <Switch label={t("vpn.dnsThroughVpn")} checked={draft.dnsThroughVpn} disabled={busy}
                onChange={(value) => patch({ dnsThroughVpn: value })} />
            </Row>
          </Panel>
          <Panel title={t("vpn.tunnel")}>
            <Row title="MTU"><Input aria-label="MTU" type="number" min={1280} max={9000} className="w-24"
              value={draft.mtu} disabled={busy} onChange={(e) => patch({ mtu: Number(e.target.value) })} /></Row>
            <Row title={t("vpn.stack")}><Select aria-label={t("vpn.stack")} value={draft.stack} disabled={busy}
              onChange={(e) => patch({ stack: e.target.value as VpnSettings["stack"] })}>
              {["mixed", "system", "gvisor"].map((stack) => <option key={stack} value={stack}>{stack}</option>)}
            </Select></Row>
            <Row title={t("vpn.ipv6")}><Switch label={t("vpn.ipv6")} checked={draft.ipv6} disabled={busy}
              onChange={(value) => patch({ ipv6: value })} /></Row>
            <Row title={t("vpn.sniff")} description={t("vpn.sniffDescription")}>
              <Switch label={t("vpn.sniff")} checked={draft.sniff} disabled={busy} onChange={(value) => patch({ sniff: value })} />
            </Row>
            <Row title={t("vpn.localPort")} description={t("vpn.localPortDescription")}>
              <Input aria-label={t("vpn.localPort")} type="number" min={1024} max={65533} className="w-24"
                value={draft.localPort} disabled={busy} onChange={(e) => patch({ localPort: Number(e.target.value) })} />
            </Row>
            <Row title={t("vpn.autoConnect")}><Switch label={t("vpn.autoConnect")} checked={draft.autoConnect} disabled={busy}
              onChange={(value) => patch({ autoConnect: value })} /></Row>
          </Panel>
          {saveButton}
        </div>}
      </>}
      {!details && !loadError && <div className="py-8"><Spinner /></div>}
      {pickerOpen && details && <VpnServerPicker details={details} busy={busy}
        onClose={() => setPickerOpen(false)}
        onPick={(id) => {
          if (id === details.selectedProfile) return setPickerOpen(false);
          void perform(async () => {
            accept(await api.vpnSelectProfile(id));
            setLatency(null);
            setPickerOpen(false);
          });
        }}
        actions={(profile) => <>
          <IconButton icon="sliders" label={t("vpn.editProfile")} disabled={busy}
            onClick={() => void perform(async () => {
              setEditorJson(await api.vpnProfileJson(profile.id));
              setEditorName(profile.name);
              setPickerOpen(false);
              setEditor(profile);
            })} />
          <IconButton icon="trash" label={t("vpn.removeProfile")} disabled={busy}
            onClick={() => {
              setPickerOpen(false);
              setDeleteTarget({ id: profile.id, kind: "profile" });
            }} />
        </>}
      />}
      <Modal open={importOpen} onClose={busy ? undefined : () => { setImportOpen(false); setInput(""); }} title={t("vpn.add")}
        footer={<Button variant="primary" disabled={busy || !input.trim()} onClick={() => void perform(async () => {
          accept(await api.vpnImport(input)); setImportOpen(false); setInput("");
          toast(t("vpn.imported"), "ok");
        })}>{busy ? <Spinner size={14} /> : t("vpn.import")}</Button>}>
        <p className="mb-3">{t("vpn.importDescription")}</p>
        <label htmlFor="vpn-import" className="mb-2 block text-[13px]">{t("vpn.key")}</label>
        <textarea id="vpn-import" value={input} disabled={busy} autoComplete="off" spellCheck={false}
          rows={7} className={TEXTAREA} onChange={(e) => setInput(e.target.value)} placeholder="vless://…" />
        <p className="mt-3 text-xs">{t("vpn.secretStorage")}</p>
      </Modal>
      <Modal open={!!editor} onClose={busy ? undefined : () => { setEditor(null); setEditorJson(""); }} title={t("vpn.editProfile")} size="lg"
        footer={<Button variant="primary" disabled={busy || !editorName.trim() || !editorJson.trim()}
          onClick={() => void perform(async () => {
            if (!editor) return;
            accept(await api.vpnUpdateProfile(editor.id, editorName.trim(), editorJson));
            setEditor(null); setEditorJson(""); setLatency(null);
          })}>{running ? t("vpn.applyReconnect") : t("common.save")}</Button>}>
        <label htmlFor="vpn-profile-name" className="mb-2 block">{t("vpn.profileName")}</label>
        <Input id="vpn-profile-name" value={editorName} disabled={busy} className="mb-4 w-full"
          onChange={(e) => setEditorName(e.target.value)} />
        <label htmlFor="vpn-profile-json" className="mb-2 block">{t("vpn.profileJson")}</label>
        <textarea id="vpn-profile-json" className={TEXTAREA} rows={14} spellCheck={false} value={editorJson} disabled={busy}
          onChange={(e) => setEditorJson(e.target.value)} />
      </Modal>
      <Modal open={appsOpen} onClose={() => setAppsOpen(false)} title={t("vpn.runningApps")} size="lg">
        <Input aria-label={t("vpn.searchApps")} placeholder={t("vpn.searchApps")} value={appSearch} className="mb-3 w-full"
          onChange={(e) => setAppSearch(e.target.value)} />
        <div className="max-h-80 overflow-y-auto">
          {availableApps.filter((app) => app.toLowerCase().includes(appSearch.toLowerCase())).map((app) =>
            <button key={app} type="button" onClick={() => { addApp(app); setAppsOpen(false); }}
              className="block w-full break-all rounded-control px-3 py-2 text-left text-[13px] text-fg hover:bg-raised">{app}</button>)}
          {availableApps.length === 0 && <Empty>{t("vpn.noRunningApps")}</Empty>}
        </div>
      </Modal>
      <Modal open={ruleOpen} onClose={() => setRuleOpen(false)} title={t("vpn.addRule")}
        footer={<Button variant="primary" disabled={!ruleValues.trim()} onClick={() => {
          if (!draft) return;
          patch({ rules: [...draft.rules, { kind: ruleKind, values: ruleValues.split(/\r?\n/).map((v) => v.trim()).filter(Boolean), outbound: ruleOutbound }] });
          setRuleOpen(false); setRuleValues("");
        }}>{t("vpn.addRule")}</Button>}>
        <label htmlFor="vpn-rule-kind" className="mb-2 block">{t("vpn.ruleType")}</label>
        <Select id="vpn-rule-kind" value={ruleKind} className="mb-3 w-full" onChange={(e) => setRuleKind(e.target.value)}>
          {["domain_suffix", "domain", "domain_keyword", "domain_regex", "ip_cidr", "process_name", "process_path"].map((kind) =>
            <option key={kind} value={kind}>{t(`vpn.ruleKinds.${kind}`)}</option>)}
        </Select>
        <label htmlFor="vpn-rule-values" className="mb-2 block">{t("vpn.ruleValues")}</label>
        <textarea id="vpn-rule-values" className={TEXTAREA} rows={5} spellCheck={false} value={ruleValues}
          onChange={(e) => setRuleValues(e.target.value)} />
        <label htmlFor="vpn-rule-outbound" className="mb-2 mt-3 block">{t("vpn.ruleAction")}</label>
        <Select id="vpn-rule-outbound" value={ruleOutbound} className="w-full"
          onChange={(e) => setRuleOutbound(e.target.value as VpnRule["outbound"])}>
          {["proxy", "direct", "reject"].map((outbound) => <option key={outbound} value={outbound}>{t(`vpn.outbounds.${outbound}`)}</option>)}
        </Select>
      </Modal>
      <ConfirmModal open={!!deleteTarget} title={t("vpn.removeTitle")} confirmLabel={t("common.remove")} tone="danger" busy={busy}
        onClose={() => setDeleteTarget(null)} onConfirm={() => void perform(async () => {
          if (!deleteTarget) return;
          accept(await (deleteTarget.kind === "profile" ? api.vpnRemoveProfile(deleteTarget.id) : api.vpnRemoveSubscription(deleteTarget.id)));
          setDeleteTarget(null); setLatency(null);
        })}>{t(deleteTarget?.kind === "subscription" ? "vpn.removeSubscriptionDescription" : "vpn.removeProfileDescription")}</ConfirmModal>
    </>
  );
}
