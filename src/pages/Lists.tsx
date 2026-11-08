import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { Button, Note, Panel, Spinner, cn } from "../components/ui";
import type { UserListFile } from "../lib/types";

export function ListsPage() {
  const { t } = useTranslation();
  const [files, setFiles] = useState<UserListFile[]>([]);
  const [active, setActive] = useState<string>("list-general-user.txt");
  // Unsaved edits are kept per file so switching files never loses changes.
  const [edits, setEdits] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);

  async function load() {
    const { files } = await api.readUserLists();
    setFiles(files);
  }

  useEffect(() => {
    void load();
  }, []);

  const activeFile = files.find((f) => f.name === active);
  const content = edits[active] ?? activeFile?.content ?? "";
  const dirty = edits[active] !== undefined && edits[active] !== (activeFile?.content ?? "");

  async function save() {
    setSaving(true);
    try {
      await api.saveUserList(active, content);
      toast(t("common.saved"), "ok");
      setEdits((e) => {
        const next = { ...e };
        delete next[active];
        return next;
      });
      await load();
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setSaving(false);
    }
  }

  const hint = t(`lists.hint.${active}`, { defaultValue: "" });

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap gap-1.5">
        {files.map((f) => (
          <button
            key={f.name}
            type="button"
            onClick={() => setActive(f.name)}
            className={cn(
              "h-8 rounded-control px-3 font-mono text-xs transition-colors",
              active === f.name
                ? "bg-accent-soft font-medium text-accent-text"
                : "text-muted hover:bg-raised hover:text-fg",
            )}
          >
            {f.name}
            {edits[f.name] !== undefined && edits[f.name] !== f.content && " •"}
            {!f.exists && " *"}
          </button>
        ))}
      </div>

      <Panel
        title={active}
        description={
          [hint, activeFile && !activeFile.exists ? t("lists.fileMissing") : ""]
            .filter(Boolean)
            .join(" — ") || undefined
        }
        action={
          <Button variant="primary" onClick={() => void save()} disabled={!dirty || saving}>
            {saving && <Spinner size={14} />}
            {t("common.save")}
          </Button>
        }
        padded
      >
        <textarea
          value={content}
          onChange={(e) => {
            const value = e.target.value;
            setEdits((prev) => ({ ...prev, [active]: value }));
          }}
          spellCheck={false}
          className="h-[360px] w-full resize-y rounded-control border border-line bg-sunken p-3 font-mono text-xs leading-relaxed focus:border-accent"
        />
      </Panel>

      <Note>{t("strategies.restartHint")}</Note>
    </div>
  );
}
