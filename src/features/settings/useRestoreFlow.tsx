import { useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { ConfirmDialog } from "@/design-system/Dialog";
import { errorMessage } from "@/lib/errorMessage";
import { formatDateTime } from "@/lib/formatDate";
import { cancelRestore, prepareRestore, restartApp, type RestorePreview } from "@/lib/ipc/backup";
import { useToastStore } from "@/stores/toastStore";

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`;
}

/** Two-step restore of a `.aivtbackup`: `begin(path)` validates and stages it
 * (`prepareRestore`), then `dialog` asks for confirmation before restarting the
 * app, which is when the restore is actually applied. Shared by Configuración >
 * Copia de seguridad and the double-click on a backup file (`App.tsx`). */
export function useRestoreFlow(): { isWorking: boolean; hasPreview: boolean; begin: (path: string) => Promise<void>; dialog: ReactNode } {
  const { t, i18n } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const [isWorking, setIsWorking] = useState(false);
  const [preview, setPreview] = useState<RestorePreview | null>(null);
  // `ConfirmDialog` llama a `onConfirm` y DESPUÉS a `onOpenChange(false)`: este flag
  // distingue "cerró porque confirmó" de "cerró porque se arrepintió".
  const confirmedRef = useRef(false);

  async function begin(path: string) {
    setIsWorking(true);
    try {
      confirmedRef.current = false;
      setPreview(await prepareRestore(path));
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.backup.restoreError"), description: errorMessage(err) });
    } finally {
      setIsWorking(false);
    }
  }

  async function handleConfirm() {
    confirmedRef.current = true;
    try {
      await restartApp();
    } catch (err) {
      confirmedRef.current = false;
      pushToast({ variant: "error", title: t("settings.backup.restoreError"), description: errorMessage(err) });
    }
  }

  function handleOpenChange(isOpen: boolean) {
    if (isOpen) return;
    setPreview(null);
    if (!confirmedRef.current) void cancelRestore();
  }

  const dialog = (
    <ConfirmDialog
      open={preview !== null}
      onOpenChange={handleOpenChange}
      title={t("settings.backup.confirmTitle")}
      description={
        preview
          ? t("settings.backup.confirmDescription", {
              date: formatDateTime(preview.createdAt, i18n.language),
              files: preview.files,
              size: formatBytes(preview.sizeBytes),
            })
          : ""
      }
      confirmLabel={t("settings.backup.confirmAction")}
      cancelLabel={t("common.cancel")}
      danger
      onConfirm={() => void handleConfirm()}
    />
  );

  return { isWorking, hasPreview: preview !== null, begin, dialog };
}
