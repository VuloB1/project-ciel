import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/design-system/Badge";
import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { LoadingState } from "@/design-system/LoadingState";
import { Switch } from "@/design-system/Switch";
import { errorMessage } from "@/lib/errorMessage";
import { useAppEvent } from "@/lib/ipc/events";
import {
  deleteSttModel,
  downloadSttModel,
  getDictationSettings,
  getSttModel,
  listSttModelStatus,
  setDictationSettings,
  setSttModel,
  type SttModelSize,
  type SttModelStatus,
} from "@/lib/ipc/stt";
import { useToastStore } from "@/stores/toastStore";

const SIZES: SttModelSize[] = ["tiny", "base", "small", "medium"];

interface Progress {
  downloadedBytes: number;
  totalBytes: number | null;
}

export function SttSettings() {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);

  const [isLoading, setIsLoading] = useState(true);
  const [selectedSize, setSelectedSize] = useState<SttModelSize | null>(null);
  const [gpuEnabled, setGpuEnabled] = useState(false);
  const [isSavingGpu, setIsSavingGpu] = useState(false);
  const [autoSendDictation, setAutoSendDictation] = useState(false);
  const [statuses, setStatuses] = useState<Record<SttModelSize, boolean>>({} as Record<SttModelSize, boolean>);
  const [progress, setProgress] = useState<Partial<Record<SttModelSize, Progress>>>({});
  const [busySize, setBusySize] = useState<SttModelSize | null>(null);

  async function refreshStatuses() {
    const list: SttModelStatus[] = await listSttModelStatus();
    setStatuses(Object.fromEntries(list.map((s) => [s.size, s.downloaded])) as Record<SttModelSize, boolean>);
  }

  useEffect(() => {
    getDictationSettings()
      .then((settings) => setAutoSendDictation(settings.autoSend))
      .catch(() => undefined);
    Promise.all([getSttModel(), listSttModelStatus()])
      .then(([model, list]) => {
        setSelectedSize(model.size);
        setGpuEnabled(model.gpuEnabled);
        setStatuses(Object.fromEntries(list.map((s) => [s.size, s.downloaded])) as Record<SttModelSize, boolean>);
        setIsLoading(false);
      })
      .catch(() => setIsLoading(false));
  }, []);

  useAppEvent((event) => {
    if (event.type === "stt_model_download_progress") {
      const size = event.modelSize as SttModelSize;
      setProgress((prev) => ({ ...prev, [size]: { downloadedBytes: event.downloadedBytes, totalBytes: event.totalBytes } }));
    } else if (event.type === "stt_model_download_finished") {
      const size = event.modelSize as SttModelSize;
      setProgress((prev) => {
        const next = { ...prev };
        delete next[size];
        return next;
      });
      setStatuses((prev) => ({ ...prev, [size]: true }));
    } else if (event.type === "stt_model_download_error") {
      const size = event.modelSize as SttModelSize;
      setProgress((prev) => {
        const next = { ...prev };
        delete next[size];
        return next;
      });
      pushToast({ variant: "error", title: t("settings.stt.downloadError"), description: event.message });
    }
  });

  async function handleDownload(size: SttModelSize) {
    setProgress((prev) => ({ ...prev, [size]: { downloadedBytes: 0, totalBytes: null } }));
    try {
      await downloadSttModel(size);
    } catch (err) {
      setProgress((prev) => {
        const next = { ...prev };
        delete next[size];
        return next;
      });
      pushToast({ variant: "error", title: t("settings.stt.downloadError"), description: errorMessage(err) });
    }
  }

  async function handleSelect(size: SttModelSize) {
    setBusySize(size);
    try {
      const saved = await setSttModel(size, gpuEnabled);
      setSelectedSize(saved.size);
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.stt.saveError"), description: errorMessage(err) });
    } finally {
      setBusySize(null);
    }
  }

  async function handleToggleGpu(next: boolean) {
    if (!selectedSize) return;
    setGpuEnabled(next);
    setIsSavingGpu(true);
    try {
      await setSttModel(selectedSize, next);
    } catch (err) {
      setGpuEnabled(!next);
      pushToast({ variant: "error", title: t("settings.stt.saveError"), description: errorMessage(err) });
    } finally {
      setIsSavingGpu(false);
    }
  }

  async function handleDelete(size: SttModelSize) {
    setBusySize(size);
    try {
      await deleteSttModel(size);
      await refreshStatuses();
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.stt.downloadError"), description: errorMessage(err) });
    } finally {
      setBusySize(null);
    }
  }

  if (isLoading) {
    return <LoadingState label={t("settings.stt.loading")} />;
  }

  return (
    <div className="flex max-w-xl flex-col gap-4">
      <Card className="flex flex-col gap-4 p-4">
        <div>
          <p className="text-sm font-medium text-text-primary">{t("settings.stt.title")}</p>
          <p className="text-xs text-text-secondary">{t("settings.stt.description")}</p>
        </div>

        <div className="flex items-center justify-between gap-4 border-t border-border pt-4">
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.stt.gpuToggle")}</p>
            <p className="mt-1 text-xs text-text-secondary">{t("settings.stt.gpuToggleHint")}</p>
          </div>
          <Switch checked={gpuEnabled} onCheckedChange={(next) => void handleToggleGpu(next)} disabled={isSavingGpu} label={t("settings.stt.gpuToggle")} />
        </div>

        <div className="flex items-center justify-between gap-4 border-t border-border pt-4">
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.stt.dictationAutoSend")}</p>
            <p className="mt-1 text-xs text-text-secondary">{t("settings.stt.dictationAutoSendHint")}</p>
          </div>
          <Switch
            checked={autoSendDictation}
            onCheckedChange={(next) => {
              setAutoSendDictation(next);
              void setDictationSettings({ autoSend: next }).catch(() => setAutoSendDictation(!next));
            }}
            label={t("settings.stt.dictationAutoSend")}
          />
        </div>

        <div className="flex flex-col divide-y divide-border border-t border-border">
          {SIZES.map((size) => {
            const downloaded = statuses[size] ?? false;
            const isActive = selectedSize === size;
            const inProgress = progress[size];
            const isBusy = busySize === size;
            const approxSizeMb = { tiny: 75, base: 142, small: 466, medium: 1500 }[size];

            return (
              <div key={size} className="flex flex-col gap-3 py-3.5 first:pt-4">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <div className="flex items-center gap-2">
                      <p className="text-sm font-medium text-text-primary">{t(`settings.stt.sizeLabel.${size}`)}</p>
                      <span className="text-xs text-text-secondary">{t("settings.stt.approxSize", { size: approxSizeMb })}</span>
                      {isActive && <Badge variant="success">{t("settings.stt.active")}</Badge>}
                      {!isActive && downloaded && <Badge variant="neutral">{t("settings.stt.downloaded")}</Badge>}
                    </div>
                    <p className="mt-1 text-xs text-text-secondary">{t(`settings.stt.sizeHint.${size}`)}</p>
                  </div>

                  <div className="flex shrink-0 gap-2">
                    {!downloaded && !inProgress && (
                      <Button size="sm" onClick={() => void handleDownload(size)}>
                        {t("settings.stt.download")}
                      </Button>
                    )}
                    {downloaded && !isActive && (
                      <Button size="sm" variant="secondary" disabled={isBusy} onClick={() => void handleSelect(size)}>
                        {t("settings.stt.select")}
                      </Button>
                    )}
                    {downloaded && !isActive && (
                      <Button size="sm" variant="ghost" disabled={isBusy} onClick={() => void handleDelete(size)}>
                        {t("settings.stt.delete")}
                      </Button>
                    )}
                  </div>
                </div>

                {inProgress && (
                  <div className="flex flex-col gap-1">
                    <div className="h-1.5 w-full overflow-hidden rounded-full bg-surface-secondary">
                      <div
                        className="h-full rounded-full bg-accent transition-[width] duration-200"
                        style={{
                          width: inProgress.totalBytes
                            ? `${Math.min(100, (inProgress.downloadedBytes / inProgress.totalBytes) * 100)}%`
                            : "30%",
                        }}
                      />
                    </div>
                    <p className="text-xs text-text-secondary">
                      {t("settings.stt.downloading")}{" "}
                      {inProgress.totalBytes
                        ? `${(inProgress.downloadedBytes / 1_000_000).toFixed(0)} / ${(inProgress.totalBytes / 1_000_000).toFixed(0)} MB`
                        : `${(inProgress.downloadedBytes / 1_000_000).toFixed(0)} MB`}
                    </p>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </Card>
    </div>
  );
}
