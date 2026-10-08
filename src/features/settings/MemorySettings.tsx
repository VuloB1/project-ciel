import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/design-system/Badge";
import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { Dialog } from "@/design-system/Dialog";
import { LoadingState } from "@/design-system/LoadingState";
import { deleteEmbeddingModel, downloadEmbeddingModel, getEmbeddingModelStatus } from "@/lib/ipc/embedding";
import { useAppEvent } from "@/lib/ipc/events";
import { backfillMemoryEmbeddings } from "@/lib/ipc/memory";
import { errorMessage } from "@/lib/errorMessage";
import { useProfileStore } from "@/stores/profileStore";
import { useToastStore } from "@/stores/toastStore";

import { MemoryFusionDialog } from "./MemoryFusionDialog";

interface Progress {
  downloadedBytes: number;
  totalBytes: number | null;
}

interface BackfillProgress {
  embedded: number;
  total: number;
}

/** Single-model version of `SttSettings.tsx` — no size grid, no GPU
 * toggle, because there's only ever one embedding model to download.
 * "Downloaded" IS the opt-in — no separate settings toggle, matching
 * STT's own UX. */
export function MemorySettings() {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const activeProfileId = useProfileStore((s) => s.activeProfileId);

  const [isLoading, setIsLoading] = useState(true);
  const [downloaded, setDownloaded] = useState(false);
  const [approxSizeMb, setApproxSizeMb] = useState(0);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [backfillProgress, setBackfillProgress] = useState<BackfillProgress | null>(null);
  const [isFusionOpen, setIsFusionOpen] = useState(false);

  useEffect(() => {
    getEmbeddingModelStatus()
      .then((status) => {
        setDownloaded(status.downloaded);
        setApproxSizeMb(status.approxSizeMb);
        setIsLoading(false);
      })
      .catch(() => setIsLoading(false));
  }, []);

  useAppEvent((event) => {
    if (event.type === "embedding_model_download_progress") {
      setProgress({ downloadedBytes: event.downloadedBytes, totalBytes: event.totalBytes });
    } else if (event.type === "embedding_model_download_finished") {
      setProgress(null);
      setDownloaded(true);
    } else if (event.type === "embedding_model_download_error") {
      setProgress(null);
      pushToast({ variant: "error", title: t("settings.memory.downloadError"), description: event.message });
    } else if (event.type === "embedding_backfill_progress" && event.profileId === activeProfileId) {
      setBackfillProgress({ embedded: event.embedded, total: event.total });
    } else if (event.type === "embedding_backfill_finished" && event.profileId === activeProfileId) {
      setBackfillProgress(null);
    } else if (event.type === "embedding_backfill_error" && event.profileId === activeProfileId) {
      setBackfillProgress(null);
      pushToast({ variant: "error", title: t("settings.memory.backfillError"), description: event.message });
    }
  });

  async function handleDownload() {
    setProgress({ downloadedBytes: 0, totalBytes: null });
    try {
      await downloadEmbeddingModel();
    } catch (err) {
      setProgress(null);
      pushToast({ variant: "error", title: t("settings.memory.downloadError"), description: errorMessage(err) });
    }
  }

  async function handleDelete() {
    setIsBusy(true);
    try {
      await deleteEmbeddingModel();
      setDownloaded(false);
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.memory.downloadError"), description: errorMessage(err) });
    } finally {
      setIsBusy(false);
    }
  }

  async function handleBackfill() {
    if (!activeProfileId) return;
    setBackfillProgress({ embedded: 0, total: 0 });
    try {
      await backfillMemoryEmbeddings(activeProfileId);
    } catch (err) {
      setBackfillProgress(null);
      pushToast({ variant: "error", title: t("settings.memory.backfillError"), description: errorMessage(err) });
    }
  }

  if (isLoading) {
    return <LoadingState label={t("settings.memory.loading")} />;
  }

  return (
    <div className="flex max-w-xl flex-col gap-4">
      <Card className="flex flex-col gap-4 p-4">
        <div>
          <p className="text-sm font-medium text-text-primary">{t("settings.memory.title")}</p>
          <p className="text-xs text-text-secondary">{t("settings.memory.description")}</p>
        </div>

        <div className="flex flex-col gap-3">
          <div className="flex items-start justify-between gap-4">
            <div className="flex items-center gap-2">
              <span className="text-xs text-text-secondary">{t("settings.memory.approxSize", { size: approxSizeMb })}</span>
              {downloaded && <Badge variant="success">{t("settings.memory.downloaded")}</Badge>}
            </div>

            <div className="flex shrink-0 gap-2">
              {!downloaded && !progress && (
                <Button size="sm" onClick={() => void handleDownload()}>
                  {t("settings.memory.download")}
                </Button>
              )}
              {downloaded && (
                <Button size="sm" variant="ghost" disabled={isBusy} onClick={() => void handleDelete()}>
                  {t("settings.memory.delete")}
                </Button>
              )}
            </div>
          </div>

          {progress && (
            <div className="flex flex-col gap-1">
              <div className="h-1.5 w-full overflow-hidden rounded-full bg-surface-secondary">
                <div
                  className="h-full rounded-full bg-accent transition-[width] duration-200"
                  style={{
                    width: progress.totalBytes ? `${Math.min(100, (progress.downloadedBytes / progress.totalBytes) * 100)}%` : "30%",
                  }}
                />
              </div>
              <p className="text-xs text-text-secondary">
                {t("settings.memory.downloading")}{" "}
                {progress.totalBytes
                  ? `${(progress.downloadedBytes / 1_000_000).toFixed(0)} / ${(progress.totalBytes / 1_000_000).toFixed(0)} MB`
                  : `${(progress.downloadedBytes / 1_000_000).toFixed(0)} MB`}
              </p>
            </div>
          )}
        </div>

        {downloaded && (
          <div className="flex flex-col gap-3 border-t border-border pt-4">
            <div className="flex items-center justify-between gap-4">
              <p className="text-xs text-text-secondary">{t("settings.memory.backfillHint")}</p>
              <Button
                size="sm"
                variant="secondary"
                className="shrink-0 whitespace-nowrap"
                disabled={!activeProfileId || backfillProgress !== null}
                onClick={() => void handleBackfill()}
              >
                {t("settings.memory.backfill")}
              </Button>
            </div>

            {backfillProgress && (
              <div className="flex flex-col gap-1">
                <div className="h-1.5 w-full overflow-hidden rounded-full bg-surface-secondary">
                  <div
                    className="h-full rounded-full bg-accent transition-[width] duration-200"
                    style={{
                      width: backfillProgress.total ? `${Math.min(100, (backfillProgress.embedded / backfillProgress.total) * 100)}%` : "30%",
                    }}
                  />
                </div>
                <p className="text-xs text-text-secondary">
                  {t("settings.memory.backfilling")}{" "}
                  {t("settings.memory.backfillProgress", { embedded: backfillProgress.embedded, total: backfillProgress.total })}
                </p>
              </div>
            )}
          </div>
        )}

        <div className="flex items-center justify-between gap-4 border-t border-border pt-4">
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.memory.fusionTitle")}</p>
            <p className="text-xs text-text-secondary">{t("settings.memory.fusionHint")}</p>
          </div>
          <Dialog open={isFusionOpen} onOpenChange={setIsFusionOpen}>
            <Button size="sm" variant="secondary" className="shrink-0" disabled={!activeProfileId} onClick={() => setIsFusionOpen(true)}>
              {t("settings.memory.fusionButton")}
            </Button>
            {isFusionOpen && activeProfileId ? (
              <MemoryFusionDialog profileId={activeProfileId} onClose={() => setIsFusionOpen(false)} />
            ) : null}
          </Dialog>
        </div>
      </Card>
    </div>
  );
}
