import { AudioLines, Fish, Layers, Server, Volume2, Zap } from "lucide-react";
import { useEffect, useState, type ComponentType, type SVGProps } from "react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/design-system/Badge";
import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { Field } from "@/design-system/Field";
import { Input } from "@/design-system/Input";
import { LoadingState } from "@/design-system/LoadingState";
import { AzureIcon, ElevenLabsIcon, GoogleIcon, OpenAiIcon } from "@/design-system/ProviderIcons";
import { Switch } from "@/design-system/Switch";
import { errorMessage } from "@/lib/errorMessage";
import {
  addTtsProvider,
  listTtsProviders,
  removeTtsProvider,
  setCurrentTtsProvider,
  setTtsProviderEnabled,
  testTtsProvider,
  updateTtsProvider,
  type TtsBackendKind,
  type TtsConnectionTestResult,
  type TtsProviderEntry,
  type TtsProvidersSettings,
} from "@/lib/ipc/settings";
import { useToastStore } from "@/stores/toastStore";

// Lo que cada servicio usa cuando el campo Modelo queda vacío.
const MODEL_PLACEHOLDERS: Partial<Record<TtsBackendKind, string>> = {
  fishaudio: "s2.1-pro-free",
  groq: "canopylabs/orpheus-v1-english",
  together: "canopylabs/orpheus-3b-0.1-ft",
};

const DEFAULT_HTTP_BASE_URL = "http://localhost:8880/v1";

const KIND_ICONS: Record<TtsBackendKind, ComponentType<SVGProps<SVGSVGElement>>> = {
  native: Volume2,
  elevenlabs: ElevenLabsIcon,
  openai: OpenAiIcon,
  google: GoogleIcon,
  azure: AzureIcon,
  fishaudio: Fish,
  deepgram: AudioLines,
  groq: Zap,
  together: Layers,
  http: Server,
};

