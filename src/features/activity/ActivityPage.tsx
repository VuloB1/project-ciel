import { Activity, ChevronDown, ChevronUp, Eye, EyeOff, MoreHorizontal, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge, type BadgeVariant } from "@/design-system/Badge";
import { Card } from "@/design-system/Card";
import { ConfirmDialog } from "@/design-system/Dialog";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/design-system/DropdownMenu";
import { EmptyState } from "@/design-system/EmptyState";
import { IconButton } from "@/design-system/IconButton";
import { LoadingState } from "@/design-system/LoadingState";
import { Select } from "@/design-system/Select";
import { errorMessage } from "@/lib/errorMessage";
import { formatDateTime } from "@/lib/formatDate";
import type { ActivityEvent, ActivityLevel } from "@/lib/ipc/activity";
import { cn } from "@/lib/theme/cn";
import { useActivityStore } from "@/stores/activityStore";
import { useToastStore } from "@/stores/toastStore";

// Live feed of `activity_events` (Fase 11) — everything already being
// recorded via the `tracing` → `ActivityLayer` bridge across Chat, Twitch,
// Discord, and the orchestrator. App-wide, not per-character: most events
// (the startup one, channel connections, tool calls) carry no `profileId`.

const POLL_INTERVAL_MS = 5000;
// Radix `Select.Item` can't use "" as a value (it's reserved internally for
// "nothing selected"), so the "show everything" option needs its own
// sentinel distinct from any real level/category string.
const ALL = "ALL";

const LEVELS: ActivityLevel[] = ["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"];
// Bootstrap/infrastructure events ("la app se inició correctamente" and
// similar) — real for a bug report, noise for day-to-day use. Hidden by
// default; the toggle in the header brings them back.
const SYSTEM_CATEGORY = "SYSTEM";

const LEVEL_VARIANT: Record<ActivityLevel, BadgeVariant> = {
  DEBUG: "neutral",
  INFO: "neutral",
  WARNING: "warning",
  ERROR: "error",
  CRITICAL: "error",
};

/** Una fila por evento en vez de una card espaciosa por evento — pedido
 * del usuario tras el rediseño de Configuración como ventana flotante: la
 * lista ocupaba demasiado alto ahí adentro. El detalle (módulo, acción,
 * estado, duración y el JSON crudo) se pliega hasta que se expande; lo
 * único siempre visible por defecto es nivel, categoría, resumen (truncado
 * a una línea) y hora — al expandir, el resumen se muestra completo. */
