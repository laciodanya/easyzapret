import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { VpnDetails, VpnProfile } from "../lib/vpn";
import { Icon } from "./icons";
import { Empty, Input, Modal, Spinner, cn } from "./ui";

export function VpnServerButton({ name, disabled, onClick }: {
  name?: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  const { t } = useTranslation();
  return (
    <button type="button" disabled={disabled} onClick={onClick}
      className="flex min-w-0 w-full items-center justify-between gap-4 rounded-panel border border-line bg-panel px-4 py-2.5 text-left transition-colors enabled:hover:border-line-strong disabled:opacity-50">
      <span className="min-w-0">
        <span className="block text-xs text-muted">{t("vpn.serverLabel")}</span>
        <span className="block truncate text-sm font-medium">{name ?? t("vpn.pickServer")}</span>
      </span>
      <Icon name="chevronDown" size={16} className="shrink-0 text-muted" />
    </button>
  );
}

/** Mirrors the strategy picker, grouping servers by the subscription they came from. */
export function VpnServerPicker({ details, busy, onClose, onPick, actions }: {
  details: VpnDetails;
  busy: boolean;
  onClose: () => void;
  onPick: (id: string) => void;
  actions?: (profile: VpnProfile) => ReactNode;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const lookup = query.trim().toLocaleLowerCase();
  const groups = new Map<string | null, { label: string; servers: VpnProfile[] }>();
  groups.set(null, { label: t("vpn.manualServers"), servers: [] });
  for (const subscription of details.subscriptions) {
    groups.set(subscription.id, { label: subscription.name, servers: [] });
  }
  for (const server of details.profiles) {
    const group = groups.get(server.subscriptionId) ?? groups.get(null)!;
    if (!lookup || [server.name, server.server, server.protocol, group.label]
      .some((field) => field.toLocaleLowerCase().includes(lookup))) {
      group.servers.push(server);
    }
  }
  const visibleGroups = Array.from(groups).filter(([, group]) => group.servers.length > 0);

  return (
    <Modal open title={t("vpn.pickServer")} size="lg" onClose={busy ? undefined : onClose}>
      <Input aria-label={t("vpn.searchServers")} placeholder={t("vpn.searchServers")}
        className="mb-4 w-full" value={query} onChange={(event) => setQuery(event.target.value)}
        disabled={busy} autoFocus />
      {busy && <div role="status" className="mb-3 flex items-center gap-2 text-sm text-muted">
        <Spinner size={14} /> {t("common.loading")}
      </div>}
      <div className="max-h-[52vh] space-y-5 overflow-y-auto pr-1">
        {visibleGroups.map(([id, group]) => (
          <div key={id ?? "manual"}>
            <div className="mb-1.5 break-words text-xs font-medium text-faint">{group.label}</div>
            <div className="grid gap-1 sm:grid-cols-2">
              {group.servers.map((server) => {
                const active = server.id === details.selectedProfile;
                return (
                  <div key={server.id} className="flex min-w-0 items-center gap-0.5">
                    <button type="button" aria-pressed={active} disabled={busy} onClick={() => onPick(server.id)}
                      className={cn(
                        "flex min-w-0 flex-1 items-center justify-between gap-2 rounded-control px-3 py-1.5 text-left text-[13px] transition-colors disabled:opacity-65",
                        active ? "bg-accent-soft font-medium text-accent-text" : "text-fg enabled:hover:bg-raised",
                      )}>
                      <span className="min-w-0">
                        <span className="block truncate" title={server.name}>{server.name}</span>
                        <span className="block truncate text-xs font-normal text-muted">{server.protocol.toUpperCase()} · {server.server}</span>
                      </span>
                      {active && <Icon name="check" size={15} strokeWidth={2.25} className="shrink-0" />}
                    </button>
                    {actions?.(server)}
                  </div>
                );
              })}
            </div>
          </div>
        ))}
        {visibleGroups.length === 0 && <Empty>{t(lookup ? "vpn.noServerMatches" : "vpn.noProfiles")}</Empty>}
      </div>
    </Modal>
  );
}
