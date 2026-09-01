import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { setupAutopilotListener, useStore } from "./lib/store";
import { BootSplash } from "./components/BootSplash";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { SetupModal } from "./components/SetupModal";
import { UpdatesModal } from "./components/UpdatesModal";
import { WhatsNewModal } from "./components/WhatsNewModal";
import { Toasts } from "./components/Toasts";
import { HomePage } from "./pages/Home";
import { StrategiesPage } from "./pages/Strategies";
import { ZapretPage } from "./pages/Zapret";
import { AutopilotPage } from "./pages/Autopilot";
import { TelegramPage } from "./pages/Telegram";
import { WarpPage } from "./pages/Warp";
import { VpnPage } from "./pages/Vpn";
import { LogsPage } from "./pages/Logs";
import { SettingsPage } from "./pages/Settings";

function pollDelayMs(): number {
  const { status, settings } = useStore.getState();
  const busy =
    status?.autopilot?.checking ||
    status?.testsRunning ||
    settings?.autopilot?.enabled;
  return busy ? 8000 : 4000;
}

export default function App() {
  const { t } = useTranslation();
  const ready = useStore((s) => s.ready);
  const initError = useStore((s) => s.initError);
  const page = useStore((s) => s.page);
  const appInfo = useStore((s) => s.appInfo);
  const init = useStore((s) => s.init);
  const refreshStatus = useStore((s) => s.refreshStatus);

  useEffect(() => {
    init();
    const cleanupAp = setupAutopilotListener(t);

    let timer: ReturnType<typeof setTimeout>;
    const schedule = () => {
      timer = setTimeout(() => {
        refreshStatus().finally(schedule);
      }, pollDelayMs());
    };
    schedule();

    const updatesInterval = setInterval(
      () => {
        const { settings, checkUpdates } = useStore.getState();
        if (settings?.checkUpdatesOnStart) {
          checkUpdates({ silent: true }).catch(() => {});
        }
      },
      60 * 60 * 1000,
    );

    return () => {
      cleanupAp();
      clearTimeout(timer);
      clearInterval(updatesInterval);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (!ready) {
    return (
      <div className="flex h-full flex-col bg-[rgb(var(--surface))] text-[rgb(var(--text))]">
        <TitleBar />
        <div className="min-h-0 flex-1">
          <BootSplash />
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col bg-[rgb(var(--surface))] text-[rgb(var(--text))]">
      <TitleBar />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <main className="flex min-w-0 flex-1 flex-col">
          {initError && (
            <div className="bg-amber-600/90 px-5 py-2 text-center text-sm font-medium text-white">
              {t("errors.initFailed")}
            </div>
          )}
          {appInfo && appInfo.isWindows && !appInfo.isAdmin && (
            <div className="bg-red-600 px-5 py-2 text-center text-sm font-semibold text-white">
              {t("adminWarning")}
            </div>
          )}
          <div className="min-h-0 flex-1 overflow-y-auto p-5 md:p-8">
            {page === "home" && <HomePage />}
            {page === "autopilot" && <AutopilotPage />}
            {page === "strategies" && <StrategiesPage />}
            {page === "zapret" && <ZapretPage />}
            {page === "telegram" && <TelegramPage />}
            {page === "vpn" && <VpnPage />}
            {page === "warp" && <WarpPage />}
            {page === "logs" && <LogsPage />}
            {page === "settings" && <SettingsPage />}
          </div>
        </main>
      </div>
      <SetupModal />
      <UpdatesModal />
      <WhatsNewModal />
      <Toasts />
    </div>
  );
}