function ActivityRow({ event }: { event: ActivityEvent }) {
  const { t, i18n } = useTranslation();
  const [isExpanded, setIsExpanded] = useState(false);
  const hasDetails = Boolean(event.detailsJson);

  return (
    <div className="flex flex-col gap-1.5 px-3 py-2">
      <button
        type="button"
        onClick={() => setIsExpanded((v) => !v)}
        className={cn("flex w-full gap-2 text-left", isExpanded ? "items-start" : "items-center")}
      >
        <Badge variant={LEVEL_VARIANT[event.level]} className="shrink-0">
          {t(`activity.levels.${event.level}`)}
        </Badge>
        <span className="shrink-0 text-xs text-text-secondary">{event.category}</span>
        <span className={cn("min-w-0 flex-1 text-sm text-text-primary", isExpanded ? "whitespace-pre-wrap" : "truncate")}>
          {event.summary}
        </span>
        <span className="shrink-0 text-xs text-text-secondary">{formatDateTime(event.timestamp, i18n.language)}</span>
        {isExpanded ? (
          <ChevronUp className="h-3 w-3 shrink-0 text-text-secondary" />
        ) : (
          <ChevronDown className="h-3 w-3 shrink-0 text-text-secondary" />
        )}
      </button>

      {isExpanded ? (
        <div className="flex flex-col gap-1.5 pl-0.5">
          <p className="text-xs text-text-secondary">
            {event.module} · {event.action} · {event.status}
            {event.durationMs !== null ? ` · ${t("activity.durationLabel", { value: event.durationMs })}` : ""}
          </p>
          {hasDetails ? (
            <pre className="whitespace-pre-wrap break-all rounded bg-surface-secondary p-2 text-xs text-text-primary">{event.detailsJson}</pre>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

export function ActivityPage() {
  const { t } = useTranslation();
  const events = useActivityStore((s) => s.events);
  const isLoading = useActivityStore((s) => s.isLoading);
  const load = useActivityStore((s) => s.load);
  const clear = useActivityStore((s) => s.clear);
  const pushToast = useToastStore((s) => s.push);

  const [levelFilter, setLevelFilter] = useState(ALL);
  const [categoryFilter, setCategoryFilter] = useState(ALL);
  const [showSystemEvents, setShowSystemEvents] = useState(false);
  const [isClearOpen, setIsClearOpen] = useState(false);

  useEffect(() => {
    void load();
    const id = window.setInterval(() => void load(), POLL_INTERVAL_MS);
    return () => window.clearInterval(id);
  }, [load]);

  async function handleClear() {
    try {
      await clear();
    } catch (err) {
      pushToast({ variant: "error", title: t("activity.clearError"), description: errorMessage(err) });
    }
  }

  function toggleSystemEvents(next: boolean) {
    setShowSystemEvents(next);
    // Hiding SYSTEM again while it was the active category filter would
    // otherwise leave the feed silently empty with no obvious reason why.
    if (!next && categoryFilter === SYSTEM_CATEGORY) setCategoryFilter(ALL);
  }

  const visibleEvents = useMemo(
    () => (showSystemEvents ? events : events.filter((event) => event.category !== SYSTEM_CATEGORY)),
    [events, showSystemEvents],
  );

  // Built from whatever's actually visible rather than a hardcoded list —
  // a new category showing up in the backend needs zero frontend changes,
  // and SYSTEM only appears here once its own toggle is on.
  const categories = useMemo(() => Array.from(new Set(visibleEvents.map((event) => event.category))).sort(), [visibleEvents]);

  const filtered = visibleEvents.filter(
    (event) => (levelFilter === ALL || event.level === levelFilter) && (categoryFilter === ALL || event.category === categoryFilter),
  );

  return (
    <div className="flex max-w-2xl flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-2">
          <Select
            ariaLabel={t("activity.levelFilter")}
            value={levelFilter}
            onValueChange={setLevelFilter}
            options={[{ value: ALL, label: t("activity.allLevels") }, ...LEVELS.map((level) => ({ value: level, label: t(`activity.levels.${level}`) }))]}
          />
          <Select
            ariaLabel={t("activity.categoryFilter")}
            value={categoryFilter}
            onValueChange={setCategoryFilter}
            options={[{ value: ALL, label: t("activity.allCategories") }, ...categories.map((category) => ({ value: category, label: category }))]}
          />
        </div>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <IconButton icon={<MoreHorizontal className="h-4 w-4" />} label={t("activity.options")} />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onSelect={() => toggleSystemEvents(!showSystemEvents)}>
              {showSystemEvents ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
              {showSystemEvents ? t("activity.hideSystemEvents") : t("activity.showSystemEvents")}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => setIsClearOpen(true)} disabled={events.length === 0}>
              <Trash2 className="h-3.5 w-3.5" />
              {t("activity.clear")}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      {isLoading && events.length === 0 ? (
        <LoadingState label={t("activity.loading")} />
      ) : filtered.length === 0 ? (
        <EmptyState
          icon={<Activity className="h-6 w-6" />}
          title={events.length === 0 ? t("activity.emptyTitle") : t("activity.emptyFilteredTitle")}
          description={events.length === 0 ? t("activity.emptyDescription") : t("activity.emptyFilteredDescription")}
        />
      ) : (
        <Card className="flex flex-col overflow-hidden">
          <div className="flex flex-col divide-y divide-border">
            {filtered.map((event) => (
              <ActivityRow key={event.id} event={event} />
            ))}
          </div>
        </Card>
      )}

      <ConfirmDialog
        open={isClearOpen}
        onOpenChange={setIsClearOpen}
        title={t("activity.clearConfirmTitle")}
        description={t("activity.clearConfirmDescription")}
        confirmLabel={t("activity.clear")}
        cancelLabel={t("common.cancel")}
        danger
        onConfirm={() => void handleClear()}
      />
    </div>
  );
}
