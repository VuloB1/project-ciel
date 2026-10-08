import { ChevronUp, Loader2, Maximize2, Mic, Minimize2, Paperclip, Send, Square, X } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState, type ChangeEvent } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { IconButton } from "@/design-system/IconButton";
import { ProfileAvatarBadge } from "@/design-system/ProfileAvatarBadge";
import { Textarea } from "@/design-system/Textarea";
import { getDictationSettings } from "@/lib/ipc/stt";
import { cn } from "@/lib/theme/cn";
import { useDictation } from "@/features/chat/useDictation";
import { MoodBadge } from "@/features/identity/MoodBadge";
import { readFileAsBase64, resizeImageToFit } from "@/lib/files";
import { listLlmProviders } from "@/lib/ipc/settings";
import { useChatStore } from "@/stores/chatStore";
import { useNavigationStore } from "@/stores/navigationStore";
import type { Message } from "@/types/chat";
import type { Profile } from "@/types/profile";

import { MessageBubble } from "./MessageBubble";

// See the matching constant in ConversationList.tsx: a `?? []` fallback
// inline in a Zustand selector allocates a new array every render, which
// triggers an infinite update loop once there's no entry for the key yet.
const EMPTY_MESSAGES: Message[] = [];

// Images ride along in every request that still references them (Fase 3) —
// keeping the longest side under this avoids sending multi-megapixel
// photos to a local model that gains nothing from the extra resolution.
const MAX_IMAGE_DIMENSION = 1500;

// Composer textarea: grows with content up to this many lines, then scrolls
// instead of growing further.
const COMPOSER_MAX_LINES = 5;

interface PendingImage {
  base64: string;
  extension: string;
  mimeType: string;
  previewUrl: string;
}

