import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { ConfirmDialog } from "@/design-system/Dialog";
import { Select } from "@/design-system/Select";
import { errorMessage } from "@/lib/errorMessage";
import { formatDateTime } from "@/lib/formatDate";
import { useAppEvent } from "@/lib/ipc/events";
import { getLlmUsage, resetLlmUsage, type LlmProviderEntry, type LlmUsageEntry, type UsagePeriod } from "@/lib/ipc/settings";
import { useToastStore } from "@/stores/toastStore";

const PERIODS: UsagePeriod[] = ["today", "last7_days", "last30_days", "all"];

/** 12 345 -> "12,3 mil" sería ambiguo entre idiomas: se usa el formato compacto del idioma activo. */
function formatCount(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

/** Cuánto se ha usado cada proveedor: peticiones, fallos y tokens estimados. Son cifras
 * locales (nunca el texto de los mensajes) y los tokens son una estimación a partir del
 * texto, no lo que factura el proveedor. */
export function ProviderUsagePanel({ providers }: { providers: LlmProviderEntry[] }) {
  const { t, i18n } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const [period, setPeriod] = useState<UsagePeriod>("last7_days");
  const [usage, setUsage] = useState<LlmUsageEntry[]>([]);
  const [confirmReset, setConfirmReset] = useState(false);

  const load = useCallback(async () => {
    try {
      setUsage(await getLlmUsage(period));
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.aiProviders.usage.loadError"), description: errorMessage(err) });
    }
  }, [period, pushToast, t]);

  useEffect(() => {
    void load();
  }, [load, providers]);

  // Un cambio automático de proveedor suele significar que hubo un fallo: la tabla se pone al día.
  useAppEvent((event) => {
    if (event.type === "llm_provider_switched") void load();
  });

  async function handleReset() {
    try {
      await resetLlmUsage();
      await load();
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.aiProviders.usage.resetError"), description: errorMessage(err) });
    }
  }

  const byId = new Map(usage.map((entry) => [entry.providerId, entry]));
  const totalRequests = usage.reduce((sum, entry) => sum + entry.requests, 0);

  return (
    <div className="flex flex-col gap-3 border-t border-border pt-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-sm font-medium text-text-primary">{t("settings.aiProviders.usage.title")}</p>
          <p className="text-xs text-text-secondary">{t("settings.aiProviders.usage.description")}</p>
        </div>
        <Select
          ariaLabel={t("settings.aiProviders.usage.period")}
          value={period}
          onValueChange={(value) => setPeriod(value as UsagePeriod)}
          options={PERIODS.map((p) => ({ value: p, label: t(`settings.aiProviders.usage.periods.${p}`) }))}
        />
      </div>

      {totalRequests === 0 ? (
        <p className="text-xs text-text-secondary">{t("settings.aiProviders.usage.empty")}</p>
      ) : (
        <ul className="flex flex-col divide-y divide-border">
          {providers.map((provider) => {
            const entry = byId.get(provider.id);
            if (!entry || entry.requests === 0) return null;
            return (
              <li key={provider.id} className="flex flex-col gap-0.5 py-2 text-xs">
                <div className="flex items-baseline justify-between gap-3">
                  <span className="text-sm text-text-primary">{provider.label}</span>
                  <span className="tabular-nums text-text-secondary">{t("settings.aiProviders.usage.requests", { count: entry.requests })}</span>
                </div>
                <p className="text-text-secondary">
                  {t("settings.aiProviders.usage.tokens", {
                    input: formatCount(entry.inputTokensEstimate, i18n.language),
                    output: formatCount(entry.outputTokensEstimate, i18n.language),
                  })}
                  {entry.errors > 0 ? ` · ${t("settings.aiProviders.usage.errors", { count: entry.errors })}` : ""}
                  {entry.lastUsedAt ? ` · ${t("settings.aiProviders.usage.lastUsed", { date: formatDateTime(entry.lastUsedAt, i18n.language) })}` : ""}
                </p>
                {entry.lastError ? <p className="break-words text-warning">{t("settings.aiProviders.usage.lastError", { message: entry.lastError })}</p> : null}
              </li>
            );
          })}
        </ul>
      )}

      <div className="flex items-center justify-between gap-4">
        <p className="text-xs text-text-secondary">{t("settings.aiProviders.usage.estimateNote")}</p>
        <Button size="sm" variant="ghost" disabled={totalRequests === 0 && period !== "all"} onClick={() => setConfirmReset(true)}>
          {t("settings.aiProviders.usage.reset")}
        </Button>
      </div>

      <ConfirmDialog
        open={confirmReset}
        onOpenChange={setConfirmReset}
        title={t("settings.aiProviders.usage.resetConfirmTitle")}
        description={t("settings.aiProviders.usage.resetConfirmDescription")}
        confirmLabel={t("settings.aiProviders.usage.reset")}
        cancelLabel={t("common.cancel")}
        danger
        onConfirm={() => void handleReset()}
      />
    </div>
  );
}
