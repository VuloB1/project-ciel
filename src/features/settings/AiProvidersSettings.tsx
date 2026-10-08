import { Bot, Cpu, Fish, Layers, Pin, Rocket, Route, Server, Wind, Zap } from "lucide-react";
import { useEffect, useState, type ComponentType, type SVGProps } from "react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/design-system/Badge";
import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { Field } from "@/design-system/Field";
import { IconButton } from "@/design-system/IconButton";
import { Input } from "@/design-system/Input";
import { LoadingState } from "@/design-system/LoadingState";
import { AnthropicIcon, GeminiIcon, OpenAiIcon } from "@/design-system/ProviderIcons";
import { Select } from "@/design-system/Select";
import { Slider } from "@/design-system/Slider";
import { Switch } from "@/design-system/Switch";
import { errorMessage } from "@/lib/errorMessage";
import { useAppEvent } from "@/lib/ipc/events";
import {
  addLlmProvider,
  listLlmProviders,
  removeLlmProvider,
  setCurrentLlmProvider,
  setLlmProviderEnabled,
  setLlmProviderPinned,
  testLlmProvider,
  updateLlmProvider,
  type PrivacyMode,
  type ConnectionStatus,
  type LlmProviderEntry,
  type LlmProviderKind,
  type LlmProvidersSettings,
} from "@/lib/ipc/settings";
import { useToastStore } from "@/stores/toastStore";

import { ProviderUsagePanel } from "./ProviderUsagePanel";

// El endpoint `/v1/models` de un servidor compatible con OpenAI (LM Studio,
// Ollama, OpenAI real, etc.) solo devuelve `id` — no hay forma estándar de
// preguntarle al servidor si un modelo acepta imágenes (ver el comentario de
// `ModelCapabilities` en `aivt-provider-llm`, ADR-003). Esto es una
// heurística sobre el NOMBRE del modelo, no una detección real: solo prende
// el switch cuando hay una coincidencia clara con familias de modelos de
// visión conocidas — nunca lo apaga, para no pisar una elección manual del
// usuario sobre un modelo que no reconocemos.
const VISION_NAME_PATTERN =
  /(^|[-_/.])(vl|vlm|vision|llava|moondream|pixtral|internvl|cogvlm|minicpm-v|florence)([-_/.]|$)|gpt-4o|gpt-4-vision|gpt-4\.1|gemini|claude-3|claude-.*-4/i;

function looksVisionCapable(modelId: string): boolean {
  return VISION_NAME_PATTERN.test(modelId);
}

const KIND_ICONS: Record<LlmProviderKind, ComponentType<SVGProps<SVGSVGElement>>> = {
  openai: OpenAiIcon,
  anthropic: AnthropicIcon,
  gemini: GeminiIcon,
  groq: Zap,
  openrouter: Route,
  cerebras: Cpu,
  mistral: Wind,
  deepseek: Fish,
  together: Layers,
  xai: Bot,
  sambanova: Rocket,
  generic: Server,
};

// Orden de las tarjetas sugeridas. Todas menos `generic` traen su URL fija.
const PRIVACY_MODES: PrivacyMode[] = ["full", "public_only", "minimal"];

const SUGGESTED_KINDS = ["openai", "anthropic", "gemini", "groq", "openrouter", "cerebras", "mistral", "deepseek", "together", "xai", "sambanova"] as const;

