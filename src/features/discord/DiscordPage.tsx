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
import { DiscordIcon } from "@/design-system/PlatformIcons";
import { Select } from "@/design-system/Select";
import { errorMessage } from "@/lib/errorMessage";
import { formatTime } from "@/lib/formatDate";
import type { DiscordPolicy } from "@/lib/ipc/discord";
import { cn } from "@/lib/theme/cn";
import { useDiscordStore } from "@/stores/discordStore";
import { useToastStore } from "@/stores/toastStore";

/** Todos los ajustes de Discord (bot, modo de respuesta, prueba) viven
 * detrás del engranaje — mismo espíritu que `CallSettingsDialog`
 * (CallPage.tsx) y su equivalente en Twitch: pedido del usuario para que
 * el chat en vivo sea lo principal en pantalla, no la configuración. El
 * antiguo interruptor "Responder en Discord" se reemplaza por el botón
 * de pausa de `DiscordPage` — ya no vive acá. */
function DiscordSettingsDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const connection = useDiscordStore((s) => s.connection);
  const policy = useDiscordStore((s) => s.policy);
  const isConnecting = useDiscordStore((s) => s.isConnecting);
  const connectBot = useDiscordStore((s) => s.connectBot);
  const disconnectAccount = useDiscordStore((s) => s.disconnectAccount);
  const savePolicy = useDiscordStore((s) => s.savePolicy);
  const sendTestMessage = useDiscordStore((s) => s.sendTestMessage);
  const isSendingTestMessage = useDiscordStore((s) => s.isSendingTestMessage);

  const [token, setToken] = useState("");
  const [isSavingPolicy, setIsSavingPolicy] = useState(false);
  const [testText, setTestText] = useState("");

  const isConnected = connection?.hasToken ?? false;

  async function handleConnect() {
    const trimmed = token.trim();
    if (!trimmed) return;
    try {
      await connectBot(trimmed);
      setToken("");
      pushToast({ variant: "success", title: t("discord.connected") });
    } catch (err) {
      pushToast({ variant: "error", title: t("discord.connectError"), description: errorMessage(err) });
    }
  }

  async function handleDisconnect() {
    try {
      await disconnectAccount();
      pushToast({ variant: "success", title: t("discord.disconnected") });
    } catch (err) {
      pushToast({ variant: "error", title: t("discord.disconnectError"), description: errorMessage(err) });
    }
  }

  async function updatePolicy(next: DiscordPolicy) {
    setIsSavingPolicy(true);
    try {
      await savePolicy(next);
    } catch (err) {
      pushToast({ variant: "error", title: t("discord.saveError"), description: errorMessage(err) });
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
      pushToast({ variant: "error", title: t("discord.testError"), description: errorMessage(err) });
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent size="lg">
        <div className="flex items-center justify-between gap-4">
          <DialogTitle className="text-base font-medium text-text-primary">{t("discord.settingsTitle")}</DialogTitle>
          {isConnected ? (
            <Badge variant="success" className="shrink-0 whitespace-nowrap">
              {t("discord.connectedAs", { bot: connection?.botUsername })}
            </Badge>
          ) : (
            <Badge variant="neutral" className="shrink-0 whitespace-nowrap">
              {t("discord.notConnected")}
            </Badge>
          )}
        </div>

        <div className="-mx-1 mt-4 flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-1">
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-sm font-medium text-text-primary">{t("discord.connectionTitle")}</p>
                <p className="text-xs text-text-secondary">{t("discord.connectionDescription")}</p>
              </div>
              {isConnected ? (
                <Button variant="secondary" onClick={() => void handleDisconnect()}>
                  {t("discord.disconnect")}
                </Button>
              ) : null}
            </div>

            {isConnected ? null : (
              <Field label={t("discord.botToken")} hint={t("discord.botTokenHint")}>
                <div className="flex gap-2">
                  <Input
                    type="password"
                    value={token}
                    onChange={(event) => setToken(event.target.value)}
                    placeholder={t("discord.botTokenPlaceholder")}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") void handleConnect();
                    }}
                  />
                  <Button onClick={() => void handleConnect()} disabled={isConnecting || !token.trim()}>
                    {isConnecting ? t("discord.connecting") : t("discord.connect")}
                  </Button>
                </div>
              </Field>
            )}
          </div>

          {policy ? (
            <div className="flex flex-col gap-4 border-t border-border pt-4">
              <div>
                <p className="text-sm font-medium text-text-primary">{t("discord.policyTitle")}</p>
                <p className="text-xs text-text-secondary">{t("discord.policyDescription")}</p>
              </div>

              <Field
                label={t("discord.fields.responseMode")}
                hint={policy.mentionedOnly && !policy.respondToAll ? t("discord.fields.responseModeMentionedHint") : undefined}
              >
                <Select
                  ariaLabel={t("discord.fields.responseMode")}
                  value={policy.respondToAll ? "all" : "mentioned"}
                  onValueChange={(value) => void updatePolicy({ ...policy, respondToAll: value === "all", mentionedOnly: value === "mentioned" })}
                  options={[
                    { value: "mentioned", label: t("discord.fields.responseModeMentioned") },
                    { value: "all", label: t("discord.fields.responseModeAll") },
                  ]}
                />
              </Field>

              <Field label={t("discord.fields.cooldown")} hint={t("discord.fields.cooldownHint")}>
                <Input
                  type="number"
                  min={0}
                  value={policy.cooldownSeconds}
                  onChange={(event) => void updatePolicy({ ...policy, cooldownSeconds: Number(event.target.value) || 0 })}
                />
              </Field>

              {isSavingPolicy ? <p className="text-xs text-text-secondary">{t("discord.saving")}</p> : null}
            </div>
          ) : null}

          {isConnected ? (
            <div className="flex flex-col gap-3 border-t border-border pt-4">
              <div>
                <p className="text-sm font-medium text-text-primary">{t("discord.testTitle")}</p>
                <p className="text-xs text-text-secondary">{t("discord.testDescription")}</p>
              </div>
              <div className="flex gap-2">
                <Input
                  value={testText}
                  onChange={(event) => setTestText(event.target.value)}
                  placeholder={t("discord.testPlaceholder")}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") void handleSendTest();
                  }}
                />
                <Button variant="secondary" onClick={() => void handleSendTest()} disabled={isSendingTestMessage || !testText.trim()}>
                  {isSendingTestMessage ? t("discord.testSending") : t("discord.testSend")}
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

