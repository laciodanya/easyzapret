import { useTranslation } from "react-i18next";
import { useStore, type ZapretTab } from "../lib/store";
import { PageHeader, Segmented } from "../components/ui";
import { ServicePage } from "./Service";
import { TestsPage } from "./Tests";
import { ListsPage } from "./Lists";

export function ZapretPage() {
  const { t } = useTranslation();
  const { zapretTab, setZapretTab } = useStore();

  const tabs: { value: ZapretTab; label: string }[] = [
    { value: "service", label: t("zapret.tabs.service") },
    { value: "tests", label: t("zapret.tabs.tests") },
    { value: "lists", label: t("zapret.tabs.lists") },
  ];

  return (
    <div className="ez-fade-up mx-auto flex h-full max-w-4xl flex-col">
      <PageHeader title={t("zapret.title")} description={t("zapret.description")} />
      <div className="mb-5">
        <Segmented value={zapretTab} options={tabs} onChange={setZapretTab} />
      </div>
      <div className="min-h-0 flex-1">
        {zapretTab === "service" && <ServicePage embedded />}
        {zapretTab === "tests" && <TestsPage embedded />}
        {zapretTab === "lists" && <ListsPage embedded />}
      </div>
    </div>
  );
}
