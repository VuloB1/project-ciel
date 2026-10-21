import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { Field } from "@/design-system/Field";
import { Input } from "@/design-system/Input";
import { LoadingState } from "@/design-system/LoadingState";
import { Switch } from "@/design-system/Switch";
import { errorMessage } from "@/lib/errorMessage";
import { useAppEvent } from "@/lib/ipc/events";
import {
  deleteObscura,
  downloadObscura,
  getObscuraStatus,
  getWebSearchSettings,
  setWebSearchSettings,
  type ObscuraStatus,
  type WebSearchSettings,
} from "@/lib/ipc/webSearch";
import { useToastStore } from "@/stores/toastStore";

/** Fase 10b — lets the character call a `web_search` tool (Brave Search
 * API) mid-response for things like weather or the current time elsewhere.
 * Opt-in and off by default (see `WebSearchSettings`'s Rust doc comment):
 * enabling it is the explicit signal that Chat/Twitch/Discord/Llamada may
 * need network access for this. */
export function WebSearchSettings() {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);

  const [isLoading, setIsLoading] = useState(true);
  const [settings, setSettings] = useState<WebSearchSettings | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [obscura, setObscura] = useState<ObscuraStatus | null>(null);
  const [download, setDownload] = useState<{ downloadedBytes: number; totalBytes: number | null } | null>(null);

  useEffect(() => {
    getWebSearchSettings()
      .then((loaded) => {
        setSettings(loaded);
        setIsLoading(false);
      })
      .catch(() => setIsLoading(false));
    getObscuraStatus().then(setObscura).catch(() => undefined);
  }, []);

  useAppEvent((event) => {
    if (event.type === "obscura_download_progress") {
      setDownload({ downloadedBytes: event.downloadedBytes, totalBytes: event.totalBytes });
    } else if (event.type === "obscura_download_finished") {
      setDownload(null);
      setObscura((prev) => (prev ? { ...prev, installed: true } : prev));
    } else if (event.type === "obscura_download_error") {
      setDownload(null);
      pushToast({ variant: "error", title: t("settings.webSearch.readPages.downloadError"), description: event.message });
    }
  });

  async function handleReadPages(next: boolean) {
    if (!settings) return;
    setIsSaving(true);
    try {
      setSettings(await setWebSearchSettings(settings.enabled, undefined, next));
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.webSearch.saveError"), description: errorMessage(err) });
    } finally {
      setIsSaving(false);
    }
  }

  async function handleDownload() {
    setDownload({ downloadedBytes: 0, totalBytes: null });
    try {
      await downloadObscura();
    } catch (err) {
      setDownload(null);
      pushToast({ variant: "error", title: t("settings.webSearch.readPages.downloadError"), description: errorMessage(err) });
    }
  }

  async function handleRemoveObscura() {
    try {
      await deleteObscura();
      setObscura((prev) => (prev ? { ...prev, installed: false } : prev));
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.webSearch.readPages.removeError"), description: errorMessage(err) });
    }
  }

  async function handleToggle(enabled: boolean) {
    if (!settings) return;
    setIsSaving(true);
    try {
      const saved = await setWebSearchSettings(enabled);
      setSettings(saved);
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.webSearch.saveError"), description: errorMessage(err) });
    } finally {
      setIsSaving(false);
    }
  }

  async function handleSaveKey() {
    if (!settings) return;
    setIsSaving(true);
    try {
      const saved = await setWebSearchSettings(settings.enabled, apiKey === "" ? undefined : apiKey);
      setSettings(saved);
      setApiKey("");
      pushToast({ variant: "success", title: t("settings.webSearch.saved") });
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.webSearch.saveError"), description: errorMessage(err) });
    } finally {
      setIsSaving(false);
    }
  }

  if (isLoading || !settings) {
    return <LoadingState label={t("settings.webSearch.loading")} />;
  }

  return (
    <div className="flex max-w-xl flex-col gap-4">
      <Card className="flex flex-col gap-4 p-4">
        <div className="flex items-center justify-between gap-4">
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.webSearch.title")}</p>
            <p className="text-xs text-text-secondary">{t("settings.webSearch.description")}</p>
          </div>
          <Switch
            checked={settings.enabled}
            onCheckedChange={(next) => void handleToggle(next)}
            disabled={isSaving}
            label={t("settings.webSearch.enable")}
          />
        </div>

        {settings.enabled ? <p className="text-xs text-text-secondary">{t("settings.webSearch.networkHint")}</p> : null}

        <Field label={t("settings.webSearch.apiKey")} hint={settings.hasApiKey ? t("settings.webSearch.apiKeyStored") : t("settings.webSearch.apiKeyHint")}>
          <div className="flex gap-2">
            <Input
              type="password"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              placeholder={settings.hasApiKey ? "••••••••" : t("settings.webSearch.apiKeyPlaceholder")}
            />
            <Button variant="secondary" onClick={() => void handleSaveKey()} disabled={isSaving}>
              {isSaving ? t("identity.saving") : t("identity.save")}
            </Button>
          </div>
        </Field>

        {obscura?.supported ? (
          <div className="flex flex-col gap-3 border-t border-border pt-4">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-sm font-medium text-text-primary">{t("settings.webSearch.readPages.title")}</p>
                <p className="text-xs text-text-secondary">{t("settings.webSearch.readPages.description")}</p>
              </div>
              <Switch
                checked={settings.readPages}
                onCheckedChange={(next) => void handleReadPages(next)}
                disabled={isSaving}
                label={t("settings.webSearch.readPages.enable")}
              />
            </div>

            {settings.readPages && !obscura.installed ? (
              <div className="flex flex-col gap-2">
                {download ? (
                  <div className="flex flex-col gap-1">
                    <div className="h-1.5 w-full overflow-hidden rounded-full bg-surface-secondary">
                      <div
                        className="h-full rounded-full bg-accent transition-[width] duration-200"
                        style={{ width: download.totalBytes ? `${Math.min(100, (download.downloadedBytes / download.totalBytes) * 100)}%` : "30%" }}
                      />
                    </div>
                    <p className="text-xs text-text-secondary">
                      {t("settings.webSearch.readPages.downloading")} {(download.downloadedBytes / 1_000_000).toFixed(0)} MB
                    </p>
                  </div>
                ) : (
                  <div className="flex items-center justify-between gap-4">
                    <p className="text-xs text-text-secondary">{t("settings.webSearch.readPages.needsDownload", { size: obscura.approxSizeMb })}</p>
                    <Button size="sm" onClick={() => void handleDownload()}>
                      {t("settings.webSearch.readPages.download")}
                    </Button>
                  </div>
                )}
              </div>
            ) : null}

            {obscura.installed ? (
              <div className="flex items-center justify-between gap-4">
                <p className="text-xs text-text-secondary">{t("settings.webSearch.readPages.installed")}</p>
                <Button size="sm" variant="ghost" onClick={() => void handleRemoveObscura()}>
                  {t("settings.webSearch.readPages.remove")}
                </Button>
              </div>
            ) : null}
          </div>
        ) : null}
      </Card>
    </div>
  );
}