export function AiProvidersSettings() {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);

  const [isLoading, setIsLoading] = useState(true);
  const [providers, setProviders] = useState<LlmProvidersSettings>({ providers: [], currentProviderId: null, preferredProviderId: null });
  const [editingId, setEditingId] = useState<string | null>(null);

  async function refresh() {
    const next = await listLlmProviders();
    setProviders(next);
    return next;
  }

  useEffect(() => {
    refresh()
      .catch((err) => pushToast({ variant: "error", title: t("settings.aiProviders.saveError"), description: errorMessage(err) }))
      .finally(() => setIsLoading(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // La rotación automática por falta de crédito (`FailoverLLMProvider`)
  // puede cambiar cuál proveedor está "en uso" sin que el usuario haga
  // nada acá — se refresca sola en vez de dejar el badge "En uso"
  // desactualizado hasta el próximo refresh manual.
  useAppEvent((event) => {
    if (event.type === "llm_provider_switched") {
      void refresh();
    }
  });

  async function handleAdd(kind: LlmProviderKind) {
    try {
      const added = await addLlmProvider({ kind, baseUrl: kind === "generic" ? "http://localhost:1234/v1" : undefined });
      await refresh();
      setEditingId(added.id);
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.aiProviders.saveError"), description: errorMessage(err) });
    }
  }

  async function handleToggleEnabled(entry: LlmProviderEntry, enabled: boolean) {
    try {
      await setLlmProviderEnabled(entry.id, enabled);
      await refresh();
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.aiProviders.saveError"), description: errorMessage(err) });
    }
  }

  async function handleTogglePinned(entry: LlmProviderEntry, pinned: boolean) {
    try {
      await setLlmProviderPinned(entry.id, pinned);
      await refresh();
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.aiProviders.saveError"), description: errorMessage(err) });
    }
  }

  async function handleUseNow(entry: LlmProviderEntry) {
    try {
      await setCurrentLlmProvider(entry.id);
      await refresh();
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.aiProviders.saveError"), description: errorMessage(err) });
    }
  }

  async function handleRemove(entry: LlmProviderEntry) {
    try {
      await removeLlmProvider(entry.id);
      if (editingId === entry.id) setEditingId(null);
      await refresh();
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.aiProviders.saveError"), description: errorMessage(err) });
    }
  }

  if (isLoading) {
    return <LoadingState label={t("settings.aiProviders.loading")} />;
  }

  const existingKinds = new Set(providers.providers.map((p) => p.kind));

  return (
    <div className="flex max-w-xl flex-col gap-4">
      <Card className="flex flex-col gap-4 p-4">
        <div>
          <p className="text-sm font-medium text-text-primary">{t("settings.aiProviders.title")}</p>
          <p className="text-xs text-text-secondary">{t("settings.aiProviders.description")}</p>
        </div>

        <div>
          <p className="mb-2 text-xs font-medium text-text-secondary">{t("settings.aiProviders.addSectionTitle")}</p>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
            {SUGGESTED_KINDS.map((kind) =>
              existingKinds.has(kind) ? null : (
                <ProviderTile key={kind} icon={KIND_ICONS[kind]} label={t(`settings.aiProviders.kinds.${kind}`)} onClick={() => void handleAdd(kind)} />
              ),
            )}
            <ProviderTile icon={KIND_ICONS.generic} label={t("settings.aiProviders.addOther")} onClick={() => void handleAdd("generic")} />
          </div>
        </div>

        {providers.providers.length === 0 ? (
          <p className="text-xs text-text-secondary">{t("settings.aiProviders.empty")}</p>
        ) : (
          <div className="flex flex-col divide-y divide-border border-t border-border">
            {providers.providers.map((entry) => {
              const Icon = KIND_ICONS[entry.kind];
              return (
                <div key={entry.id} className="flex flex-col gap-3 py-3.5 first:pt-4">
                  <div className="flex items-start justify-between gap-4">
                    <div className="flex items-start gap-2.5">
                      <Icon className="mt-0.5 h-4 w-4 shrink-0 text-text-secondary" />
                      <div>
                        <div className="flex items-center gap-2">
                          <p className="text-sm font-medium text-text-primary">{entry.label}</p>
                          <span className="text-xs text-text-secondary">{t(`settings.aiProviders.kinds.${entry.kind}`)}</span>
                          {providers.currentProviderId === entry.id && <Badge variant="success">{t("settings.aiProviders.inUse")}</Badge>}
                          <span title={t(entry.isLocal ? "settings.aiProviders.localTitle" : "settings.aiProviders.cloudTitle")}>
                            <Badge variant={entry.isLocal ? "success" : "neutral"}>
                              {t(entry.isLocal ? "settings.aiProviders.local" : "settings.aiProviders.cloud")}
                            </Badge>
                          </span>
                          {entry.privacyMode !== "full" && (
                            <span title={t(`settings.aiProviders.privacy.modes.${entry.privacyMode}.hint`)}>
                              <Badge variant="warning">{t(`settings.aiProviders.privacy.modes.${entry.privacyMode}.badge`)}</Badge>
                            </span>
                          )}
                          {providers.preferredProviderId === entry.id && providers.currentProviderId !== entry.id && (
                            <span title={t("settings.aiProviders.preferredTitle")}>
                              <Badge variant="neutral">{t("settings.aiProviders.preferred")}</Badge>
                            </span>
                          )}
                        </div>
                        <p className="mt-1 text-xs text-text-secondary">{entry.model || t("settings.aiProviders.noModel")}</p>
                      </div>
                    </div>
                    <div className="flex shrink-0 gap-2">
                      <Button size="sm" variant="secondary" onClick={() => setEditingId(entry.id)}>
                        {t("settings.aiProviders.edit")}
                      </Button>
                      <Button size="sm" variant="ghost" onClick={() => void handleRemove(entry)}>
                        {t("settings.aiProviders.remove")}
                      </Button>
                    </div>
                  </div>

                  <div className="flex flex-wrap items-center gap-3">
                    <label
                      className="flex items-center gap-2 text-xs text-text-secondary"
                      title={t("settings.aiProviders.enableTitle")}
                    >
                      <Switch
                        checked={entry.enabled}
                        onCheckedChange={(next) => void handleToggleEnabled(entry, next)}
                        label={t("settings.aiProviders.enable")}
                      />
                      {t("settings.aiProviders.enable")}
                    </label>
                    <IconButton
                      icon={<Pin className="h-4 w-4" fill={entry.pinned ? "currentColor" : "none"} />}
                      label={t("settings.aiProviders.pin")}
                      title={t("settings.aiProviders.pinTitle")}
                      active={entry.pinned}
                      disabled={!entry.enabled}
                      onClick={() => void handleTogglePinned(entry, !entry.pinned)}
                    />
                    <Button
                      size="sm"
                      variant="secondary"
                      disabled={!entry.enabled || providers.currentProviderId === entry.id}
                      onClick={() => void handleUseNow(entry)}
                    >
                      {t("settings.aiProviders.useNow")}
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
        )}
        {providers.providers.length > 0 && <ProviderUsagePanel providers={providers.providers} />}
      </Card>
    </div>
  );
}

function ProviderEditForm({ entry, onClose, onSaved }: { entry: LlmProviderEntry; onClose: () => void; onSaved: () => void }) {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);

  const [label, setLabel] = useState(entry.label);
  const [baseUrl, setBaseUrl] = useState(entry.baseUrl ?? "http://localhost:1234/v1");
  const [model, setModel] = useState(entry.model);
  const [temperaturePercent, setTemperaturePercent] = useState(Math.round(entry.temperature * 100));
  const [maxTokens, setMaxTokens] = useState(entry.maxTokens);
  const [apiKey, setApiKey] = useState("");
  const [visionEnabled, setVisionEnabled] = useState(entry.visionEnabled);
  const [privacyMode, setPrivacyMode] = useState<PrivacyMode>(entry.privacyMode);
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [testResult, setTestResult] = useState<ConnectionStatus | null>(null);
  const [testFailed, setTestFailed] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  const isGeneric = entry.kind === "generic";

  async function handleTest() {
    setIsTesting(true);
    setTestFailed(false);
    setTestResult(null);
    try {
      const status = await testLlmProvider(entry.kind, isGeneric ? baseUrl : null, apiKey || null, entry.id);
      setTestResult(status);
      const models = status.models.map((m) => m.id);
      setAvailableModels(models);
      const [firstModel] = models;
      let resolvedModel = model;
      if (firstModel && !models.includes(model)) {
        setModel(firstModel);
        resolvedModel = firstModel;
      }
      if (resolvedModel && looksVisionCapable(resolvedModel) && !visionEnabled) {
        setVisionEnabled(true);
        pushToast({ variant: "success", title: t("settings.aiProviders.visionAutoDetected") });
      }
    } catch (err) {
      setTestFailed(true);
      pushToast({ variant: "error", title: t("settings.aiProviders.testError"), description: errorMessage(err) });
    } finally {
      setIsTesting(false);
    }
  }

  async function handleSave() {
    setIsSaving(true);
    try {
      await updateLlmProvider(entry.id, {
        label,
        baseUrl: isGeneric ? baseUrl : undefined,
        model,
        temperature: temperaturePercent / 100,
        maxTokens,
        apiKey: apiKey === "" ? undefined : apiKey,
        // El modo mínimo no manda imágenes: la visión queda apagada (el backend también lo fuerza).
        visionEnabled: visionEnabled && privacyMode !== "minimal",
        privacyMode,
      });
      pushToast({ variant: "success", title: t("settings.aiProviders.saved") });
      onSaved();
      onClose();
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.aiProviders.saveError"), description: errorMessage(err) });
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="flex flex-col gap-4 rounded-lg border border-border bg-surface-secondary p-4">
      <div className="flex items-center justify-between gap-4">
        <p className="text-sm font-medium text-text-primary">
          {t("settings.aiProviders.editing", { name: t(`settings.aiProviders.kinds.${entry.kind}`) })}
        </p>
        <Button size="sm" variant="ghost" onClick={onClose}>
          {t("settings.aiProviders.close")}
        </Button>
      </div>

      <Field label={t("settings.aiProviders.name")}>
        <Input value={label} onChange={(event) => setLabel(event.target.value)} />
      </Field>

      <div className="grid grid-cols-2 gap-4">
        {isGeneric && (
          <Field label={t("settings.aiProviders.baseUrl")}>
            <Input value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} />
          </Field>
        )}
        <Field
          label={t("settings.aiProviders.apiKey")}
          hint={
            entry.hasApiKey
              ? t("settings.aiProviders.apiKeyStored")
              : t(`settings.aiProviders.keyHints.${entry.kind}`, { defaultValue: "" }) || undefined
          }
        >
          <Input
            type="password"
            value={apiKey}
            onChange={(event) => setApiKey(event.target.value)}
            placeholder={entry.hasApiKey ? "••••••••" : t("settings.aiProviders.apiKeyPlaceholder")}
          />
        </Field>
      </div>

      <Field label={t("settings.aiProviders.model")}>
        {availableModels.length > 0 ? (
          <Select
            ariaLabel={t("settings.aiProviders.model")}
            value={model}
            onValueChange={setModel}
            options={availableModels.map((id) => ({ value: id, label: id }))}
          />
        ) : (
          <Input value={model} onChange={(event) => setModel(event.target.value)} placeholder={t("settings.aiProviders.modelPlaceholder")} />
        )}
      </Field>

      <div className="grid grid-cols-2 gap-4">
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center justify-between text-sm">
            <span className="text-text-primary">{t("settings.aiProviders.temperature")}</span>
            <span className="tabular-nums text-text-secondary">{(temperaturePercent / 100).toFixed(2)}</span>
          </div>
          <Slider label={t("settings.aiProviders.temperature")} value={temperaturePercent} onValueChange={setTemperaturePercent} />
        </div>
        <Field label={t("settings.aiProviders.maxTokens")}>
          <Input type="number" min={1} value={maxTokens} onChange={(event) => setMaxTokens(Number(event.target.value) || 0)} />
        </Field>
      </div>

      {testResult ? (
        <p className="text-xs text-text-secondary">
          {t("settings.aiProviders.latency", { ms: testResult.latencyMs ?? 0 })} · {t("settings.aiProviders.modelsFound", { count: testResult.models.length })}
        </p>
      ) : null}

      <div className="flex flex-col gap-2 border-t border-border pt-4">
        <Field
          label={t("settings.aiProviders.privacy.title")}
          hint={entry.isLocal ? t("settings.aiProviders.privacy.localNote") : t("settings.aiProviders.privacy.cloudNote")}
        >
          <Select
            ariaLabel={t("settings.aiProviders.privacy.title")}
            value={privacyMode}
            onValueChange={(value) => setPrivacyMode(value as PrivacyMode)}
            options={PRIVACY_MODES.map((mode) => ({ value: mode, label: t(`settings.aiProviders.privacy.modes.${mode}.label`) }))}
          />
        </Field>
        <p className="text-xs text-text-secondary">{t(`settings.aiProviders.privacy.modes.${privacyMode}.hint`)}</p>
      </div>

      <div className="flex items-center justify-between gap-4 border-t border-border pt-4">
        <div>
          <p className="text-sm text-text-primary">{t("settings.aiProviders.vision")}</p>
          <p className="text-xs text-text-secondary">
            {privacyMode === "minimal" ? t("settings.aiProviders.privacy.visionOff") : t("settings.aiProviders.visionHint")}
          </p>
        </div>
        <Switch
          label={t("settings.aiProviders.vision")}
          checked={visionEnabled && privacyMode !== "minimal"}
          disabled={privacyMode === "minimal"}
          onCheckedChange={setVisionEnabled}
        />
      </div>

      <div className="flex justify-end gap-2">
        <Button variant="secondary" onClick={() => void handleTest()} disabled={isTesting || (isGeneric && !baseUrl)}>
          {isTesting ? t("settings.aiProviders.testing") : t("settings.aiProviders.test")}
        </Button>
        {testFailed && !testResult ? <Badge variant="error">{t("settings.aiProviders.notConnected")}</Badge> : null}
        <Button onClick={() => void handleSave()} disabled={isSaving}>
          {isSaving ? t("identity.saving") : t("identity.save")}
        </Button>
      </div>
    </div>
  );
}

/** Tile de "agregar proveedor": más alto que un botón normal (h-20) y con
 * su logo bien visible arriba del nombre — pedido explícito del usuario
 * para que la lista de proveedores se distinga de un vistazo, no solo por
 * texto. La insignia "+" en la esquina deja claro que es una acción de
 * agregar sin tener que repetir la palabra en cada tile. */
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
      aria-label={t("settings.aiProviders.addAction", { name: label })}
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