export function DiscordPage() {
  const { t, i18n } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const isLoading = useDiscordStore((s) => s.isLoading);
  const connection = useDiscordStore((s) => s.connection);
  const policy = useDiscordStore((s) => s.policy);
  const status = useDiscordStore((s) => s.status);
  const feed = useDiscordStore((s) => s.feed);
  const load = useDiscordStore((s) => s.load);
  const start = useDiscordStore((s) => s.start);
  const stop = useDiscordStore((s) => s.stop);
  const savePolicy = useDiscordStore((s) => s.savePolicy);

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
    return <LoadingState label={t("discord.loading")} />;
  }

  const isConnected = connection.hasToken;
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
      pushToast({ variant: "error", title: t("discord.toggleError"), description: errorMessage(err) });
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
      pushToast({ variant: "error", title: t("discord.saveError"), description: errorMessage(err) });
    } finally {
      setIsTogglingPause(false);
    }
  }

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex items-center gap-3 border-b border-border px-4 py-3">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-full bg-surface-secondary text-text-secondary">
          {isConnected && connection.avatarUrl ? (
            <img src={connection.avatarUrl} alt="" className="h-full w-full object-cover" />
          ) : (
            <DiscordIcon className="h-4 w-4" />
          )}
        </div>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium text-text-primary">
            {isConnected ? t("discord.connectedAs", { bot: connection.botUsername }) : t("discord.notConnected")}
          </p>
          {status ? (
            <p className={cn("text-xs", status === "error" ? "text-error" : "text-text-secondary")}>{t(`discord.status.${status}`)}</p>
          ) : null}
        </div>
        <IconButton
          icon={<Settings className="h-4 w-4" />}
          label={t("discord.settingsTitle")}
          title={t("discord.settingsTitle")}
          onClick={() => setSettingsOpen(true)}
        />
      </header>

      <div className="flex-1 overflow-y-auto px-4 py-4">
        {!isConnected ? (
          <EmptyState icon={<MessageCircle className="h-6 w-6" />} title={t("discord.emptyFeedTitle")} description={t("discord.emptyFeedDescription")} />
        ) : feed.length === 0 ? (
          <div className="flex h-full items-center justify-center">
            <p className="text-sm text-text-secondary">{t("discord.feedEmpty")}</p>
          </div>
        ) : (
          <div className="mx-auto flex max-w-2xl flex-col gap-1.5">
            {feed.map((message) => (
              <div key={message.id} className="text-sm leading-relaxed">
                <span className="mr-1.5 text-xs text-text-secondary">{formatTime(message.at, i18n.language)}</span>
                <span className={message.from === "character" ? "text-accent" : "font-medium text-text-primary"}>{message.displayName}: </span>
                <span className="text-text-secondary">{message.text}</span>
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
                {isPaused ? t("discord.pausedStatus") : t(`discord.status.${status}`)}
              </p>
              <div className="flex items-center gap-5">
                <IconButton
                  icon={isPaused ? <Play className="h-4 w-4" /> : <Pause className="h-4 w-4" />}
                  label={isPaused ? t("discord.resumeResponses") : t("discord.pauseResponses")}
                  title={isPaused ? t("discord.resumeResponses") : t("discord.pauseResponses")}
                  active={isPaused}
                  disabled={isTogglingPause || !policy}
                  onClick={() => void handleTogglePause()}
                />
                <button
                  type="button"
                  onClick={() => void handleToggleConnection()}
                  disabled={isToggling}
                  aria-label={t("discord.stopListening")}
                  title={t("discord.stopListening")}
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
              aria-label={t("discord.startListening")}
              title={t("discord.startListening")}
              className="flex h-16 w-16 items-center justify-center rounded-full bg-accent text-accent-foreground shadow-md transition-transform duration-150 hover:scale-105 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Play className="h-6 w-6" />
            </button>
          )}
        </div>
      </div>

      <DiscordSettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    </div>
  );
}
