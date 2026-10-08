import { Mic, MicOff, MessageSquarePlus, Phone, PhoneOff, Settings, UserRound } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/design-system/Dialog";
import { EmptyState } from "@/design-system/EmptyState";
import { Field } from "@/design-system/Field";
import { IconButton } from "@/design-system/IconButton";
import { Input } from "@/design-system/Input";
import { LoadingState } from "@/design-system/LoadingState";
import { ProfileAvatarBadge } from "@/design-system/ProfileAvatarBadge";
import { Select } from "@/design-system/Select";
import { Switch } from "@/design-system/Switch";
import { UserAvatarBadge } from "@/design-system/UserAvatarBadge";
import { MicMeter } from "@/features/call/MicMeter";
import { MoodBadge } from "@/features/identity/MoodBadge";
import { errorMessage } from "@/lib/errorMessage";
import { formatTime } from "@/lib/formatDate";
import { listMicrophones, startMicTest, stopMicTest, type VadMode } from "@/lib/ipc/call";
import { getOutputDevice, listOutputDevices, setOutputDevice } from "@/lib/ipc/voice";
import { cn } from "@/lib/theme/cn";
import { useCallStore, type CallFeedMessage } from "@/stores/callStore";
import { useChatStore } from "@/stores/chatStore";
import { useNavigationStore } from "@/stores/navigationStore";
import { useProfileStore } from "@/stores/profileStore";
import { useToastStore } from "@/stores/toastStore";
import type { Profile } from "@/types/profile";

/** Radix's `Select.Item` requires a non-empty `value` — this stands in for
 * "use the system's current default input device" (`policy.microphone === null`). */
const DEFAULT_MICROPHONE_VALUE = "__default__";
/** Lo mismo para la salida de audio. */
const DEFAULT_SPEAKER_VALUE = "__default__";

/** Same bubble shape/alignment as Chat's `MessageBubble` — pedido del
 * usuario: que la Llamada se sienta como un chat. Plain text only (no
 * markdown/attachments/speak button): a transcript line is never
 * AI-generated rich content, just what was heard or said out loud. */
function TranscriptBubble({ message, profile }: { message: CallFeedMessage; profile: Profile }) {
  const { i18n } = useTranslation();
  const isUser = message.from === "user";
  const isError = message.from === "error";

  return (
    <div className={cn("flex items-end gap-2", isUser ? "justify-end" : "justify-start")}>
      {!isUser && !isError ? <ProfileAvatarBadge profile={profile} size={36} /> : null}
      <div
        className={cn(
          "max-w-[75%] rounded-lg px-3.5 py-2.5 text-sm leading-relaxed",
          isUser ? "bg-accent text-accent-foreground" : "bg-surface-secondary text-text-primary",
          isError && "border border-error/50 text-error",
        )}
      >
        <p>{message.text}</p>
        <span className={cn("mt-1 block text-right text-xs", isUser ? "text-accent-foreground/85" : "text-text-secondary")}>
          {formatTime(message.at, i18n.language)}
        </span>
      </div>
      {isUser ? <UserAvatarBadge size={36} /> : null}
    </div>
  );
}

/** Todos los ajustes de la llamada (antes en una card siempre visible)
 * viven ahora detrás del engranaje — pedido del usuario para que la
 * llamada en sí sea lo más importante en pantalla, no la configuración. */
function CallSettingsDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const policy = useCallStore((s) => s.policy);
  const savePolicy = useCallStore((s) => s.savePolicy);

  const [isSaving, setIsSaving] = useState(false);
  const status = useCallStore((s) => s.status);
  const [microphones, setMicrophones] = useState<string[]>([]);
  const [speakers, setSpeakers] = useState<string[]>([]);
  const [speaker, setSpeaker] = useState<string | null>(null);
  const [micTestFailed, setMicTestFailed] = useState(false);
  const microphone = policy?.microphone ?? null;
  const callRunning = status === "running" || status === "starting";

  useEffect(() => {
    if (!open) return;
    listMicrophones()
      .then(setMicrophones)
      .catch(() => setMicrophones([]));
    listOutputDevices()
      .then(setSpeakers)
      .catch(() => setSpeakers([]));
    getOutputDevice()
      .then(setSpeaker)
      .catch(() => setSpeaker(null));
  }, [open]);

  // Mientras el diálogo está abierto y no hay llamada, se escucha el micrófono
  // elegido solo para mover la barra de nivel; al cambiar de micrófono se
  // reabre con el nuevo y al cerrar se suelta.
  useEffect(() => {
    if (!open || callRunning) return;
    let cancelled = false;
    setMicTestFailed(false);
    startMicTest().catch(() => {
      if (!cancelled) setMicTestFailed(true);
    });
    return () => {
      cancelled = true;
      useCallStore.setState({ micLevel: 0 });
      void stopMicTest().catch(() => undefined);
    };
  }, [open, callRunning, microphone]);

  async function updateSpeaker(value: string) {
    const next = value === DEFAULT_SPEAKER_VALUE ? null : value;
    setSpeaker(next);
    try {
      await setOutputDevice(next);
    } catch (err) {
      pushToast({ variant: "error", title: t("call.saveError"), description: errorMessage(err) });
    }
  }

  if (!policy) return null;

  async function update(next: typeof policy) {
    if (!next) return;
    setIsSaving(true);
    try {
      await savePolicy(next);
    } catch (err) {
      pushToast({ variant: "error", title: t("call.saveError"), description: errorMessage(err) });
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogTitle className="text-base font-medium text-text-primary">{t("call.settingsTitle")}</DialogTitle>

        <div className="-mx-1 mt-4 flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-1">
          <Field label={t("call.fields.microphone")}>
            <Select
              ariaLabel={t("call.fields.microphone")}
              value={policy.microphone ?? DEFAULT_MICROPHONE_VALUE}
              onValueChange={(value) => void update({ ...policy, microphone: value === DEFAULT_MICROPHONE_VALUE ? null : value })}
              options={[
                { value: DEFAULT_MICROPHONE_VALUE, label: t("call.fields.microphoneDefault") },
                ...microphones.map((name) => ({ value: name, label: name })),
              ]}
            />
          </Field>

          <div className="flex flex-col gap-1.5">
            <MicMeter />
            <p className="text-xs text-text-secondary">{micTestFailed ? t("call.fields.micLevelError") : t("call.fields.micLevelHint")}</p>
          </div>

          <Field label={t("call.fields.speaker")} hint={t("call.fields.speakerHint")}>
            <Select
              ariaLabel={t("call.fields.speaker")}
              value={speaker ?? DEFAULT_SPEAKER_VALUE}
              onValueChange={(value) => void updateSpeaker(value)}
              options={[
                { value: DEFAULT_SPEAKER_VALUE, label: t("call.fields.speakerDefault") },
                ...speakers.map((name) => ({ value: name, label: name })),
              ]}
            />
          </Field>

          <Field label={t("call.fields.echoGuard")} hint={t("call.fields.echoGuardHint")}>
            <Switch
              label={t("call.fields.echoGuard")}
              checked={policy.echoGuard}
              onCheckedChange={(checked) => void update({ ...policy, echoGuard: checked })}
            />
          </Field>

          <Field label={t("call.fields.responseMode")} hint={!policy.respondToAll ? t("call.fields.responseModeWakePhraseHint") : undefined}>
            <Select
              ariaLabel={t("call.fields.responseMode")}
              value={policy.respondToAll ? "all" : "wake_phrase"}
              onValueChange={(value) => void update({ ...policy, respondToAll: value === "all" })}
              options={[
                { value: "all", label: t("call.fields.responseModeAll") },
                { value: "wake_phrase", label: t("call.fields.responseModeWakePhrase") },
              ]}
            />
          </Field>

          {!policy.respondToAll ? (
            <Field label={t("call.fields.wakePhrase")} hint={t("call.fields.wakePhraseHint")}>
              <Input
                value={policy.wakePhrase ?? ""}
                onChange={(event) => void update({ ...policy, wakePhrase: event.target.value || null })}
                placeholder={t("call.fields.wakePhrasePlaceholder")}
              />
            </Field>
          ) : null}

          <Field label={t("call.fields.vadMode")} hint={t("call.fields.vadModeHint")}>
            <Select
              ariaLabel={t("call.fields.vadMode")}
              value={policy.vadMode}
              onValueChange={(value) => void update({ ...policy, vadMode: value as VadMode })}
              options={[
                { value: "quality", label: t("call.fields.vadModeQuality") },
                { value: "low_bitrate", label: t("call.fields.vadModeLowBitrate") },
                { value: "aggressive", label: t("call.fields.vadModeAggressive") },
                { value: "very_aggressive", label: t("call.fields.vadModeVeryAggressive") },
              ]}
            />
          </Field>

          <Field label={t("call.fields.silenceTimeout")} hint={t("call.fields.silenceTimeoutHint")}>
            <Input
              type="number"
              min={100}
              step={100}
              value={policy.silenceTimeoutMs}
              onChange={(event) => void update({ ...policy, silenceTimeoutMs: Number(event.target.value) || 0 })}
            />
          </Field>

          {isSaving ? <p className="text-xs text-text-secondary">{t("call.saving")}</p> : null}
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

export function CallPage() {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);

  const profiles = useProfileStore((s) => s.profiles);
  const activeProfileId = useProfileStore((s) => s.activeProfileId);
  const activeProfile = profiles.find((p) => p.id === activeProfileId) ?? null;

  const isLoading = useCallStore((s) => s.isLoading);
  const load = useCallStore((s) => s.load);
  const policy = useCallStore((s) => s.policy);
  const status = useCallStore((s) => s.status);
  const callState = useCallStore((s) => s.callState);
  const feed = useCallStore((s) => s.feed);
  const isToggling = useCallStore((s) => s.isToggling);
  const isMuted = useCallStore((s) => s.isMuted);
  const start = useCallStore((s) => s.start);
  const stop = useCallStore((s) => s.stop);
  const toggleMute = useCallStore((s) => s.toggleMute);
  const importCallToChat = useChatStore((s) => s.importCallToChat);
  const setActiveTab = useNavigationStore((s) => s.setActiveTab);

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ block: "end" });
  }, [feed]);

  if (isLoading) {
    return <LoadingState label={t("call.loading")} />;
  }

  if (!activeProfile) {
    return <EmptyState icon={<UserRound className="h-6 w-6" />} title={t("call.emptyTitle")} description={t("call.emptyDescription")} />;
  }

  const isRunning = status === "running" || status === "starting";
  const isActive = callState !== null && callState !== "idle" && callState !== "error";
  const stateLabel = t(`call.callState.${callState ?? "idle"}`);

  async function handleToggle() {
    try {
      if (isRunning) {
        await stop();
      } else {
        await start();
      }
    } catch (err) {
      pushToast({ variant: "error", title: t("call.toggleError"), description: errorMessage(err) });
    }
  }

  // Rescata algo interesante que surgió por voz antes de que se pierda —
  // la transcripción de la Llamada solo vive en memoria (`callStore.feed`),
  // nunca se guarda. Las líneas de error se descartan: son diagnóstico de
  // la propia llamada, no algo que el usuario o el personaje "dijeron".
  async function handleExport() {
    if (!activeProfile) return;
    setIsExporting(true);
    try {
      await importCallToChat(
        activeProfile.id,
        feed.filter((message) => message.from !== "error").map((message) => ({ from: message.from, text: message.text })),
      );
      setActiveTab("chat");
      pushToast({ variant: "success", title: t("call.exportedToChat") });
    } catch (err) {
      pushToast({ variant: "error", title: t("call.exportError"), description: errorMessage(err) });
    } finally {
      setIsExporting(false);
    }
  }

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex items-center gap-3 border-b border-border px-4 py-3">
        <ProfileAvatarBadge profile={activeProfile} size="md" />
        <div>
          <p className="text-sm font-medium text-text-primary">{activeProfile.name}</p>
          {status ? (
            <p className={cn("text-xs", status === "error" ? "text-error" : "text-text-secondary")}>{t(`call.status.${status}`)}</p>
          ) : null}
        </div>
        <MoodBadge profileId={activeProfile.id} />
        <IconButton
          className="ml-auto"
          icon={<MessageSquarePlus className="h-4 w-4" />}
          label={t("call.exportToChat")}
          title={t("call.exportToChat")}
          disabled={feed.length === 0 || isExporting}
          onClick={() => void handleExport()}
        />
        <IconButton
          icon={<Settings className="h-4 w-4" />}
          label={t("call.settingsTitle")}
          title={t("call.settingsTitle")}
          onClick={() => setSettingsOpen(true)}
        />
      </header>

      <div className="flex-1 overflow-y-auto px-4 py-4">
        {feed.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <ProfileAvatarBadge profile={activeProfile} size="lg" />
            <p className="max-w-xs text-sm text-text-secondary">{isRunning ? stateLabel : t("call.policyDescription")}</p>
          </div>
        ) : (
          <div className="mx-auto flex max-w-2xl flex-col gap-3">
            {feed.map((message) => (
              <TranscriptBubble key={message.id} message={message} profile={activeProfile} />
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
                    isMuted ? "bg-error" : isActive ? "animate-pulse bg-accent" : "bg-text-secondary",
                  )}
                />
                {isMuted ? t("call.muted") : stateLabel}
              </p>
              <MicMeter muted={isMuted} className="max-w-48" />
              <div className="flex items-center gap-5">
                <IconButton
                  icon={isMuted ? <MicOff className="h-4 w-4" /> : <Mic className="h-4 w-4" />}
                  label={isMuted ? t("call.unmute") : t("call.mute")}
                  title={isMuted ? t("call.unmute") : t("call.mute")}
                  active={isMuted}
                  onClick={() => void toggleMute()}
                />
                <button
                  type="button"
                  onClick={() => void handleToggle()}
                  disabled={isToggling}
                  aria-label={t("call.end")}
                  title={t("call.end")}
                  className="flex h-16 w-16 items-center justify-center rounded-full bg-error text-error-foreground shadow-md transition-transform duration-150 hover:scale-105 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  <PhoneOff className="h-6 w-6" />
                </button>
              </div>
            </>
          ) : (
            <button
              type="button"
              onClick={() => void handleToggle()}
              disabled={isToggling || !policy}
              aria-label={t("call.startCall")}
              title={t("call.startCall")}
              className="flex h-16 w-16 items-center justify-center rounded-full bg-accent text-accent-foreground shadow-md transition-transform duration-150 hover:scale-105 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Phone className="h-6 w-6" />
            </button>
          )}
        </div>
      </div>

      <CallSettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    </div>
  );
}
