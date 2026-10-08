import { save } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, CheckCircle2, Info, XCircle, type LucideIcon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge, type BadgeVariant } from "@/design-system/Badge";
import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { LoadingState } from "@/design-system/LoadingState";
import { errorMessage } from "@/lib/errorMessage";
import { formatDateTime } from "@/lib/formatDate";
import {
  getDiagnosticsText,
  runDiagnostics,
  saveDiagnostics,
  type DiagnosticsReport,
  type Severity,
} from "@/lib/ipc/diagnostics";
import { useToastStore } from "@/stores/toastStore";

import { formatBytes } from "./BackupSettings";

const SEVERITY_ICON: Record<Severity, LucideIcon> = {
  ok: CheckCircle2,
  info: Info,
  warning: AlertTriangle,
  error: XCircle,
};

const SEVERITY_COLOR: Record<Severity, string> = {
  ok: "text-success",
  info: "text-text-secondary",
  warning: "text-warning",
  error: "text-error",
};

const SEVERITY_BADGE: Record<Severity, BadgeVariant> = {
  ok: "success",
  info: "neutral",
  warning: "warning",
  error: "error",
};

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-4 text-xs">
      <span className="text-text-secondary">{label}</span>
      <span className="break-all text-right tabular-nums text-text-primary">{value}</span>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-2 border-t border-border p-4">
      <p className="text-sm font-medium text-text-primary">{title}</p>
      {children}
    </div>
  );
}

/** Centro de diagnóstico: el estado de la app en una pantalla, con comprobaciones
 * explicadas en lenguaje normal y un informe de texto (sin secretos) para compartir. */
