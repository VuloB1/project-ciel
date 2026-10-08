import {
  BrainCircuit,
  Calendar,
  ChevronDown,
  ChevronUp,
  Globe,
  Heart,
  Info,
  Lock,
  type LucideIcon,
  Pencil,
  Pin,
  PinOff,
  Plus,
  Search,
  Tag,
  Trash2,
  User,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/design-system/Badge";
import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { ConfirmDialog, Dialog, DialogClose, DialogContent, DialogTitle } from "@/design-system/Dialog";
import { EmptyState } from "@/design-system/EmptyState";
import { Field } from "@/design-system/Field";
import { IconButton } from "@/design-system/IconButton";
import { Input } from "@/design-system/Input";
import { LoadingState } from "@/design-system/LoadingState";
import { Select } from "@/design-system/Select";
import { Slider } from "@/design-system/Slider";
import { Textarea } from "@/design-system/Textarea";
import { formatDateTime } from "@/lib/formatDate";
import { cn } from "@/lib/theme/cn";
import { useMemoryStore } from "@/stores/memoryStore";
import { useProfileStore } from "@/stores/profileStore";
import type { Memory, MemoryCategory } from "@/types/memory";

const CATEGORIES: MemoryCategory[] = ["fact", "preference", "person", "event", "other"];

const CATEGORY_ICONS: Record<MemoryCategory, LucideIcon> = {
  person: User,
  preference: Heart,
  event: Calendar,
  fact: Info,
  other: Tag,
};

const MAX_TITLE_LENGTH = 80;
const MAX_SUMMARY_LENGTH = 200;
const MAX_CONTENT_LENGTH = 600;

/** Rough character count past which the 3-line clamp on `MemoryCard`'s
 * preview is likely to actually cut text off — just a heuristic to decide
 * whether "Ver más" is worth showing. The clamp itself is CSS (`line-clamp-3`)
 * and always renders correctly regardless of this guess; getting it slightly
 * wrong only means the toggle appears (harmless) or not (still safe, since
 * a preview short enough to not need clamping never hid anything anyway). */
const PREVIEW_CLAMP_HEURISTIC_CHARS = 140;

/** The delete confirmation quotes the memory's title inline in a single-line
 * dialog heading (no wrapping there, unlike the card preview) — a title
 * with no spaces to break on would otherwise run past the dialog's edge, so
 * it's hard-truncated for that one line rather than wrapped. */
const DELETE_TITLE_PREVIEW_CHARS = 40;

function truncateForTitle(text: string): string {
  return text.length > DELETE_TITLE_PREVIEW_CHARS ? `${text.slice(0, DELETE_TITLE_PREVIEW_CHARS)}…` : text;
}

/** Shared form for both creating a new memory and editing an existing one —
 * same fields either way, just a different save action and starting
 * values. Three separate fields (pedido del usuario): a short title so the
 * list reads at a glance, a compact summary (what actually gets recalled in
 * chat — see `AIOrchestrator`'s memory section), and a longer, more
 * detailed version for when the summary alone isn't enough context. */
function MemoryFormDialog({
  profileId,
  memory,
  onClose,
}: {
  profileId: string;
  memory?: Memory;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const create = useMemoryStore((s) => s.create);
  const update = useMemoryStore((s) => s.update);
  const [title, setTitle] = useState(memory?.title ?? "");
  const [summary, setSummary] = useState(memory?.summary ?? "");
  const [content, setContent] = useState(memory?.content ?? "");
  const [category, setCategory] = useState<MemoryCategory>(memory?.category ?? "fact");
  const [importance, setImportance] = useState(memory?.importance ?? 50);
  const [isSaving, setIsSaving] = useState(false);

  const canSave = title.trim().length > 0 && content.trim().length > 0;

  async function handleSave() {
    if (!canSave) return;
    setIsSaving(true);
    try {
      const payload = {
        title: title.trim(),
        content: content.trim(),
        summary: summary.trim() || undefined,
        category,
        importance,
      };
      if (memory) {
        await update(memory.id, profileId, payload);
      } else {
        await create({ profileId, ...payload });
      }
      onClose();
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <DialogContent>
      <DialogTitle className="text-base font-medium text-text-primary">
        {memory ? t("memory.edit.title") : t("memory.new.title")}
      </DialogTitle>
      {/* `overflow-y-auto` here also computes `overflow-x` to `auto` per the
          CSS overflow spec (an axis can't stay `visible` once the other
          scrolls) — without matching horizontal padding, the focus ring on
          a field flush against the left edge gets clipped by the container
          itself. `-mx-1 px-1` keeps the fields visually aligned with the
          title/buttons above and below while still reserving that room. */}
      <div className="-mx-1 mt-4 flex max-h-[70vh] flex-col gap-4 overflow-y-auto px-1">
        <Field label={t("memory.fields.title")}>
          <Input value={title} onChange={(event) => setTitle(event.target.value.slice(0, MAX_TITLE_LENGTH))} autoFocus />
        </Field>
        <Field label={t("memory.fields.summary")} hint={t("memory.fields.summaryHint")}>
          <Textarea
            autoGrow
            value={summary}
            onChange={(event) => setSummary(event.target.value.slice(0, MAX_SUMMARY_LENGTH))}
            rows={2}
          />
        </Field>
        <Field label={t("memory.fields.content")} hint={t("memory.fields.contentHint")}>
          <Textarea
            autoGrow
            value={content}
            onChange={(event) => setContent(event.target.value.slice(0, MAX_CONTENT_LENGTH))}
            rows={5}
          />
          <p className="mt-1 text-right text-xs text-text-secondary">{content.length}/{MAX_CONTENT_LENGTH}</p>
        </Field>
        <div className="grid grid-cols-2 gap-4">
          <Field label={t("memory.category")}>
            <Select
              ariaLabel={t("memory.category")}
              value={category}
              onValueChange={(value) => setCategory(value as MemoryCategory)}
              options={CATEGORIES.map((c) => ({ value: c, label: t(`memory.categories.${c}`) }))}
            />
          </Field>
          <div className="flex flex-col gap-1.5">
            <div className="flex items-center justify-between text-sm">
              <span className="text-text-primary">{t("memory.importance")}</span>
              <span className="tabular-nums text-text-secondary">{importance}</span>
            </div>
            <Slider label={t("memory.importance")} value={importance} onValueChange={setImportance} />
          </div>
        </div>
      </div>
      <div className="mt-5 flex justify-end gap-2">
        <DialogClose asChild>
          <Button variant="ghost">{t("common.cancel")}</Button>
        </DialogClose>
        <Button onClick={() => void handleSave()} disabled={isSaving || !canSave}>
          {isSaving ? t("identity.saving") : t("memory.new.save")}
        </Button>
      </div>
    </DialogContent>
  );
}

function MemoryCard({ memory, profileId }: { memory: Memory; profileId: string }) {
  const { t, i18n } = useTranslation();
  const setPinned = useMemoryStore((s) => s.setPinned);
  const setScope = useMemoryStore((s) => s.setScope);
  const remove = useMemoryStore((s) => s.remove);
  const [isEditing, setIsEditing] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [publicConfirmOpen, setPublicConfirmOpen] = useState(false);
  const isPublic = memory.scope === "public";

  // The summary (if there is one) is the compact version shown by default;
  // `content` — the longer, more detailed version — only shows when the
  // user asks for it, so the list itself stays scannable rather than a
  // wall of paragraphs (pedido del usuario). Even without a separate
  // summary, a long `content` alone still needs the same collapse/expand
  // treatment — otherwise it either overflows the card (a giant unbroken
  // string) or blows up its height (a giant paragraph) once it's long.
  const previewText = memory.summary?.trim() || memory.content;
  const hasMoreToShow = memory.content.trim() !== previewText.trim() || previewText.length > PREVIEW_CLAMP_HEURISTIC_CHARS;
  const CategoryIcon = CATEGORY_ICONS[memory.category];

  return (
    <Card className="flex flex-col gap-2 p-3.5">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="truncate text-sm font-semibold text-text-primary">{memory.title}</p>
          <p
            className={cn(
              "mt-0.5 whitespace-pre-wrap break-words text-sm",
              isExpanded ? "text-text-primary" : "line-clamp-3 text-text-secondary",
            )}
          >
            {isExpanded ? memory.content : previewText}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <IconButton
            icon={memory.isPinned ? <Pin className="h-3.5 w-3.5 fill-current" /> : <PinOff className="h-3.5 w-3.5" />}
            label={memory.isPinned ? t("memory.unpin") : t("memory.pin")}
            active={memory.isPinned}
            onClick={() => void setPinned(memory.id, profileId, !memory.isPinned)}
          />
          <IconButton
            icon={isPublic ? <Globe className="h-3.5 w-3.5" /> : <Lock className="h-3.5 w-3.5" />}
            label={isPublic ? t("memory.scope.makePrivate") : t("memory.scope.makePublic")}
            active={isPublic}
            // Hacerla pública expone lo que contiene a quien escriba en Twitch o
            // Discord: pide confirmación. Volver a privada es inmediato.
            onClick={() => (isPublic ? void setScope(memory.id, profileId, "private") : setPublicConfirmOpen(true))}
          />
          <Dialog open={isEditing} onOpenChange={setIsEditing}>
            <IconButton icon={<Pencil className="h-3.5 w-3.5" />} label={t("memory.edit.title")} onClick={() => setIsEditing(true)} />
            {isEditing ? (
              <MemoryFormDialog profileId={profileId} memory={memory} onClose={() => setIsEditing(false)} />
            ) : null}
          </Dialog>
          <IconButton
            icon={<Trash2 className="h-3.5 w-3.5" />}
            label={t("memory.delete")}
            onClick={() => setDeleteOpen(true)}
          />
        </div>
      </div>

      <ConfirmDialog
        open={deleteOpen}
        onOpenChange={setDeleteOpen}
        title={t("memory.deleteConfirmTitle", { title: truncateForTitle(memory.title) })}
        description={t("memory.deleteConfirmDescription")}
        confirmLabel={t("memory.delete")}
        cancelLabel={t("common.cancel")}
        danger
        onConfirm={() => void remove(memory.id, profileId)}
      />

      <ConfirmDialog
        open={publicConfirmOpen}
        onOpenChange={setPublicConfirmOpen}
        title={t("memory.scope.confirmTitle", { title: truncateForTitle(memory.title) })}
        description={t("memory.scope.confirmDescription")}
        confirmLabel={t("memory.scope.makePublic")}
        cancelLabel={t("common.cancel")}
        onConfirm={() => void setScope(memory.id, profileId, "public")}
      />

      {hasMoreToShow ? (
        <button
          type="button"
          onClick={() => setIsExpanded((v) => !v)}
          className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary"
        >
          {isExpanded ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
          {isExpanded ? t("memory.showLess") : t("memory.showMore")}
        </button>
      ) : null}

      <div className="flex items-center gap-2">
        <Badge>
          <CategoryIcon className="h-3 w-3" />
          {t(`memory.categories.${memory.category}`)}
        </Badge>
        {isPublic ? (
          <span title={t("memory.scope.publicTitle")}>
            <Badge variant="success">
              <Globe className="h-3 w-3" />
              {t("memory.scope.publicBadge")}
            </Badge>
          </span>
        ) : null}
        <div className="flex items-center gap-1.5" title={t("memory.importanceLabel", { value: memory.importance })}>
          <div className="h-1 w-10 overflow-hidden rounded-full bg-surface-secondary">
            <div className="h-full rounded-full bg-accent" style={{ width: `${memory.importance}%` }} />
          </div>
          <span className="text-xs tabular-nums text-text-secondary">{memory.importance}</span>
        </div>
        <span className="ml-auto text-xs text-text-secondary">{formatDateTime(memory.createdAt, i18n.language)}</span>
      </div>
    </Card>
  );
}

export function MemoryPage() {
  const { t } = useTranslation();
  const profiles = useProfileStore((s) => s.profiles);
  const activeProfileId = useProfileStore((s) => s.activeProfileId);
  const activeProfile = profiles.find((p) => p.id === activeProfileId) ?? null;

  const memories = useMemoryStore((s) => (activeProfile ? s.memoriesByProfile[activeProfile.id] : undefined));
  const isLoading = useMemoryStore((s) => s.isLoading);
  const load = useMemoryStore((s) => s.load);
  const search = useMemoryStore((s) => s.search);

  const [query, setQuery] = useState("");
  const [isCreating, setIsCreating] = useState(false);

  useEffect(() => {
    if (activeProfile) void load(activeProfile.id);
    setQuery("");
    // Reload only when the active profile actually changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeProfile?.id]);

  if (!activeProfile) {
    return (
      <EmptyState
        icon={<BrainCircuit className="h-6 w-6" />}
        title={t("placeholder.chat.noProfileTitle")}
        description={t("memory.noProfileDescription")}
      />
    );
  }

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex items-center gap-2 border-b border-border px-4 py-3">
        <div className="relative flex-1 max-w-sm">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-text-secondary" />
          <Input
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              void search(activeProfile.id, event.target.value);
            }}
            placeholder={t("memory.search")}
            className="h-9 pl-8"
          />
        </div>
        <Dialog open={isCreating} onOpenChange={setIsCreating}>
          <Button onClick={() => setIsCreating(true)}>
            <Plus className="h-4 w-4" />
            {t("memory.new.title")}
          </Button>
          {isCreating ? <MemoryFormDialog profileId={activeProfile.id} onClose={() => setIsCreating(false)} /> : null}
        </Dialog>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        {isLoading && !memories ? (
          <LoadingState label={t("memory.loading")} />
        ) : !memories || memories.length === 0 ? (
          <EmptyState
            icon={<BrainCircuit className="h-6 w-6" />}
            title={query ? t("memory.emptySearchTitle") : t("memory.emptyTitle")}
            description={query ? t("memory.emptySearchDescription") : t("memory.emptyDescription")}
          />
        ) : (
          <div className="mx-auto flex max-w-2xl flex-col gap-2">
            {memories.map((memory) => (
              <MemoryCard key={memory.id} memory={memory} profileId={activeProfile.id} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
