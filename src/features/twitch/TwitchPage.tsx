import { MessageCircle, Pause, Play, Settings, Square } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/design-system/Badge";
import { Button } from "@/design-system/Button";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/design-system/Dialog";
import { EmptyState } from "@/design-system/EmptyState";
import { Field } from "@/design-system/Field";
import { IconButton } from "@/design-system/IconButton";
import { Input } from "@/design-system/Input";
import { LoadingState } from "@/design-system/LoadingState";
import { TwitchIcon } from "@/design-system/PlatformIcons";
import { Select } from "@/design-system/Select";
import { Switch } from "@/design-system/Switch";
import { errorMessage } from "@/lib/errorMessage";
import { formatTime } from "@/lib/formatDate";
import { useAppEvent } from "@/lib/ipc/events";
import {
  getTwitchEmoteSettings,
  getTwitchEmoteStatus,
  refreshTwitchEmotes,
  setTwitchEmoteSettings,
  startEmoteCaptioning,
  type EmoteCacheStatus,
  type TwitchEmoteSettings,
  type TwitchPolicy,
} from "@/lib/ipc/twitch";
import { cn } from "@/lib/theme/cn";
import { useToastStore } from "@/stores/toastStore";
import { useTwitchStore } from "@/stores/twitchStore";

/** Todos los ajustes de Twitch (cuenta, canal, modo de respuesta, prueba)
 * viven detrás del engranaje — mismo espíritu que `CallSettingsDialog`
 * (CallPage.tsx): pedido del usuario para que el chat en vivo sea lo
 * principal en pantalla, no la configuración. El antiguo interruptor
 * "Responder en el chat" se reemplaza por el botón de pausa de
 * `TwitchPage` — ya no vive acá. */
function TwitchSettingsDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const connection = useTwitchStore((s) => s.connection);
  const policy = useTwitchStore((s) => s.policy);
  const status = useTwitchStore((s) => s.status);
  const isConnecting = useTwitchStore((s) => s.isConnecting);
  const saveTargetChannel = useTwitchStore((s) => s.saveTargetChannel);
  const connectAccount = useTwitchStore((s) => s.connectAccount);
  const disconnectAccount = useTwitchStore((s) => s.disconnectAccount);
  const savePolicy = useTwitchStore((s) => s.savePolicy);
  const sendTestMessage = useTwitchStore((s) => s.sendTestMessage);
  const isSendingTestMessage = useTwitchStore((s) => s.isSendingTestMessage);
  const loadEmoteMap = useTwitchStore((s) => s.loadEmoteMap);

  const [targetChannel, setTargetChannel] = useState(connection?.targetChannelLogin ?? "");
  const [isSavingTargetChannel, setIsSavingTargetChannel] = useState(false);
  const [isSavingPolicy, setIsSavingPolicy] = useState(false);
  const [testText, setTestText] = useState("");

  const [emoteSettings, setEmoteSettingsState] = useState<TwitchEmoteSettings | null>(null);
  const [emoteStatus, setEmoteStatus] = useState<EmoteCacheStatus | null>(null);
  const [isSavingEmoteSettings, setIsSavingEmoteSettings] = useState(false);
  const [isRefreshingEmotes, setIsRefreshingEmotes] = useState(false);

  const [isStartingCaptioning, setIsStartingCaptioning] = useState(false);
  const [captionProgress, setCaptionProgress] = useState<{ captioned: number; total: number } | null>(null);

  useEffect(() => {
    if (open) setTargetChannel(connection?.targetChannelLogin ?? "");
  }, [open, connection?.targetChannelLogin]);

  useEffect(() => {
    if (!open) return;
    getTwitchEmoteSettings().then(setEmoteSettingsState).catch(() => setEmoteSettingsState(null));
    getTwitchEmoteStatus().then(setEmoteStatus).catch(() => setEmoteStatus(null));
  }, [open]);

  useAppEvent((event) => {
    if (event.type === "twitch_emote_caption_progress") {
      setCaptionProgress({ captioned: event.captioned, total: event.total });
    } else if (event.type === "twitch_emote_caption_finished") {
      setCaptionProgress(null);
      setIsStartingCaptioning(false);
      void getTwitchEmoteStatus().then(setEmoteStatus);
    } else if (event.type === "twitch_emote_caption_error") {
      setCaptionProgress(null);
      setIsStartingCaptioning(false);
      pushToast({ variant: "error", title: t("twitch.emotes.generateError"), description: event.message });
    }
  });

  const isConnected = connection?.hasTokens ?? false;
  const isRunning = status === "running" || status === "starting";

  async function handleConnect() {
    try {
      await connectAccount();
      pushToast({ variant: "success", title: t("twitch.connected") });
    } catch (err) {
      pushToast({ variant: "error", title: t("twitch.connectError"), description: errorMessage(err) });
    }
  }

  async function handleDisconnect() {
    try {
      await disconnectAccount();
      pushToast({ variant: "success", title: t("twitch.disconnected") });
    } catch (err) {
      pushToast({ variant: "error", title: t("twitch.disconnectError"), description: errorMessage(err) });
    }
  }

  async function handleSaveTargetChannel() {
    setIsSavingTargetChannel(true);
    try {
      await saveTargetChannel(targetChannel.trim());
      pushToast({ variant: "success", title: t("twitch.targetChannelSaved") });
    } catch (err) {
      pushToast({ variant: "error", title: t("twitch.saveError"), description: errorMessage(err) });
    } finally {
      setIsSavingTargetChannel(false);
    }
  }

  async function updatePolicy(next: TwitchPolicy) {
    setIsSavingPolicy(true);
    try {
      await savePolicy(next);
    } catch (err) {
      pushToast({ variant: "error", title: t("twitch.saveError"), description: errorMessage(err) });
    } finally {
      setIsSavingPolicy(false);
    }
  }

  async function handleSendTest() {
    const trimmed = testText.trim();
    if (!trimmed) return;
    try {
      await sendTestMessage(trimmed);
      setTestText("");
    } catch (err) {
      pushToast({ variant: "error", title: t("twitch.testError"), description: errorMessage(err) });
    }
  }

  async function handleToggleEmotes(enabled: boolean) {
    setIsSavingEmoteSettings(true);
    try {
      const saved = await setTwitchEmoteSettings({ enabled });
      setEmoteSettingsState(saved);
      void loadEmoteMap();
    } catch (err) {
      pushToast({ variant: "error", title: t("twitch.saveError"), description: errorMessage(err) });
    } finally {
      setIsSavingEmoteSettings(false);
    }
  }

  async function handleRefreshEmotes() {
    setIsRefreshingEmotes(true);
    try {
      const status = await refreshTwitchEmotes();
      setEmoteStatus(status);
      void loadEmoteMap();
    } catch (err) {
      pushToast({ variant: "error", title: t("twitch.emotes.refreshError"), description: errorMessage(err) });
    } finally {
      setIsRefreshingEmotes(false);
    }
  }

  async function handleGenerateDescriptions() {
    setIsStartingCaptioning(true);
    try {
      await startEmoteCaptioning();
    } catch (err) {
      setIsStartingCaptioning(false);
      pushToast({ variant: "error", title: t("twitch.emotes.generateError"), description: errorMessage(err) });
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent size="lg">
        <div className="flex items-center justify-between gap-4">
          <DialogTitle className="text-base font-medium text-text-primary">{t("twitch.settingsTitle")}</DialogTitle>
          {isConnected ? (
            <Badge variant="success" className="shrink-0 whitespace-nowrap">
              {t("twitch.connectedAs", { channel: connection?.channelLogin })}
            </Badge>
          ) : (
            <Badge variant="neutral" className="shrink-0 whitespace-nowrap">
              {t("twitch.notConnected")}
            </Badge>
          )}
        </div>

        <div className="mt-4 flex max-h-[70vh] flex-col gap-4 overflow-y-auto">
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-sm font-medium text-text-primary">{t("twitch.connectionTitle")}</p>
                <p className="text-xs text-text-secondary">{t("twitch.connectionDescription")}</p>
              </div>
              {isConnected ? (
                <Button variant="secondary" onClick={() => void handleDisconnect()}>
                  {t("twitch.disconnect")}
                </Button>
              ) : (
                <Button onClick={() => void handleConnect()} disabled={isConnecting}>
                  {isConnecting ? t("twitch.connecting") : t("twitch.connect")}
                </Button>
              )}
            </div>

            {isConnected ? (
              <Field label={t("twitch.targetChannel")} hint={isRunning ? t("twitch.targetChannelRestartNote") : t("twitch.targetChannelHint")}>
                <div className="flex gap-2">
                  <Input
                    value={targetChannel}
                    onChange={(event) => setTargetChannel(event.target.value)}
                    placeholder={t("twitch.targetChannelPlaceholder")}
                    disabled={isRunning}
                  />
                  <Button variant="secondary" onClick={() => void handleSaveTargetChannel()} disabled={isSavingTargetChannel || isRunning}>
                    {isSavingTargetChannel ? t("identity.saving") : t("identity.save")}
                  </Button>
                </div>
              </Field>
            ) : null}
          </div>

          {policy ? (
            <div className="flex flex-col gap-4 border-t border-border pt-4">
              <div>
                <p className="text-sm font-medium text-text-primary">{t("twitch.policyTitle")}</p>
                <p className="text-xs text-text-secondary">{t("twitch.policyDescription")}</p>
              </div>

              <Field
                label={t("twitch.fields.responseMode")}
                hint={policy.mentionedOnly && !policy.respondToAll ? t("twitch.fields.responseModeMentionedHint") : undefined}
              >
                <Select
                  ariaLabel={t("twitch.fields.responseMode")}
                  value={policy.respondToAll ? "all" : "mentioned"}
                  onValueChange={(value) => void updatePolicy({ ...policy, respondToAll: value === "all", mentionedOnly: value === "mentioned" })}
                  options={[
                    { value: "mentioned", label: t("twitch.fields.responseModeMentioned") },
                    { value: "all", label: t("twitch.fields.responseModeAll") },
                  ]}
                />
              </Field>

              <Field label={t("twitch.fields.cooldown")} hint={t("twitch.fields.cooldownHint")}>
                <Input
                  type="number"
                  min={0}
                  value={policy.cooldownSeconds}
                  onChange={(event) => void updatePolicy({ ...policy, cooldownSeconds: Number(event.target.value) || 0 })}
                />
              </Field>

              {isSavingPolicy ? <p className="text-xs text-text-secondary">{t("twitch.saving")}</p> : null}
            </div>
          ) : null}

          <div className="flex flex-col gap-4 border-t border-border pt-4">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-sm font-medium text-text-primary">{t("twitch.emotes.title")}</p>
              </div>
              <Switch
                label={t("twitch.emotes.enable")}
                checked={emoteSettings?.enabled ?? false}
                disabled={isSavingEmoteSettings || !emoteSettings}
                onCheckedChange={(checked) => void handleToggleEmotes(checked)}
              />
            </div>

            {emoteSettings?.enabled ? (
              <>
                <div className="flex items-center justify-between gap-4">
                  <p className="text-xs text-text-secondary">
                    {emoteStatus ? t("twitch.emotes.statusLabel", { total: emoteStatus.total, described: emoteStatus.described }) : ""}
                  </p>
                  <Button size="sm" variant="secondary" onClick={() => void handleRefreshEmotes()} disabled={isRefreshingEmotes}>
                    {isRefreshingEmotes ? t("identity.saving") : t("twitch.emotes.refreshButton")}
                  </Button>
                </div>

                <div className="flex flex-col gap-3 border-t border-border pt-3">
                  <div className="flex items-center justify-between gap-2">
                    <p className="text-xs text-text-secondary">{t("twitch.emotes.visionHint")}</p>
                    <Button
                      size="sm"
                      onClick={() => void handleGenerateDescriptions()}
                      disabled={isStartingCaptioning || captionProgress !== null || !emoteStatus?.total}
                    >
                      {captionProgress
                        ? t("twitch.emotes.generating", { done: captionProgress.captioned, total: captionProgress.total })
                        : t("twitch.emotes.generateButton")}
                    </Button>
                  </div>

                  {captionProgress ? (
                    <div className="h-1.5 w-full overflow-hidden rounded-full bg-surface-secondary">
                      <div
                        className="h-full rounded-full bg-accent transition-[width] duration-200"
                        style={{ width: `${captionProgress.total ? Math.min(100, (captionProgress.captioned / captionProgress.total) * 100) : 0}%` }}
                      />
                    </div>
                  ) : null}
                </div>
              </>
            ) : null}
          </div>

          {isConnected ? (
            <div className="flex flex-col gap-3 border-t border-border pt-4">
              <div>
                <p className="text-sm font-medium text-text-primary">{t("twitch.testTitle")}</p>
                <p className="text-xs text-text-secondary">{t("twitch.testDescription")}</p>
              </div>
              <div className="flex gap-2">
                <Input
                  value={testText}
                  onChange={(event) => setTestText(event.target.value)}
                  placeholder={t("twitch.testPlaceholder")}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") void handleSendTest();
                  }}
                />
                <Button variant="secondary" onClick={() => void handleSendTest()} disabled={isSendingTestMessage || !testText.trim()}>
                  {isSendingTestMessage ? t("twitch.testSending") : t("twitch.testSend")}
                </Button>
              </div>
            </div>
          ) : null}
        </div>

        <div className="mt-5 flex justify-end">
          <DialogClose asChild>
            <Button variant="secondary">{t("common.close")}</Button>
          </DialogClose>
        </div>
      </DialogContent>
    </Dialog>
  );
}

