import { Square, Volume2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkBreaks from "remark-breaks";
import remarkGfm from "remark-gfm";

import { ProfileAvatarBadge } from "@/design-system/ProfileAvatarBadge";
import { UserAvatarBadge } from "@/design-system/UserAvatarBadge";
import { formatTime } from "@/lib/formatDate";
import { getAttachmentBase64 } from "@/lib/ipc/chat";
import { useAttachmentImage } from "@/lib/useAttachmentImage";
import { cn } from "@/lib/theme/cn";
import { useVoiceStore } from "@/stores/voiceStore";
import type { Message } from "@/types/chat";
import type { Profile } from "@/types/profile";

// Los mensajes de la IA llevan el color de acento de fondo: ahí un enlace
// `text-accent` sería invisible (acento sobre acento), así que usa el color de
// texto del propio acento. Los del usuario van sobre la superficie neutra.
const LINK_CLASS_ON_SURFACE = "text-accent underline hover:text-accent-secondary";
const LINK_CLASS_ON_ACCENT = "text-accent-foreground underline hover:opacity-80";

// Module-level so it's built once, not recreated on every render.
// `remark-gfm` (autolinks/strikethrough/tables) + `remark-breaks` (a lone
// "\n" inside a paragraph becomes a real line break — without it, plain
// CommonMark collapses it to a space, a regression from the raw
// `whitespace-pre-wrap` text this replaces, since a model writing casual
// single-newline lines is far more common than one writing strict Markdown).
const REMARK_PLUGINS = [remarkGfm, remarkBreaks];

function buildMarkdownComponents(linkClass: string): Components {
  return {
  a: ({ children, ...props }) => (
    <a {...props} target="_blank" rel="noreferrer" className={linkClass}>
      {children}
    </a>
  ),
  // A model-authored `![alt](url)` would silently fetch an arbitrary
  // third-party URL the moment the message renders — unlike a real
  // attachment (`useAttachmentImage`, bytes already local via IPC), that
  // leaks the viewer's IP/user-agent to whoever owns the URL with no
  // chance to opt out. Render it as a link instead of an auto-loading `<img>`.
  img: ({ alt, src }) => (
    <a href={typeof src === "string" ? src : undefined} target="_blank" rel="noreferrer" className={linkClass}>
      {alt || src || "imagen"}
    </a>
  ),
  p: ({ children }) => <p className="mb-2 last:mb-0">{children}</p>,
  ul: ({ children }) => <ul className="mb-2 list-disc pl-5">{children}</ul>,
  ol: ({ children }) => <ol className="mb-2 list-decimal pl-5">{children}</ol>,
  li: ({ children }) => <li className="mb-0.5">{children}</li>,
  // Fenced code blocks get a `language-*` class from remark/rehype; inline
  // `` `code` `` doesn't — that's the only signal available here to tell
  // them apart and avoid double-styling (pill background nested inside the
  // `pre` block's own background).
  code: ({ className, children, ...props }) =>
    /language-/.test(className ?? "") ? (
      <code className={cn("font-mono text-xs", className)} {...props}>
        {children}
      </code>
    ) : (
      <code className="rounded bg-black/10 px-1 py-0.5 font-mono text-xs dark:bg-white/10" {...props}>
        {children}
      </code>
    ),
  pre: ({ children }) => <pre className="mb-2 overflow-x-auto rounded bg-black/10 p-2 dark:bg-white/10">{children}</pre>,
  // GFM tables render with zero borders/padding once Tailwind's preflight
  // strips the browser defaults — left unstyled they're an unreadable wall
  // of text.
  table: ({ children }) => <table className="mb-2 w-full border-collapse text-left">{children}</table>,
  th: ({ children }) => <th className="border border-border px-2 py-1 font-medium">{children}</th>,
  td: ({ children }) => <td className="border border-border px-2 py-1">{children}</td>,
  };
}

const MARKDOWN_COMPONENTS_ON_SURFACE = buildMarkdownComponents(LINK_CLASS_ON_SURFACE);
const MARKDOWN_COMPONENTS_ON_ACCENT = buildMarkdownComponents(LINK_CLASS_ON_ACCENT);

export function MessageBubble({
  message,
  profile,
  isNew = false,
}: {
  message: Message;
  profile: Profile;
  /** Plays a subtle entrance animation once — true only for a message that
   * showed up while this chat window was already open (see
   * `ChatWindow.tsx`), never for the conversation's existing history. */
  isNew?: boolean;
}) {
  const { t, i18n } = useTranslation();
  const isUser = message.role === "user";
  // Texto atenuado (hora, botón de voz): sobre el acento (mensajes de la IA) se
  // deriva del color de texto del acento para no perder contraste.
  const mutedText = isUser ? "text-text-secondary" : "text-accent-foreground/85";
  // A message whose voice reply was saved (Configuración > Privacidad >
  // "guardar audio de los mensajes") replays the exact original clip
  // instead of calling the TTS engine again — see `handleSpeakClick`. Its
  // attachment is audio, not an image, so `useAttachmentImage` below must
  // skip it (nothing to render as an `<img>`).
  const hasStoredAudio = !isUser && Boolean(message.attachmentId) && (message.attachmentMimeType?.startsWith("audio/") ?? false);
  const attachmentUrl = useAttachmentImage(
    hasStoredAudio ? null : message.attachmentId,
    hasStoredAudio ? null : message.attachmentMimeType,
  );
  const isSpeakingThis = useVoiceStore(
    (s) => s.speaking?.profileId === profile.id && s.speaking.messageId === message.id,
  );
  const isPendingThis = useVoiceStore((s) => s.pending?.profileId === profile.id && s.pending.messageId === message.id);
  const speak = useVoiceStore((s) => s.speak);
  const stop = useVoiceStore((s) => s.stop);
  const playStored = useVoiceStore((s) => s.playStored);
  const isPlayingStored = useVoiceStore((s) => s.storedPlayingId === message.id);

  const canSpeak = !isUser && message.status === "complete" && message.content.trim().length > 0;
  // Antes de que llegue el primer token todavía no hay nada que mostrar
  // con el cursor parpadeante de siempre — se ve como una rayita sola,
  // sin indicar que algo está pasando. Los puntitos saltarines cubren ese
  // hueco y desaparecen solos en cuanto `content` deja de estar vacío.
  const isThinking = !isUser && message.status === "streaming" && message.content.trim().length === 0;

  async function handleSpeakClick() {
    // Si el motor de voz está sonando (o generando) ESTE mensaje —p. ej. la
    // reproducción automática al terminar la respuesta—, el botón es "stop",
    // incluso cuando ya hay una copia guardada del audio: antes caía a la
    // rama de abajo y reproducía esa copia ENCIMA de la voz original, y el
    // segundo clic solo pausaba la copia.
    if (isSpeakingThis || isPendingThis) {
      void stop();
      return;
    }
    if (hasStoredAudio) {
      if (isPlayingStored) {
        void stop();
        return;
      }
      try {
        if (!message.attachmentId) throw new Error("missing attachment id");
        const base64 = await getAttachmentBase64(message.attachmentId);
        // El store corta cualquier otra voz (motor o clip) antes de sonar.
        await playStored(message.id, `data:${message.attachmentMimeType};base64,${base64}`);
        return;
      } catch {
        // Falls through to a live re-synthesis below — the stored clip
        // couldn't be read (deleted, corrupted), but the message can
        // still be spoken the normal way.
      }
    }
    void (isSpeakingThis || isPendingThis ? stop() : speak(profile.id, message.content, message.id));
  }

  return (
    <div className={cn("flex items-end gap-2", isUser ? "justify-end" : "justify-start", isNew && "animate-pop-in")}>
      {isUser ? null : <ProfileAvatarBadge profile={profile} size={43} />}
      <div
        // `data-copy-text` lo lee `AppContextMenu.tsx` (un solo listener
        // global en `window`) para ofrecer "Copiar mensaje" — el contenido
        // crudo del mensaje, no el texto ya renderizado por ReactMarkdown.
        data-copy-text={message.content.trim() ? message.content : undefined}
        className={cn(
          "max-w-[75%] rounded-lg px-[var(--chat-message-padding-x)] py-[var(--chat-message-padding-y)] text-sm leading-relaxed",
          isUser ? "bg-surface-secondary text-text-primary" : "bg-accent text-accent-foreground",
          message.status === "error" && "border border-error/50",
        )}
      >
        {attachmentUrl ? (
          <img src={attachmentUrl} alt={t("chat.message.attachmentAlt")} className="mb-2 max-h-64 rounded-md object-contain" />
        ) : null}
        {isThinking ? (
          <span className="flex items-center gap-1 py-0.5" role="status" aria-label={t("chat.message.thinking")}>
            {/* `animation-delay` set inline, not via a Tailwind
                `[animation-delay:...]` utility — `.animate-typing-bounce`'s
                `animation` shorthand (globals.css) is compiled after
                Tailwind's utilities layer, so a utility class for just the
                delay gets silently overridden back to 0 by the shorthand
                and all three dots bounced together. Inline style always
                wins over both. */}
            <span className="h-1.5 w-1.5 animate-typing-bounce rounded-full bg-current" style={{ animationDelay: "0ms" }} />
            <span className="h-1.5 w-1.5 animate-typing-bounce rounded-full bg-current" style={{ animationDelay: "150ms" }} />
            <span className="h-1.5 w-1.5 animate-typing-bounce rounded-full bg-current" style={{ animationDelay: "300ms" }} />
          </span>
        ) : (
          <>
            <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={isUser ? MARKDOWN_COMPONENTS_ON_SURFACE : MARKDOWN_COMPONENTS_ON_ACCENT}>
              {message.content}
            </ReactMarkdown>
            {message.status === "streaming" ? (
              <span className="ml-0.5 inline-block h-3.5 w-1.5 animate-pulse bg-current align-text-bottom" />
            ) : null}
          </>
        )}
        {message.status === "cancelled" ? (
          <span className={cn("mt-1 block text-xs italic", mutedText)}>{t("chat.message.cancelled")}</span>
        ) : null}
        <div className="mt-1.5 flex items-center gap-2">
          {canSpeak ? (
            <button
              type="button"
              onClick={() => void handleSpeakClick()}
              aria-label={isSpeakingThis || isPendingThis || isPlayingStored ? t("chat.message.stopSpeaking") : t("chat.message.speak")}
              title={isSpeakingThis || isPendingThis || isPlayingStored ? t("chat.message.stopSpeaking") : t("chat.message.speak")}
              className={cn("flex h-5 w-5 items-center justify-center rounded transition-colors hover:bg-black/5 dark:hover:bg-white/10", mutedText, isUser ? "hover:text-text-primary" : "hover:text-accent-foreground")}
            >
              {isSpeakingThis || isPendingThis || isPlayingStored ? <Square className="h-3 w-3" /> : <Volume2 className="h-3.5 w-3.5" />}
            </button>
          ) : null}
          <span className={cn("ml-auto text-xs", mutedText)}>{formatTime(message.createdAt, i18n.language)}</span>
        </div>
      </div>
      {isUser ? <UserAvatarBadge size={43} /> : null}
    </div>
  );
}
