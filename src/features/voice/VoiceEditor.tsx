import { Check, Square, Volume2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { Field } from "@/design-system/Field";
import { Input } from "@/design-system/Input";
import { LoadingState } from "@/design-system/LoadingState";
import { Select } from "@/design-system/Select";
import { Slider } from "@/design-system/Slider";
import { Switch } from "@/design-system/Switch";
import { errorMessage } from "@/lib/errorMessage";
import { listTtsProviders, type TtsBackendKind } from "@/lib/ipc/settings";
import { getVoiceSettings, listVoices, searchVoices, setVoiceSettings, type VoiceInfo } from "@/lib/ipc/voice";
import { useAutosave } from "@/lib/useAutosave";
import { useToastStore } from "@/stores/toastStore";
import { useVoiceStore } from "@/stores/voiceStore";
import { DEFAULT_VOICE, type Profile, type ProfileVoice } from "@/types/profile";

const PREVIEW_TEXT_MAX_LENGTH = 200;

export function VoiceEditor({ profile }: { profile: Profile }) {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const speakVoice = useVoiceStore((s) => s.speak);
  const stopVoice = useVoiceStore((s) => s.stop);
  const isPreviewing = useVoiceStore(
    (s) =>
      (s.speaking?.profileId === profile.id && s.speaking.messageId === null) ||
      (s.pending?.profileId === profile.id && s.pending.messageId === null),
  );

  const [isLoading, setIsLoading] = useState(true);
  const [draft, setDraft] = useState<ProfileVoice>(DEFAULT_VOICE);
  const [backend, setBackend] = useState<TtsBackendKind>("native");
  /** Enumerated by the active backend — real for `native` (OS voices) and
   * `elevenlabs` (the account's own voices via its `/v1/voices`), empty
   * for `http` (no standard "list voices" endpoint exists for that
   * contract, so it falls back to a free-text field below). */
  const [voices, setVoices] = useState<VoiceInfo[]>([]);
  const [previewText, setPreviewText] = useState("");

  useEffect(() => {
    setIsLoading(true);
    Promise.all([getVoiceSettings(profile.id), listTtsProviders(), listVoices()])
      .then(([voice, providers, listedVoices]) => {
        setDraft(voice);
        const current = providers.providers.find((p) => p.id === providers.currentProviderId);
        setBackend(current?.kind ?? "native");
        setVoices(listedVoices);
        setIsLoading(false);
      })
      .catch((err) => {
        pushToast({ variant: "error", title: t("voice.loadError"), description: errorMessage(err) });
        setIsLoading(false);
      });
    setPreviewText(t("voice.previewDefaultText", { name: profile.name }));
    // Re-sync only when switching to a different profile.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [profile.id]);

  const { isSaving } = useAutosave(
    draft,
    async (next) => {
      await setVoiceSettings(profile.id, next);
    },
    { enabled: !isLoading, onError: (err) => pushToast({ variant: "error", title: t("voice.saveError"), description: errorMessage(err) }) },
  );

  function update<K extends keyof ProfileVoice>(key: K, value: ProfileVoice[K]) {
    setDraft((prev) => ({ ...prev, [key]: value }));
  }

  async function handlePreview() {
    try {
      await speakVoice(profile.id, previewText.trim() || t("voice.previewDefaultText", { name: profile.name }), undefined, draft);
    } catch (err) {
      pushToast({ variant: "error", title: t("voice.previewError"), description: errorMessage(err) });
    }
  }

  if (isLoading) {
    return <LoadingState label={t("voice.loading")} />;
  }

  // None of the external backends have a pitch/volume equivalent in their
  // own API — see `HttpEngine`/`ElevenLabsEngine`/`GoogleEngine`/
  // `AzureEngine`'s `synthesize` (Rust), all of which silently ignore
  // these two fields. Only `native` maps them onto something real.
  const pitchVolumeUnsupported = backend !== "native";
  // `elevenlabs`/`google`/`azure` each have a real "list voices" endpoint
  // (`VoiceInfo`'s Rust doc comment) — only `openai`/`http` fall back to a
  // free-text voice id field.
  // Fish Audio lists a starting set (own + popular voices) but its catalogue
  // is huge, so it also keeps a free-text id field below the picker.
  const hasVoiceList = backend === "native" || backend === "elevenlabs" || backend === "google" || backend === "azure" || backend === "fishaudio" || backend === "groq" || backend === "together";

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex items-center gap-4 border-b border-border px-6 py-4">
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-lg font-semibold text-text-primary">{profile.name}</h1>
          <p className="text-sm text-text-secondary">{t("voice.subtitle")}</p>
        </div>
        {isSaving ? <p className="text-xs text-text-secondary">{t("identity.saving")}</p> : null}
      </header>

      <div className="flex-1 overflow-y-auto p-6">
        <div className="flex max-w-2xl flex-col gap-4">
          <Card className="flex flex-col gap-4 p-4">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-sm text-text-primary">{t("voice.fields.enabled")}</p>
                <p className="text-xs text-text-secondary">{t("voice.fields.enabledHint")}</p>
              </div>
              <Switch label={t("voice.fields.enabled")} checked={draft.enabled} onCheckedChange={(v) => update("enabled", v)} />
            </div>

            <div className="flex flex-col gap-4 border-t border-border pt-4">
              <Field label={t("voice.fields.voice")} hint={hasVoiceList ? undefined : t("voice.fields.voiceHintFreeText")}>
              {hasVoiceList ? (
                <Select
                  ariaLabel={t("voice.fields.voice")}
                  value={draft.voiceId}
                  onValueChange={(value) => update("voiceId", value)}
                  options={[
                    { value: "", label: t("voice.fields.voiceDefault") },
                    ...voices.map((v) => ({ value: v.id, label: v.language ? `${v.name} (${v.language})` : v.name })),
                  ]}
                />
              ) : (
                <Input
                  value={draft.voiceId}
                  onChange={(event) => update("voiceId", event.target.value)}
                  placeholder={backend === "deepgram" ? "aura-2-thalia-en" : t("voice.fields.voiceFreeTextPlaceholder")}
                />
              )}
            </Field>
            {backend === "fishaudio" ? (
              <FishVoiceSearch
                selectedId={draft.voiceId}
                onSelect={(id) => update("voiceId", id)}
              />
            ) : null}
            {backend === "fishaudio" || backend === "groq" || backend === "together" ? (
              <Field label={t("voice.fields.voiceCustomId")} hint={t("voice.fields.voiceCustomIdHint")}>
                <Input value={draft.voiceId} onChange={(event) => update("voiceId", event.target.value)} />
              </Field>
            ) : null}

            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between text-sm">
                <span className="text-text-primary">{t("voice.fields.rate")}</span>
                <span className="tabular-nums text-text-secondary">{draft.rate.toFixed(2)}×</span>
              </div>
              <Slider label={t("voice.fields.rate")} value={draft.rate} onValueChange={(v) => update("rate", v)} min={0.5} max={2} step={0.05} />
            </div>

            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between text-sm">
                <span className={pitchVolumeUnsupported ? "text-text-secondary" : "text-text-primary"}>{t("voice.fields.pitch")}</span>
                <span className="tabular-nums text-text-secondary">{draft.pitch.toFixed(2)}</span>
              </div>
              <Slider
                label={t("voice.fields.pitch")}
                value={draft.pitch}
                onValueChange={(v) => update("pitch", v)}
                min={0}
                max={2}
                step={0.05}
                disabled={pitchVolumeUnsupported}
              />
              {pitchVolumeUnsupported ? <p className="text-xs text-text-secondary">{t("voice.fields.pitchHintExternal")}</p> : null}
            </div>

            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between text-sm">
                <span className={pitchVolumeUnsupported ? "text-text-secondary" : "text-text-primary"}>{t("voice.fields.volume")}</span>
                <span className="tabular-nums text-text-secondary">{Math.round(draft.volume * 100)}%</span>
              </div>
              <Slider
                label={t("voice.fields.volume")}
                value={draft.volume}
                onValueChange={(v) => update("volume", v)}
                min={0}
                max={1}
                step={0.05}
                disabled={pitchVolumeUnsupported}
              />
              {pitchVolumeUnsupported ? <p className="text-xs text-text-secondary">{t("voice.fields.volumeHintExternal")}</p> : null}
            </div>
            </div>

            <div className="flex flex-col gap-3 border-t border-border pt-4">
              <Field label={t("voice.fields.previewText")}>
                <Input
                  value={previewText}
                  maxLength={PREVIEW_TEXT_MAX_LENGTH}
                  onChange={(event) => setPreviewText(event.target.value)}
                  placeholder={t("voice.previewDefaultText", { name: profile.name })}
                />
              </Field>
              <div className="flex justify-end">
                {isPreviewing ? (
                  <Button variant="secondary" onClick={() => void stopVoice()}>
                    <Square className="h-4 w-4" />
                    {t("voice.stopPreview")}
                  </Button>
                ) : (
                  <Button variant="secondary" onClick={() => void handlePreview()}>
                    <Volume2 className="h-4 w-4" />
                    {t("voice.testVoice")}
                  </Button>
                )}
              </div>
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}

/** Buscador por nombre en el catálogo público de Fish Audio — el selector de
 * arriba solo trae las voces propias y las más populares. Consulta con una
 * pequeña espera tras dejar de escribir para no pedir una búsqueda por tecla. */
function FishVoiceSearch({ selectedId, onSelect }: { selectedId: string; onSelect: (id: string) => void }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<VoiceInfo[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (query.trim() === "") {
      setResults([]);
      setFailed(false);
      return;
    }
    let cancelled = false;
    setIsSearching(true);
    const timer = setTimeout(() => {
      searchVoices(query)
        .then((found) => {
          if (cancelled) return;
          setResults(found);
          setFailed(false);
        })
        .catch(() => {
          if (!cancelled) setFailed(true);
        })
        .finally(() => {
          if (!cancelled) setIsSearching(false);
        });
    }, 400);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [query]);

  return (
    <Field label={t("voice.fields.voiceSearch")} hint={t("voice.fields.voiceSearchHint")}>
      <div className="flex flex-col gap-2">
        <Input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("voice.fields.voiceSearchPlaceholder")} />
        {failed ? <p className="text-xs text-error">{t("voice.fields.voiceSearchError")}</p> : null}
        {!failed && query.trim() !== "" && !isSearching && results.length === 0 ? (
          <p className="text-xs text-text-secondary">{t("voice.fields.voiceSearchEmpty")}</p>
        ) : null}
        {results.length > 0 ? (
          <div className="flex max-h-56 flex-col divide-y divide-border overflow-y-auto rounded-md border border-border">
            {results.map((voice) => (
              <button
                key={voice.id}
                type="button"
                onClick={() => onSelect(voice.id)}
                className="flex items-center justify-between gap-3 px-3 py-2 text-left text-sm text-text-primary transition-colors hover:bg-surface-secondary"
              >
                <span className="truncate">{voice.name}</span>
                <span className="flex shrink-0 items-center gap-2 text-xs text-text-secondary">
                  {voice.language ?? ""}
                  {voice.id === selectedId ? <Check className="h-3.5 w-3.5" /> : null}
                </span>
              </button>
            ))}
          </div>
        ) : null}
      </div>
    </Field>
  );
}
