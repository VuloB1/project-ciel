import { open, save } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { errorMessage } from "@/lib/errorMessage";
import { createBackup } from "@/lib/ipc/backup";
import { useToastStore } from "@/stores/toastStore";

import { formatBytes, useRestoreFlow } from "./useRestoreFlow";

export { formatBytes };

const BACKUP_EXTENSION = "aivtbackup";

function defaultFileName(): string {
  return `Project-Ciel-${new Date().toISOString().slice(0, 10)}.${BACKUP_EXTENSION}`;
}

/** Copia de seguridad completa y restauración. Los secretos (API keys, tokens) NO van
 * en la copia: viven en el almacén seguro del sistema, y se avisa en pantalla. */
export function BackupSettings() {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const [isCreating, setIsCreating] = useState(false);
  const restore = useRestoreFlow();
  const isWorking = isCreating || restore.isWorking;

  async function handleCreate() {
    try {
      const path = await save({
        defaultPath: defaultFileName(),
        filters: [{ name: "Project Ciel Backup", extensions: [BACKUP_EXTENSION] }],
      });
      if (!path) return;
      setIsCreating(true);
      const summary = await createBackup(path);
      pushToast({
        variant: "success",
        title: t("settings.backup.created"),
        description: t("settings.backup.createdDetail", { size: formatBytes(summary.sizeBytes), files: summary.files }),
      });
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.backup.createError"), description: errorMessage(err) });
    } finally {
      setIsCreating(false);
    }
  }

  async function handlePickRestore() {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Project Ciel Backup", extensions: [BACKUP_EXTENSION] }],
    });
    if (!picked || Array.isArray(picked)) return;
    await restore.begin(picked);
  }

  return (
    <div className="flex max-w-xl flex-col gap-4">
      <Card className="flex flex-col p-0">
        <div className="flex flex-col gap-3 p-4">
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.backup.createTitle")}</p>
            <p className="text-xs text-text-secondary">{t("settings.backup.createDescription")}</p>
          </div>
          <ul className="list-disc pl-5 text-xs text-text-secondary">
            <li>{t("settings.backup.includes")}</li>
            <li>{t("settings.backup.excludesSecrets")}</li>
            <li>{t("settings.backup.excludesDownloads")}</li>
          </ul>
          <div>
            <Button size="sm" disabled={isWorking} onClick={() => void handleCreate()}>
              {isCreating ? t("settings.backup.working") : t("settings.backup.createAction")}
            </Button>
          </div>
        </div>

        <div className="flex flex-col gap-3 border-t border-border p-4">
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.backup.restoreTitle")}</p>
            <p className="text-xs text-text-secondary">{t("settings.backup.restoreDescription")}</p>
          </div>
          <div>
            <Button size="sm" variant="secondary" disabled={isWorking} onClick={() => void handlePickRestore()}>
              {t("settings.backup.restoreAction")}
            </Button>
          </div>
        </div>
      </Card>

      {restore.dialog}
    </div>
  );
}