export function ChatWindow({ profile, conversationId }: { profile: Profile; conversationId: string }) {
  const { t } = useTranslation();
  const messages = useChatStore((s) => s.messagesByConversation[conversationId] ?? EMPTY_MESSAGES);
  const isGenerating = useChatStore((s) => Boolean(s.generatingConversations[conversationId]));
  const loadMessages = useChatStore((s) => s.loadMessages);
  const send = useChatStore((s) => s.send);

  const cancel = useChatStore((s) => s.cancel);
  const immersiveMode = useNavigationStore((s) => s.immersiveMode);
  const toggleImmersiveMode = useNavigationStore((s) => s.toggleImmersiveMode);

  const [draft, setDraft] = useState("");
  const [pendingImage, setPendingImage] = useState<PendingImage | null>(null);
  const [visionEnabled, setVisionEnabled] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  // Entrance animation is only for messages that show up WHILE this window
  // is open (a real send, a streaming reply, a proactive message arriving)
  // — not the whole history replaying at once every time a conversation is
  // opened. `seenMessageIds` starts empty and `hasLoadedInitialMessages`
  // stays false until the first non-empty render, so that entire initial
  // batch is marked "seen" without animating; anything appended after that
  // point is genuinely new. Both reset for free on conversation switch
  // since `ChatWindow` is remounted via `key={conversationId}` in ChatPage.
  const seenMessageIdsRef = useRef<Set<string>>(new Set());
  const hasLoadedInitialMessagesRef = useRef(false);

  useEffect(() => {
    void loadMessages(conversationId);
    // Reload only when switching conversations, not on every store change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [conversationId]);

  const dictation = useDictation({ profileId: profile.id, disabled: isGenerating, onText: (text) => void handleDictated(text) });

  // Mientras se dicta, el cuadro muestra el borrador + el texto provisional que
  // llega en vivo (solo lectura); al terminar, ese texto pasa a ser el borrador.
  const shownDraft = dictation.partialText ? (draft.trim() ? `${draft.trim()} ${dictation.partialText}` : dictation.partialText) : draft;

  useLayoutEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    // Reset before measuring so scrollHeight reflects the current content,
    // not a taller height left over from before text was deleted.
    el.style.height = "auto";
    const style = window.getComputedStyle(el);
    const verticalExtra =
      parseFloat(style.paddingTop) +
      parseFloat(style.paddingBottom) +
      parseFloat(style.borderTopWidth) +
      parseFloat(style.borderBottomWidth);
    const maxHeight = parseFloat(style.lineHeight) * COMPOSER_MAX_LINES + verticalExtra;
    const nextHeight = Math.min(el.scrollHeight, maxHeight);
    el.style.height = `${nextHeight}px`;
    el.style.overflowY = el.scrollHeight > maxHeight ? "auto" : "hidden";
  }, [shownDraft]);

  useEffect(() => {
    listLlmProviders()
      .then((settings) => {
        const current = settings.providers.find((p) => p.id === settings.currentProviderId);
        setVisionEnabled(current?.visionEnabled ?? false);
      })
      .catch(() => setVisionEnabled(false));
  }, []);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ block: "end" });
  }, [messages]);

  // Runs after the ids above were read for this render, so it can't mark a
  // message "seen" before its own entrance animation had a chance to play.
  useEffect(() => {
    if (messages.length > 0) hasLoadedInitialMessagesRef.current = true;
    for (const message of messages) seenMessageIdsRef.current.add(message.id);
  }, [messages]);

  // Preview URLs are per-selected-file and need revoking once replaced or
  // the component unmounts — `URL.revokeObjectURL` has no effect if called
  // twice, so this is safe even across a quick pick-then-clear.
  useEffect(() => {
    const url = pendingImage?.previewUrl;
    return () => {
      if (url) URL.revokeObjectURL(url);
    };
  }, [pendingImage?.previewUrl]);

  async function handlePickImage(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    const resized = await resizeImageToFit(file, MAX_IMAGE_DIMENSION);
    const { base64, extension, mimeType } = await readFileAsBase64(resized);
    setPendingImage({ base64, extension, mimeType, previewUrl: URL.createObjectURL(resized) });
  }

  /** Texto que acaba de llegar del dictado: se agrega al borrador, o se envía
   * directo si Configuración > Transcripción de voz lo pide. */
  async function handleDictated(text: string) {
    const settings = await getDictationSettings().catch(() => ({ autoSend: false }));
    const combined = draft.trim() ? `${draft.trim()} ${text}` : text;
    if (settings.autoSend && !isGenerating) {
      await handleSend(combined);
      return;
    }
    setDraft(combined);
    textareaRef.current?.focus();
  }

  async function handleSend(override?: string) {
    const content = (override ?? draft).trim();
    if ((!content && !pendingImage) || isGenerating) return;
    setDraft("");
    const image = pendingImage;
    setPendingImage(null);
    try {
      await send(
        profile.id,
        conversationId,
        content,
        image ? { base64: image.base64, mimeType: image.mimeType, extension: image.extension } : undefined,
      );
    } catch {
      // Give the draft back so the user doesn't lose what they typed.
      setDraft(content);
      setPendingImage(image);
    }
  }

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <header className="flex items-center gap-3 border-b border-border px-4 py-3">
        <ProfileAvatarBadge profile={profile} size="md" />
        <div>
          <p className="text-sm font-medium text-text-primary">{profile.name}</p>
          <p className="text-xs text-text-secondary">{isGenerating ? t("chat.status.thinking") : t("chat.status.ready")}</p>
        </div>
        <MoodBadge profileId={profile.id} />
        <IconButton
          className="ml-auto"
          icon={immersiveMode ? <Minimize2 className="h-4 w-4" /> : <Maximize2 className="h-4 w-4" />}
          label={immersiveMode ? t("chat.immersive.exit") : t("chat.immersive.enter")}
          title={immersiveMode ? t("chat.immersive.exit") : t("chat.immersive.enter")}
          active={immersiveMode}
          onClick={toggleImmersiveMode}
        />
      </header>

      <div className="flex-1 overflow-y-auto px-4 py-4">
        <div className="mx-auto flex max-w-2xl flex-col gap-[var(--chat-message-gap)]">
          {messages.map((message) => (
            <MessageBubble
              key={message.id}
              message={message}
              profile={profile}
              isNew={hasLoadedInitialMessagesRef.current && !seenMessageIdsRef.current.has(message.id)}
            />
          ))}
          <div ref={bottomRef} />
        </div>
      </div>

      <div className="border-t border-border p-3">
        <div className="mx-auto flex max-w-2xl flex-col gap-2">
          {pendingImage ? (
            <div className="relative w-fit">
              <img src={pendingImage.previewUrl} alt="" className="h-16 w-16 rounded-md object-cover" />
              <button
                type="button"
                onClick={() => setPendingImage(null)}
                aria-label={t("chat.composer.removeImage")}
                className="absolute -right-1.5 -top-1.5 flex h-5 w-5 items-center justify-center rounded-full bg-surface-secondary text-text-primary shadow-sm hover:bg-border"
              >
                <X className="h-3 w-3" />
              </button>
            </div>
          ) : null}

          {dictation.phase !== "idle" ? (
            <div className="flex items-center gap-2 rounded-md bg-surface-secondary px-3 py-1.5 text-xs text-text-secondary">
              {dictation.phase === "transcribing" ? (
                <>
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  <span>{t("chat.dictation.transcribing")}</span>
                </>
              ) : (
                <>
                  <span className="h-2 w-2 shrink-0 animate-pulse rounded-full bg-error" />
                  <span className="tabular-nums text-text-primary">
                    {Math.floor(dictation.seconds / 60)}:{String(dictation.seconds % 60).padStart(2, "0")}
                  </span>
                  <span className="flex-1 truncate">
                    {dictation.phase === "holding" ? t("chat.dictation.slideToLock") : t("chat.dictation.lockedHint")}
                  </span>
                  {dictation.phase === "locked" ? (
                    <button type="button" onClick={dictation.cancel} className="rounded-sm px-1.5 py-0.5 text-text-secondary hover:bg-surface hover:text-text-primary">
                      {t("chat.dictation.cancel")}
                    </button>
                  ) : null}
                </>
              )}
            </div>
          ) : null}

          <div className="flex items-end gap-2">
            <input ref={fileInputRef} type="file" accept="image/*" className="hidden" onChange={(event) => void handlePickImage(event)} />
            <IconButton
              round
              size="lg"
              icon={<Paperclip className="h-4 w-4" />}
              label={visionEnabled ? t("chat.composer.attachImage") : t("chat.composer.attachImageDisabled")}
              title={visionEnabled ? t("chat.composer.attachImage") : t("chat.composer.attachImageDisabled")}
              disabled={!visionEnabled || isGenerating}
              onClick={() => fileInputRef.current?.click()}
            />
            <Textarea
              ref={textareaRef}
              value={shownDraft}
              readOnly={dictation.phase !== "idle"}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  void handleSend();
                }
              }}
              placeholder={t("chat.composer.placeholder")}
              rows={1}
              className="resize-none overflow-y-hidden"
              style={{ resize: "none" }}
            />
            <div className="relative">
              {dictation.phase === "holding" ? (
                // Pista de "desliza hacia arriba para fijar": sube con el dedo/cursor.
                <div
                  className="pointer-events-none absolute bottom-full left-1/2 mb-2 flex -translate-x-1/2 flex-col items-center gap-1 rounded-full bg-surface-secondary px-2 py-2 text-text-secondary shadow-md"
                  style={{ transform: `translate(-50%, ${-Math.round(dictation.lockProgress * 24)}px)` }}
                >
                  <ChevronUp className="h-4 w-4" />
                  <ChevronUp className="-mt-3 h-4 w-4 opacity-60" />
                </div>
              ) : null}
              <button
                type="button"
                {...dictation.buttonHandlers}
                disabled={isGenerating || dictation.phase === "transcribing"}
                aria-label={
                  dictation.phase === "locked"
                    ? t("chat.dictation.stop")
                    : dictation.phase === "transcribing"
                      ? t("chat.dictation.transcribing")
                      : t("chat.dictation.start")
                }
                title={dictation.phase === "locked" ? t("chat.dictation.stop") : t("chat.dictation.start")}
                className={cn(
                  "inline-flex h-10 w-10 shrink-0 touch-none select-none items-center justify-center rounded-lg transition duration-150",
                  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-50",
                  dictation.phase === "locked" && "bg-error text-error-foreground hover:opacity-90",
                  dictation.phase === "holding" && "animate-pulse bg-error/20 text-error ring-2 ring-error",
                  (dictation.phase === "idle" || dictation.phase === "transcribing") &&
                    "text-text-secondary hover:bg-surface-secondary hover:text-text-primary active:scale-90",
                )}
              >
                {dictation.phase === "transcribing" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : dictation.phase === "locked" ? (
                  <Square className="h-4 w-4" />
                ) : (
                  <Mic className="h-4 w-4" />
                )}
              </button>
            </div>
            {isGenerating ? (
              <Button variant="danger" onClick={() => void cancel(conversationId)}>
                <Square className="h-4 w-4" />
                {t("chat.composer.stop")}
              </Button>
            ) : (
              <button
                type="button"
                onClick={() => void handleSend()}
                disabled={(!draft.trim() && !pendingImage) || dictation.isRecording || dictation.phase === "transcribing"}
                aria-label={t("chat.composer.send")}
                title={t("chat.composer.send")}
                className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-accent text-accent-foreground transition duration-150 hover:bg-accent-secondary active:scale-95 disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background"
              >
                <Send className="h-4 w-4" />
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
