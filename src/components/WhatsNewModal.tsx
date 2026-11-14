import { useTranslation } from "react-i18next";
import { useStore } from "../lib/store";
import { Button, Modal } from "./ui";

const ITEMS = ["autopilot", "tests", "logs", "telegram", "warp", "design", "maintenance"] as const;

export function WhatsNewModal() {
  const { t } = useTranslation();
  const { showWhatsNew, appInfo, dismissWhatsNew } = useStore();

  if (!showWhatsNew) return null;

  return (
    <Modal
      open={showWhatsNew}
      onClose={() => void dismissWhatsNew()}
      title={t("whatsNew.title", { version: appInfo?.version ?? "" })}
      size="lg"
      footer={
        <Button variant="primary" onClick={() => void dismissWhatsNew()}>
          {t("whatsNew.gotIt")}
        </Button>
      }
    >
      <div className="divide-y divide-line">
        {ITEMS.map((id) => (
          <div key={id} className="py-3">
            <div className="text-sm font-medium text-fg">{t(`whatsNew.items.${id}.title`)}</div>
            <div className="mt-0.5">{t(`whatsNew.items.${id}.text`)}</div>
          </div>
        ))}
      </div>
    </Modal>
  );
}
