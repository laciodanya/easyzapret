import { useTranslation } from "react-i18next";
import { useStore } from "../lib/store";
import { Button, Modal } from "./ui";

export function WhatsNewModal() {
  const { t } = useTranslation();
  const { showWhatsNew, appInfo, dismissWhatsNew } = useStore();

  if (!showWhatsNew) return null;

  const items = [
    t("whatsNew.itemAutopilotFix"),
    t("whatsNew.itemAutostart"),
    t("whatsNew.itemStartup"),
    t("whatsNew.itemTheme"),
  ];

  return (
    <Modal
      open={showWhatsNew}
      onClose={() => dismissWhatsNew()}
      title={t("whatsNew.title", { version: appInfo?.version ?? "0.3.0" })}
      footer={
        <Button variant="primary" onClick={() => dismissWhatsNew()}>
          {t("whatsNew.gotIt")}
        </Button>
      }
    >
      <p className="mb-4 text-sm leading-relaxed text-slate-600 dark:text-slate-300">
        {t("whatsNew.intro")}
      </p>
      <ul className="space-y-2.5">
        {items.map((item) => (
          <li
            key={item}
            className="flex gap-2.5 rounded-xl bg-accent-soft px-3.5 py-2.5 text-sm leading-relaxed text-slate-700 dark:text-slate-200"
          >
            <span className="text-accent">✦</span>
            <span>{item}</span>
          </li>
        ))}
      </ul>
    </Modal>
  );
}