export function DiagnosticsSettings() {
  const { t, i18n } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const [report, setReport] = useState<DiagnosticsReport | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const load = useCallback(async () => {
    setIsLoading(true);
    try {
      setReport(await runDiagnostics());
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.diagnostics.runError"), description: errorMessage(err) });
    } finally {
      setIsLoading(false);
    }
  }, [pushToast, t]);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(await getDiagnosticsText());
      pushToast({ variant: "success", title: t("settings.diagnostics.copied") });
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.diagnostics.copyError"), description: errorMessage(err) });
    }
  }

  async function handleSave() {
    try {
      const path = await save({
        defaultPath: `Project-Ciel-diagnostico-${new Date().toISOString().slice(0, 10)}.txt`,
        filters: [{ name: "Text", extensions: ["txt"] }],
      });
      if (!path) return;
      await saveDiagnostics(path);
      pushToast({ variant: "success", title: t("settings.diagnostics.saved") });
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.diagnostics.saveError"), description: errorMessage(err) });
    }
  }

  if (!report) {
    return isLoading ? <LoadingState label={t("settings.diagnostics.running")} /> : null;
  }

  const OverallIcon = SEVERITY_ICON[report.overall];

  return (
    <div className="flex max-w-xl flex-col gap-4">
      <Card className="flex flex-col p-0">
        <div className="flex flex-col gap-3 p-4">
          <div className="flex items-start justify-between gap-3">
            <div>
              <p className="text-sm font-medium text-text-primary">{t("settings.diagnostics.title")}</p>
              <p className="text-xs text-text-secondary">{t("settings.diagnostics.description")}</p>
            </div>
            <Badge variant={SEVERITY_BADGE[report.overall]}>
              <OverallIcon className="h-3 w-3" />
              {t(`settings.diagnostics.overall.${report.overall}`)}
            </Badge>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button size="sm" disabled={isLoading} onClick={() => void load()}>
              {isLoading ? t("settings.diagnostics.running") : t("settings.diagnostics.refresh")}
            </Button>
            <Button size="sm" variant="secondary" onClick={() => void handleCopy()}>
              {t("settings.diagnostics.copy")}
            </Button>
            <Button size="sm" variant="secondary" onClick={() => void handleSave()}>
              {t("settings.diagnostics.save")}
            </Button>
          </div>
          <p className="text-xs text-text-secondary">{t("settings.diagnostics.noSecrets")}</p>
          <p className="text-xs text-text-secondary">
            {t("settings.diagnostics.generatedAt", { date: formatDateTime(report.generatedAt, i18n.language) })}
          </p>
        </div>

        <Section title={t("settings.diagnostics.checksTitle")}>
          <ul className="flex flex-col gap-1.5">
            {report.checks.map((check, index) => {
              const Icon = SEVERITY_ICON[check.severity];
              return (
                <li key={`${check.id}-${index}`} className="flex items-start gap-2 text-sm text-text-primary">
                  <Icon className={`mt-0.5 h-4 w-4 shrink-0 ${SEVERITY_COLOR[check.severity]}`} />
                  <span>
                    {t(
                      [
                        `settings.diagnostics.checks.${check.id}.${check.severity}`,
                        `settings.diagnostics.checks.${check.id}.default`,
                        "settings.diagnostics.checks.unknown",
                      ],
                      {
                        ...check.params,
                        size: check.params.bytes ? formatBytes(Number(check.params.bytes)) : undefined,
                        id: check.id,
                      },
                    )}
                  </span>
                </li>
              );
            })}
          </ul>
        </Section>

        <Section title={t("settings.diagnostics.servicesTitle")}>
          {report.services.map((service) => (
            <Row
              key={service.name}
              label={t(`settings.diagnostics.serviceNames.${service.name}`, { defaultValue: service.name })}
              value={t(`settings.diagnostics.serviceStatus.${service.status}`, { defaultValue: service.status })}
            />
          ))}
        </Section>

        <Section title={t("settings.diagnostics.aiTitle")}>
          {report.ai.llm.map((p) => (
            <Row
              key={p.label}
              label={p.label}
              value={[
                p.current ? t("settings.diagnostics.inUse") : null,
                p.ready ? t("settings.diagnostics.ready") : t("settings.diagnostics.notReady"),
                p.cooldownSecs ? t("settings.diagnostics.pausedFor", { seconds: p.cooldownSecs }) : null,
              ]
                .filter(Boolean)
                .join(" · ")}
            />
          ))}
          {report.ai.tts.map((p) => (
            <Row
              key={`tts-${p.label}`}
              label={`${t("settings.diagnostics.voice")}: ${p.label}`}
              value={p.current ? t("settings.diagnostics.inUse") : p.enabled ? t("settings.diagnostics.available") : "—"}
            />
          ))}
          {report.ai.sttModels.map((m) => (
            <Row
              key={`stt-${m.size}`}
              label={`${t("settings.diagnostics.transcription")} ${m.size}`}
              value={m.downloaded ? t("settings.diagnostics.downloaded") : t("settings.diagnostics.notDownloaded")}
            />
          ))}
          <Row
            label={t("settings.diagnostics.semanticSearch")}
            value={report.ai.embeddingDownloaded ? t("settings.diagnostics.downloaded") : t("settings.diagnostics.notDownloaded")}
          />
        </Section>

        <Section title={t("settings.diagnostics.databaseTitle")}>
          <Row
            label={t("settings.diagnostics.integrity")}
            value={report.database.integrityOk ? t("settings.diagnostics.integrityOk") : t("settings.diagnostics.integrityBad")}
          />
          <Row
            label={t("settings.diagnostics.schema")}
            value={`${report.database.schemaVersion} / ${report.database.latestSchemaVersion}`}
          />
          <Row label={t("settings.diagnostics.size")} value={formatBytes(report.database.sizeBytes)} />
          {report.database.counts.map((c) => (
            <Row key={c.table} label={t(`settings.diagnostics.tables.${c.table}`, { defaultValue: c.table })} value={String(c.count)} />
          ))}
        </Section>

        <Section title={t("settings.diagnostics.storageTitle")}>
          <Row label={t("settings.diagnostics.attachments")} value={formatBytes(report.storage.attachmentsBytes)} />
          <Row label={t("settings.diagnostics.avatars")} value={formatBytes(report.storage.avatarsBytes)} />
          <Row label={t("settings.diagnostics.models")} value={formatBytes(report.storage.modelsBytes)} />
          <Row label={t("settings.diagnostics.logs")} value={formatBytes(report.storage.logsBytes)} />
          <Row label={t("settings.diagnostics.preRestore")} value={formatBytes(report.storage.preRestoreBytes)} />
        </Section>

        <Section title={t("settings.diagnostics.systemTitle")}>
          <Row label={t("settings.diagnostics.version")} value={report.system.appVersion} />
          <Row label={t("settings.diagnostics.system")} value={`${report.system.os} (${report.system.arch})`} />
          <Row label={t("settings.diagnostics.dataDir")} value={report.system.dataDir} />
        </Section>

        <Section title={t("settings.diagnostics.problemsTitle")}>
          {report.problems.length === 0 ? (
            <p className="text-xs text-text-secondary">{t("settings.diagnostics.noProblems")}</p>
          ) : (
            <ul className="flex flex-col gap-2">
              {report.problems.map((p, index) => (
                <li key={`${p.timestamp}-${index}`} className="text-xs">
                  <span className="text-text-secondary">
                    {formatDateTime(p.timestamp, i18n.language)} · {p.level} · {p.category}
                  </span>
                  <p className="break-words text-text-primary">{p.summary}</p>
                </li>
              ))}
            </ul>
          )}
        </Section>
      </Card>
    </div>
  );
}