export function TtsSettings() {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);

  const [isLoading, setIsLoading] = useState(true);
  const [providers, setProviders] = useState<TtsProvidersSettings>({ providers: [], currentProviderId: null });
  const [editingId, setEditingId] = useState<string | null>(null);

  async function refresh() {
    const next = await listTtsProviders();
    setProviders(next);
    return next;
  }

  useEffect(() => {
    refresh()
      .catch((err) => pushToast({ variant: "error", title: t("settings.tts.saveError"), description: errorMessage(err) }))
      .finally(() => setIsLoading(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function handleAdd(kind: Exclude<TtsBackendKind, "native">) {
    try {
      const added = await addTtsProvider({ kind, baseUrl: kind === "http" ? DEFAULT_HTTP_BASE_URL : undefined });
      await refresh();
      setEditingId(added.id);
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.tts.saveError"), description: errorMessage(err) });
    }
  }

  async function handleToggleEnabled(entry: TtsProviderEntry, enabled: boolean) {
    try {
      await setTtsProviderEnabled(entry.id, enabled);
      await refresh();
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.tts.saveError"), description: errorMessage(err) });
    }
  }

  async function handleUseNow(entry: TtsProviderEntry) {
    try {
      await setCurrentTtsProvider(entry.id);
      await refresh();
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.tts.saveError"), description: errorMessage(err) });
    }
  }

  async function handleRemove(entry: TtsProviderEntry) {
    try {
      await removeTtsProvider(entry.id);
      if (editingId === entry.id) setEditingId(null);
      await refresh();
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.tts.saveError"), description: errorMessage(err) });
    }
  }

  if (isLoading) {
    return <LoadingState label={t("settings.tts.loading")} />;
  }

  const existingKinds = new Set(providers.providers.map((p) => p.kind));

  return (
    <div className="flex max-w-xl flex-col gap-4">
      <Card className="flex flex-col gap-4 p-4">
        <div>
          <p className="text-sm font-medium text-text-primary">{t("settings.tts.title")}</p>
          <p className="text-xs text-text-secondary">{t("settings.tts.description")}</p>
        </div>

        <div>
          <p className="mb-2 text-xs font-medium text-text-secondary">{t("settings.tts.addSectionTitle")}</p>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
            {(["elevenlabs", "openai", "google", "azure", "fishaudio", "deepgram", "groq", "together"] as const).map((kind) =>
              existingKinds.has(kind) ? null : (
                <ProviderTile key={kind} icon={KIND_ICONS[kind]} label={t(`settings.tts.kinds.${kind}`)} onClick={() => void handleAdd(kind)} />
              ),
            )}
            <ProviderTile icon={KIND_ICONS.http} label={t("settings.tts.addOther")} onClick={() => void handleAdd("http")} />
          </div>
        </div>

        <div className="flex flex-col divide-y divide-border border-t border-border">
          {providers.providers.map((entry) => {
            const Icon = KIND_ICONS[entry.kind];
            const isNative = entry.kind === "native";
            return (
              <div key={entry.id} className="flex flex-col gap-3 py-3.5 first:pt-4">
                <div className="flex items-start justify-between gap-4">
                  <div className="flex items-start gap-2.5">
                    <Icon className="mt-0.5 h-4 w-4 shrink-0 text-text-secondary" />
                    <div>
                      <div className="flex items-center gap-2">
                        <p className="text-sm font-medium text-text-primary">{entry.label}</p>
                        {providers.currentProviderId === entry.id && <Badge variant="success">{t("settings.tts.inUse")}</Badge>}
                      </div>
                      {isNative ? (
                        <p className="mt-1 text-xs text-text-secondary">{t("settings.tts.alwaysAvailable")}</p>
                      ) : null}
                    </div>
                  </div>
                  {isNative ? null : (
                    <div className="flex shrink-0 gap-2">
                      <Button size="sm" variant="secondary" onClick={() => setEditingId(entry.id)}>
                        {t("settings.tts.edit")}
                      </Button>
                      <Button size="sm" variant="ghost" onClick={() => void handleRemove(entry)}>
                        {t("settings.tts.remove")}
                      </Button>
                    </div>
                  )}
                </div>

                <div className="flex flex-wrap items-center gap-4">
                  {isNative ? null : (
                    <label className="flex items-center gap-2 text-xs text-text-secondary" title={t("settings.tts.enableTitle")}>
                      <Switch
                        checked={entry.enabled}
                        onCheckedChange={(next) => void handleToggleEnabled(entry, next)}
                        label={t("settings.tts.enable")}
                      />
                      {t("settings.tts.enable")}
                    </label>
                  )}
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={(!isNative && !entry.enabled) || providers.currentProviderId === entry.id}
                    onClick={() => void handleUseNow(entry)}
                  >
                    {t("settings.tts.useNow")}
                  </Button>
                </div>

                {editingId === entry.id && (
                  <ProviderEditForm
                    entry={entry}
                    onClose={() => setEditingId(null)}
                    onSaved={() => {
                      void refresh();
                    }}
                  />
                )}
              </div>
            );
          })}
        </div>
      </Card>
    </div>
  );
}

function ProviderEditForm({ entry, onClose, onSaved }: { entry: TtsProviderEntry; onClose: () => void; onSaved: () => void }) {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);

  const [label, setLabel] = useState(entry.label);
  const [baseUrl, setBaseUrl] = useState(entry.baseUrl ?? DEFAULT_HTTP_BASE_URL);
  const [region, setRegion] = useState(entry.region ?? "");
  const [model, setModel] = useState(entry.model ?? "");
  const [apiKey, setApiKey] = useState("");
  const [testResult, setTestResult] = useState<TtsConnectionTestResult | null>(null);
  const [testFailed, setTestFailed] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  const isHttp = entry.kind === "http";
  const isAzure = entry.kind === "azure";
  // Servicios con un modelo elegible (cada uno trae uno por defecto).
  const hasModel = entry.kind === "fishaudio" || entry.kind === "groq" || entry.kind === "together";
  const modelPlaceholder = MODEL_PLACEHOLDERS[entry.kind] ?? "";

  async function handleTest() {
    setIsTesting(true);
    setTestFailed(false);
    setTestResult(null);
    try {
      const result = await testTtsProvider(
        entry.kind,
        isHttp ? baseUrl : null,
        isAzure ? region : null,
        apiKey || null,
        t("settings.tts.testPhrase"),
        entry.id,
        hasModel ? model : null,
      );
      setTestResult(result);
    } catch (err) {
      setTestFailed(true);
      pushToast({ variant: "error", title: t("settings.tts.testError"), description: errorMessage(err) });
    } finally {
      setIsTesting(false);
    }
  }

  async function handleSave() {
    setIsSaving(true);
    try {
      await updateTtsProvider(entry.id, {
        label,
        baseUrl: isHttp ? baseUrl : undefined,
        region: isAzure ? region : undefined,
        model: hasModel ? model : undefined,
        apiKey: apiKey === "" ? undefined : apiKey,
      });
      pushToast({ variant: "success", title: t("settings.tts.saved") });
      onSaved();
      onClose();
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.tts.saveError"), description: errorMessage(err) });
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="flex flex-col gap-4 rounded-lg border border-border bg-surface-secondary p-4">
      <div className="flex items-center justify-between gap-4">
        <p className="text-sm font-medium text-text-primary">{t("settings.tts.editing", { name: t(`settings.tts.kinds.${entry.kind}`) })}</p>
        <Button size="sm" variant="ghost" onClick={onClose}>
          {t("settings.tts.close")}
        </Button>
      </div>

      <Field label={t("settings.tts.name")}>
        <Input value={label} onChange={(event) => setLabel(event.target.value)} />
      </Field>

      <div className="grid grid-cols-2 gap-4">
        {isHttp && (
          <Field label={t("settings.tts.baseUrl")}>
            <Input value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} placeholder={DEFAULT_HTTP_BASE_URL} />
          </Field>
        )}
        {isAzure && (
          <Field label={t("settings.tts.region")} hint={t("settings.tts.regionHint")}>
            <Input value={region} onChange={(event) => setRegion(event.target.value)} placeholder="eastus" />
          </Field>
        )}
        {hasModel && (
          <Field label={t("settings.tts.model")} hint={t(`settings.tts.modelHints.${entry.kind}`)}>
            <Input value={model} onChange={(event) => setModel(event.target.value)} placeholder={modelPlaceholder} />
          </Field>
        )}
        <Field label={t("settings.tts.apiKey")} hint={entry.hasApiKey ? t("settings.tts.apiKeyStored") : t(`settings.tts.keyHints.${entry.kind}`, { defaultValue: "" }) || undefined}>
          <Input
            type="password"
            value={apiKey}
            onChange={(event) => setApiKey(event.target.value)}
            placeholder={entry.hasApiKey ? "••••••••" : t("settings.tts.apiKeyPlaceholder")}
          />
        </Field>
      </div>

      {testResult ? (
        <p className="text-xs text-text-secondary">
          {testResult.voicesFound != null ? t("settings.tts.voicesFound", { count: testResult.voicesFound }) : t("settings.tts.testPlayedHint")}
        </p>
      ) : null}

      <div className="flex justify-end gap-2">
        <Button variant="secondary" onClick={() => void handleTest()} disabled={isTesting || (isHttp && !baseUrl) || (isAzure && !region)}>
          {isTesting ? t("settings.tts.testing") : t("settings.tts.test")}
        </Button>
        {testFailed && !testResult ? <Badge variant="error">{t("settings.tts.notConnected")}</Badge> : null}
        <Button onClick={() => void handleSave()} disabled={isSaving}>
          {isSaving ? t("identity.saving") : t("identity.save")}
        </Button>
      </div>
    </div>
  );
}

/** Mismo tile de `AiProvidersSettings.tsx` (h-20, logo arriba del nombre,
 * insignia "+" en la esquina) — mismo pedido del usuario de que la lista
 * de proveedores se distinga de un vistazo, ahora también en Motor de voz. */
function ProviderTile({
  icon: Icon,
  label,
  onClick,
}: {
  icon: ComponentType<SVGProps<SVGSVGElement>>;
  label: string;
  onClick: () => void;
}) {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={t("settings.tts.addAction", { name: label })}
      className="relative flex h-20 flex-col items-center justify-center gap-1.5 rounded-lg border border-border bg-surface-secondary px-2 text-center transition duration-150 hover:brightness-110 active:scale-[0.97] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background"
    >
      <span className="absolute right-1.5 top-1.5 flex h-4 w-4 items-center justify-center rounded-full bg-accent text-[10px] leading-none text-accent-foreground">
        +
      </span>
      <Icon className="h-6 w-6 text-text-primary" />
      <span className="text-xs font-medium text-text-primary">{label}</span>
    </button>
  );
}
