import { useTranslation } from "react-i18next";
import { useStore, type ZapretTab } from "../lib/store";
import { PageHeader, Tabs } from "../components/ui";
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
    <div>
      <PageHeader title={t("zapret.title")} description={t("zapret.description")} />
      <Tabs value={zapretTab} options={tabs} onChange={setZapretTab} />
      {zapretTab === "service" && <ServicePage />}
      {zapretTab === "tests" && <TestsPage />}
      {zapretTab === "lists" && <ListsPage />}
    </div>
  );
}
