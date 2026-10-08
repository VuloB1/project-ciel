import { BrainCircuit, GripVertical, MessageSquarePlus, MoreHorizontal, Pencil, Phone, Pin, PinOff, Search, Trash2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { ConfirmDialog } from "@/design-system/Dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/design-system/DropdownMenu";
import { IconButton } from "@/design-system/IconButton";
import { Input } from "@/design-system/Input";
import { Switch } from "@/design-system/Switch";
import { errorMessage } from "@/lib/errorMessage";
import { extractMemories } from "@/lib/ipc/chat";
import { cn } from "@/lib/theme/cn";
import { useChatStore } from "@/stores/chatStore";
import { useMemoryStore } from "@/stores/memoryStore";
import { useProactiveStore } from "@/stores/proactiveStore";
import { useToastStore } from "@/stores/toastStore";
import type { Conversation } from "@/types/chat";

// Stable reference so the selector below never hands React a fresh array on
// every render when there's no entry yet — a `?? []` inline would create a
// new array each call, which Zustand's reference-equality check reads as a
// change every time and loops forever ("Maximum update depth exceeded").
const EMPTY_CONVERSATIONS: Conversation[] = [];

export function ConversationList({ profileId }: { profileId: string }) {
  const { t } = useTranslation();
  const conversations = useChatStore((s) => s.conversationsByProfile[profileId] ?? EMPTY_CONVERSATIONS);
  const activeConversationId = useChatStore((s) => s.activeConversationId);
  const setActiveConversationId = useChatStore((s) => s.setActiveConversationId);
  const createNewConversation = useChatStore((s) => s.createNewConversation);
  const renameConversationTitle = useChatStore((s) => s.renameConversationTitle);
  const removeConversation = useChatStore((s) => s.removeConversation);
  const togglePin = useChatStore((s) => s.togglePin);
  const reorderPinned = useChatStore((s) => s.reorderPinned);
  // Conversación con un mensaje proactivo sin leer (ver Sidebar.tsx) —
  // acá se muestra el mismo puntito, pero en la fila exacta de donde
  // viene, para no tener que adivinar cuál conversación fue.
  const unreadConversationId = useProactiveStore((s) => s.unreadConversationByProfile[profileId]);
  const loadMemories = useMemoryStore((s) => s.load);
  const pushToast = useToastStore((s) => s.push);

  const [query, setQuery] = useState("");
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [extractingId, setExtractingId] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Conversation | null>(null);
  const [extractFirst, setExtractFirst] = useState(false);

  // Arrastre de fijados con Pointer Events en vez de drag-and-drop nativo
  // del HTML5: la API nativa deja que el navegador dibuje el "fantasma" del
  // elemento siguiendo al cursor libremente en X e Y, sin forma de acotarlo.
  // Con pointer events aplicamos nosotros mismos un `translateY` acotado al
  // espacio del grupo "Fijados", así el ítem solo se mueve verticalmente.
  const pinnedContainerRef = useRef<HTMLDivElement | null>(null);
  const pinnedRowRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const [pinDrag, setPinDrag] = useState<{
    id: string;
    pointerId: number;
    startClientY: number;
    minTranslate: number;
    maxTranslate: number;
    translateY: number;
    rowHeight: number;
    startIndex: number;
    targetIndex: number;
  } | null>(null);
  // El mouse puede reportar `pointermove` a una tasa mucho más alta que la
  // pantalla puede pintar — acá se guarda solo el último dato recibido y se
  // aplica a lo sumo una vez por frame, para no acumular trabajo de más si
  // llegan ráfagas de eventos (ver nota en `handlePinPointerMove`).
  const pendingMoveRef = useRef<{ pointerId: number; clientY: number } | null>(null);
  const rafIdRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (rafIdRef.current !== null) cancelAnimationFrame(rafIdRef.current);
    };
  }, []);

  async function handleTogglePin(conversation: Conversation) {
    try {
      await togglePin(conversation.id, profileId);
    } catch (err) {
      pushToast({ variant: "error", title: t("chat.conversations.pinError"), description: errorMessage(err) });
    }
  }

  // Solo se reordena dentro del grupo "Fijados" — soltar ahí no fija ni
  // desfija nada por sí solo, eso sigue siendo una acción explícita del
  // menú (ver `handleTogglePin`).
  function reorderPinnedRows(draggedConversationId: string, targetId: string, ids: string[]) {
    if (draggedConversationId === targetId) return;
    const next = [...ids];
    const from = next.indexOf(draggedConversationId);
    const to = next.indexOf(targetId);
    if (from === -1 || to === -1) return;
    next.splice(from, 1);
    next.splice(to, 0, draggedConversationId);
    reorderPinned(profileId, next).catch((err) => {
      pushToast({ variant: "error", title: t("chat.conversations.reorderError"), description: errorMessage(err) });
    });
  }

  function handlePinPointerDown(event: React.PointerEvent, conversationId: string) {
    const rowEl = pinnedRowRefs.current.get(conversationId);
    const containerEl = pinnedContainerRef.current;
    if (!rowEl || !containerEl) return;
    event.preventDefault();
    const rowRect = rowEl.getBoundingClientRect();
    const containerRect = containerEl.getBoundingClientRect();
    event.currentTarget.setPointerCapture(event.pointerId);
    setPinDrag({
      id: conversationId,
      pointerId: event.pointerId,
      startClientY: event.clientY,
      // Cuánto puede subir/bajar el elemento sin salirse del espacio del
      // grupo "Fijados" — la "disponibilidad" de la que habla el pedido.
      minTranslate: containerRect.top - rowRect.top,
      maxTranslate: containerRect.bottom - rowRect.bottom,
      translateY: 0,
      rowHeight: rowRect.height,
      startIndex: pinnedIds.indexOf(conversationId),
      targetIndex: pinnedIds.indexOf(conversationId),
    });
  }

  // Antes esto releía `getBoundingClientRect()` de cada fila fijada en CADA
  // evento y encima disparaba un re-render por cada uno — algunos mouses
  // reportan `pointermove` a cientos o miles de eventos por segundo, muy por
  // encima de lo que la pantalla puede pintar, así que esa ráfaga saturaba
  // el hilo principal y llegaba a congelar la interfaz con movimientos
  // rápidos y repetidos. Ahora se guarda solo el último dato y se aplica
  // como mucho una vez por frame (`requestAnimationFrame`); el cálculo en
  // sí ya era aritmética simple sobre la altura de fila, sin tocar el DOM.
  function handlePinPointerMove(event: React.PointerEvent) {
    pendingMoveRef.current = { pointerId: event.pointerId, clientY: event.clientY };
    if (rafIdRef.current !== null) return;
    rafIdRef.current = requestAnimationFrame(() => {
      rafIdRef.current = null;
      const pending = pendingMoveRef.current;
      if (!pending) return;
      setPinDrag((prev) => {
        if (!prev || prev.pointerId !== pending.pointerId) return prev;
        const translateY = Math.min(prev.maxTranslate, Math.max(prev.minTranslate, pending.clientY - prev.startClientY));
        const slots = Math.round(translateY / prev.rowHeight);
        const targetIndex = Math.min(pinnedIds.length - 1, Math.max(0, prev.startIndex + slots));
        if (translateY === prev.translateY && targetIndex === prev.targetIndex) return prev;
        return { ...prev, translateY, targetIndex };
      });
    });
  }

  function handlePinPointerUp(event: React.PointerEvent) {
    if (rafIdRef.current !== null) {
      cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = null;
    }
    pendingMoveRef.current = null;
    setPinDrag((prev) => {
      if (!prev || prev.pointerId !== event.pointerId) return null;
      const targetId = pinnedIds[prev.targetIndex];
      if (targetId) reorderPinnedRows(prev.id, targetId, pinnedIds);
      return null;
    });
  }

  function openDeleteConfirm(conversation: Conversation) {
    setDeleteTarget(conversation);
    setExtractFirst(false);
  }

  async function handleConfirmDelete() {
    const target = deleteTarget;
    if (!target) return;
    // `handleExtractMemories` already toasts its own errors and never
    // rethrows — deletion always proceeds, since the user already
    // explicitly confirmed it and a failed (unrelated) extraction
    // shouldn't block the action they asked for.
    if (extractFirst) await handleExtractMemories(target.id);
    await removeConversation(target.id, profileId);
    setDeleteTarget(null);
  }

  async function handleExtractMemories(conversationId: string) {
    setExtractingId(conversationId);
    try {
      const created = await extractMemories(conversationId, profileId);
      await loadMemories(profileId);
      pushToast(
        created.length > 0
          ? { variant: "success", title: t("chat.conversations.memoriesExtracted", { count: created.length }) }
          : { variant: "info", title: t("chat.conversations.noNewMemories") },
      );
    } catch (err) {
      pushToast({ variant: "error", title: t("chat.conversations.extractMemoriesError"), description: errorMessage(err) });
    } finally {
      setExtractingId(null);
    }
  }

  const filtered = useMemo(
    () => conversations.filter((c) => c.title.toLowerCase().includes(query.toLowerCase())),
    [conversations, query],
  );
  // Ya vienen en el orden correcto (el store espeja el `ORDER BY` del
  // backend) — acá solo se separan para poder darle su propio encabezado
  // "Fijados" al primer grupo, sin encabezado para el resto (igual que
  // antes de que existiera fijar).
  const pinned = useMemo(() => filtered.filter((c) => c.isPinned), [filtered]);
  const unpinned = useMemo(() => filtered.filter((c) => !c.isPinned), [filtered]);
  const pinnedIds = useMemo(() => pinned.map((c) => c.id), [pinned]);

  function startRename(conversation: Conversation) {
    setRenamingId(conversation.id);
    setRenameValue(conversation.title);
  }

  async function commitRename() {
    if (renamingId && renameValue.trim()) {
      await renameConversationTitle(renamingId, profileId, renameValue.trim());
    }
    setRenamingId(null);
  }

  function renderConversationLabel(conversation: Conversation) {
    if (renamingId === conversation.id) {
      return (
        <Input
          autoFocus
          value={renameValue}
          onChange={(event) => setRenameValue(event.target.value)}
          onBlur={() => void commitRename()}
          onKeyDown={(event) => {
            if (event.key === "Enter") void commitRename();
            if (event.key === "Escape") setRenamingId(null);
          }}
          className="my-1 ml-2 h-7 flex-1 text-xs"
        />
      );
    }
    return (
      // `flex-1` alone only sizes this to its text's own line height —
      // the row's visual padding lived on the parent div, so clicking
      // above/below the text (inside the row but outside the text
      // line) missed the button entirely. Moving that padding onto
      // the button itself makes its clickable area match what's
      // visually painted as the row.
      <button
        type="button"
        onClick={() => setActiveConversationId(conversation.id)}
        className="flex flex-1 min-w-0 items-center gap-1.5 truncate rounded-md py-1.5 pl-2 text-left text-sm text-text-primary"
      >
        {conversation.source === "call" ? (
          <Phone
            className="h-3 w-3 shrink-0 text-text-secondary"
            aria-label={t("chat.conversations.importedFromCallLabel")}
          />
        ) : null}
        <span className="truncate">{conversation.title}</span>
        {conversation.id === unreadConversationId ? (
          <span aria-hidden="true" className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent" />
        ) : null}
      </button>
    );
  }

  function renderConversationMenu(conversation: Conversation) {
    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <IconButton
            icon={<MoreHorizontal className="h-3.5 w-3.5" />}
            label={t("chat.conversations.options")}
            className="mr-2 opacity-0 group-hover:opacity-100"
          />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onSelect={() => startRename(conversation)}>
            <Pencil className="h-3.5 w-3.5" />
            {t("chat.conversations.rename")}
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => void handleTogglePin(conversation)}>
            {conversation.isPinned ? <PinOff className="h-3.5 w-3.5" /> : <Pin className="h-3.5 w-3.5" />}
            {conversation.isPinned ? t("chat.conversations.unpin") : t("chat.conversations.pin")}
          </DropdownMenuItem>
          <DropdownMenuItem
            disabled={extractingId === conversation.id}
            onSelect={() => void handleExtractMemories(conversation.id)}
          >
            <BrainCircuit className="h-3.5 w-3.5" />
            {extractingId === conversation.id
              ? t("chat.conversations.extractingMemories")
              : t("chat.conversations.extractMemories")}
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => openDeleteConfirm(conversation)}>
            <Trash2 className="h-3.5 w-3.5" />
            {t("chat.conversations.delete")}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    );
  }

  return (
    <div className="flex w-64 shrink-0 flex-col border-r border-border">
      <div className="flex items-center justify-between gap-2 px-3 py-3">
        <span className="text-xs font-medium uppercase tracking-wide text-text-secondary">
          {t("chat.conversations.title")}
        </span>
        <IconButton
          icon={<MessageSquarePlus className="h-4 w-4" />}
          label={t("chat.conversations.new")}
          onClick={() => void createNewConversation(profileId)}
        />
      </div>

      <div className="px-3 pb-2">
        <div className="relative">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-text-secondary" />
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("chat.conversations.search")}
            className="h-8 pl-8 text-xs"
          />
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-2 pb-2">
        {filtered.length === 0 ? (
          <p className="px-2 py-4 text-center text-xs text-text-secondary">{t("chat.conversations.empty")}</p>
        ) : (
          <>
            {pinned.length > 0 ? (
              <p className="px-2 pb-1 pt-2 text-[10px] font-medium uppercase tracking-wide text-text-secondary">
                {t("chat.conversations.pinnedGroup")}
              </p>
            ) : null}
            <div ref={pinnedContainerRef} className="relative">
              {pinned.map((conversation, index) => {
                const isDragging = pinDrag?.id === conversation.id;
                // Mientras se arrastra, los vecinos entre la posición
                // original del arrastrado y la posición del destino se
                // corren un lugar para "abrir hueco" — el mismo efecto
                // visual de listas como Trello o Linear, en vez de solo
                // resaltar dónde va a caer.
                let neighborShift = 0;
                if (pinDrag && !isDragging) {
                  const { startIndex, targetIndex } = pinDrag;
                  if (startIndex < targetIndex && index > startIndex && index <= targetIndex) {
                    neighborShift = -pinDrag.rowHeight;
                  } else if (startIndex > targetIndex && index >= targetIndex && index < startIndex) {
                    neighborShift = pinDrag.rowHeight;
                  }
                }
                const translateY = isDragging ? pinDrag.translateY : neighborShift;
                return (
                  <div
                    key={conversation.id}
                    ref={(el) => {
                      if (el) pinnedRowRefs.current.set(conversation.id, el);
                      else pinnedRowRefs.current.delete(conversation.id);
                    }}
                    style={translateY !== 0 ? { transform: `translateY(${translateY}px)` } : undefined}
                    className={cn(
                      "group flex items-center gap-1 rounded-md",
                      conversation.id === activeConversationId ? "bg-surface-secondary" : "hover:bg-surface-secondary",
                      isDragging && "relative z-10 bg-surface-secondary shadow-md",
                      // Solo animado mientras hay un arrastre activo: al
                      // soltar, el store ya reordena el array en el mismo
                      // instante en que `pinDrag` pasa a null, así que la
                      // fila cae directo a su posición final. Si esta clase
                      // quedara puesta en ese momento, el reset del
                      // `transform` a 0 se animaría justo cuando el nuevo
                      // orden ya está en el DOM, dando el efecto de que la
                      // fila "se acomoda" con un salto extra.
                      !isDragging && pinDrag && "transition-transform duration-150 ease-out",
                    )}
                  >
                    {/* En reposo se ve el ícono de fijado; recién al pasar
                        el mouse (o mientras se arrastra) se revela la
                        agarradera para reordenar — mismo lugar, crossfade
                        por opacidad en vez de dos elementos que empujan el
                        layout. */}
                    <div className="relative ml-1 h-3.5 w-3.5 shrink-0">
                      <Pin
                        aria-hidden="true"
                        className={cn(
                          "pointer-events-none absolute inset-0 h-3.5 w-3.5 text-text-secondary transition-opacity",
                          isDragging ? "opacity-0" : "opacity-100 group-hover:opacity-0",
                        )}
                      />
                      <GripVertical
                        className={cn(
                          "absolute inset-0 h-3.5 w-3.5 touch-none text-text-secondary transition-opacity",
                          isDragging ? "cursor-grabbing opacity-100" : "cursor-grab opacity-0 group-hover:opacity-100",
                        )}
                        onPointerDown={(event) => handlePinPointerDown(event, conversation.id)}
                        onPointerMove={handlePinPointerMove}
                        onPointerUp={handlePinPointerUp}
                        onPointerCancel={handlePinPointerUp}
                      />
                    </div>
                    {renderConversationLabel(conversation)}
                    {renderConversationMenu(conversation)}
                  </div>
                );
              })}
            </div>
            {unpinned.map((conversation) => (
              <div
                key={conversation.id}
                className={cn(
                  "group flex items-center gap-1 rounded-md",
                  conversation.id === activeConversationId ? "bg-surface-secondary" : "hover:bg-surface-secondary",
                )}
              >
                {renderConversationLabel(conversation)}
                {renderConversationMenu(conversation)}
              </div>
            ))}
          </>
        )}
      </div>

      <ConfirmDialog
        open={deleteTarget !== null}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null);
        }}
        title={t("chat.conversations.deleteConfirmTitle", { title: deleteTarget?.title ?? "" })}
        description={t("chat.conversations.deleteConfirmDescription")}
        confirmLabel={t("chat.conversations.delete")}
        cancelLabel={t("common.cancel")}
        danger
        onConfirm={() => void handleConfirmDelete()}
      >
        <div className="flex items-center gap-2">
          <span className="text-sm text-text-primary">{t("chat.conversations.extractFirstLabel")}</span>
          <Switch label={t("chat.conversations.extractFirstLabel")} checked={extractFirst} onCheckedChange={setExtractFirst} />
        </div>
      </ConfirmDialog>
    </div>
  );
}