/** Splits `text` on whitespace (kept as its own tokens, so spacing is
 * preserved exactly) and swaps any token that exactly matches a cached
 * 7TV/BTTV/FFZ emote name for its image — same exact-name matching Twitch
 * itself uses (an emote is a whole word, never a substring match: "GAMBA"
 * inside "GAMBAZO" must stay plain text). `emoteMap` is empty whenever the
 * feature is off or nothing's cached yet, so this is a no-op then. */
function renderTwitchMessage(text: string, emoteMap: Record<string, string>) {
  if (Object.keys(emoteMap).length === 0) return text;

  return text.split(/(\s+)/).map((token, index) => {
    const imageUrl = emoteMap[token];
    if (!imageUrl) return token;
    return <img key={index} src={imageUrl} alt={token} title={token} className="inline-block h-6 w-auto align-middle" />;
  });
}

export function TwitchPage() {
  const { t, i18n } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const isLoading = useTwitchStore((s) => s.isLoading);
  const connection = useTwitchStore((s) => s.connection);
  const policy = useTwitchStore((s) => s.policy);
  const status = useTwitchStore((s) => s.status);
  const feed = useTwitchStore((s) => s.feed);
  const emoteMap = useTwitchStore((s) => s.emoteMap);
  const load = useTwitchStore((s) => s.load);
  const start = useTwitchStore((s) => s.start);
  const stop = useTwitchStore((s) => s.stop);
  const savePolicy = useTwitchStore((s) => s.savePolicy);

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [isToggling, setIsToggling] = useState(false);
  const [isTogglingPause, setIsTogglingPause] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ block: "end" });
  }, [feed]);

  if (isLoading || !connection) {
    return <LoadingState label={t("twitch.loading")} />;
  }

  const isConnected = connection.hasTokens;
  const isRunning = status === "running" || status === "starting";
  const canToggle = isConnected && status !== "starting" && status !== "stopping";
  const isPaused = policy ? !policy.enabled : false;

  async function handleToggleConnection() {
    setIsToggling(true);
    try {
      if (isRunning) {
        await stop();
      } else {
        await start();
      }
    } catch (err) {
      pushToast({ variant: "error", title: t("twitch.toggleError"), description: errorMessage(err) });
    } finally {
      setIsToggling(false);
    }
  }

  async function handleTogglePause() {
    if (!policy) return;
    setIsTogglingPause(true);
    try {
      await savePolicy({ ...policy, enabled: !policy.enabled });
    } catch (err) {
      pushToast({ variant: "error", title: t("twitch.saveError"), description: errorMessage(err) });
    } finally {
      setIsTogglingPause(false);
    }
  }

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex items-center gap-3 border-b border-border px-4 py-3">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-full bg-surface-secondary text-text-secondary">
          {isConnected && connection.profileImageUrl ? (
            <img src={connection.profileImageUrl} alt="" className="h-full w-full object-cover" />
          ) : (
            <TwitchIcon className="h-4 w-4" />
          )}
        </div>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium text-text-primary">
            {isConnected ? t("twitch.connectedAs", { channel: connection.channelLogin }) : t("twitch.notConnected")}
          </p>
          {status ? (
            <p className={cn("text-xs", status === "error" ? "text-error" : "text-text-secondary")}>{t(`twitch.status.${status}`)}</p>
          ) : null}
        </div>
        <IconButton
          icon={<Settings className="h-4 w-4" />}
          label={t("twitch.settingsTitle")}
          title={t("twitch.settingsTitle")}
          onClick={() => setSettingsOpen(true)}
        />
      </header>

      <div className="flex-1 overflow-y-auto px-4 py-4">
        {!isConnected ? (
          <EmptyState icon={<MessageCircle className="h-6 w-6" />} title={t("twitch.emptyFeedTitle")} description={t("twitch.emptyFeedDescription")} />
        ) : feed.length === 0 ? (
          <div className="flex h-full items-center justify-center">
            <p className="text-sm text-text-secondary">{t("twitch.feedEmpty")}</p>
          </div>
        ) : (
          <div className="mx-auto flex max-w-2xl flex-col gap-1.5">
            {feed.map((message) => (
              <div key={message.id} className="text-sm leading-relaxed">
                <span className="mr-1.5 text-xs text-text-secondary">{formatTime(message.at, i18n.language)}</span>
                <span className={message.from === "character" ? "text-accent" : "font-medium text-text-primary"}>{message.displayName}: </span>
                <span className="text-text-secondary">{renderTwitchMessage(message.text, emoteMap)}</span>
              </div>
            ))}
            <div ref={bottomRef} />
          </div>
        )}
      </div>

      <div className="border-t border-border px-4 py-6">
        <div className="mx-auto flex max-w-2xl flex-col items-center gap-3">
          {isRunning ? (
            <>
              <p className="flex items-center gap-1.5 text-xs font-medium text-text-secondary">
                <span
                  className={cn(
                    "h-1.5 w-1.5 rounded-full",
                    isPaused ? "bg-warning" : status === "running" ? "animate-pulse bg-accent" : "bg-text-secondary",
                  )}
                />
                {isPaused ? t("twitch.pausedStatus") : t(`twitch.status.${status}`)}
              </p>
              <div className="flex items-center gap-5">
                <IconButton
                  icon={isPaused ? <Play className="h-4 w-4" /> : <Pause className="h-4 w-4" />}
                  label={isPaused ? t("twitch.resumeResponses") : t("twitch.pauseResponses")}
                  title={isPaused ? t("twitch.resumeResponses") : t("twitch.pauseResponses")}
                  active={isPaused}
                  disabled={isTogglingPause || !policy}
                  onClick={() => void handleTogglePause()}
                />
                <button
                  type="button"
                  onClick={() => void handleToggleConnection()}
                  disabled={isToggling}
                  aria-label={t("twitch.stopListening")}
                  title={t("twitch.stopListening")}
                  className="flex h-16 w-16 items-center justify-center rounded-full bg-error text-error-foreground shadow-md transition-transform duration-150 hover:scale-105 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  <Square className="h-6 w-6" />
                </button>
              </div>
            </>
          ) : (
            <button
              type="button"
              onClick={() => void handleToggleConnection()}
              disabled={isToggling || !canToggle}
              aria-label={t("twitch.startListening")}
              title={t("twitch.startListening")}
              className="flex h-16 w-16 items-center justify-center rounded-full bg-accent text-accent-foreground shadow-md transition-transform duration-150 hover:scale-105 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Play className="h-6 w-6" />
            </button>
          )}
        </div>
      </div>

      <TwitchSettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    </div>
  );
}
