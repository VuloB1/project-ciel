import { Camera, Check } from "lucide-react";
import { useEffect, useRef, useState, type ChangeEvent } from "react";
import { useTranslation } from "react-i18next";

import { Card } from "@/design-system/Card";
import { ColorPicker } from "@/design-system/ColorPicker";
import { Field } from "@/design-system/Field";
import { Input } from "@/design-system/Input";
import { LoadingState } from "@/design-system/LoadingState";
import { Switch } from "@/design-system/Switch";
import { Textarea } from "@/design-system/Textarea";
import { ThemePresetPicker } from "@/features/identity/ThemePresetPicker";
import { errorMessage } from "@/lib/errorMessage";
import { AVATAR_MAX_DIMENSION, readFileAsBase64, resizeImageToFit } from "@/lib/files";
import { setProfileAvatar } from "@/lib/ipc/profiles";
import { DEFAULT_ACCENT_HEX } from "@/lib/theme/accentTheme";
import type { ThemePresetId } from "@/lib/theme/presets";
import { useProfileStore } from "@/stores/profileStore";
import { useToastStore } from "@/stores/toastStore";
import { profileToUpdateInput } from "@/types/profile";

interface CreateCharacterStepProps {
  /** Ver el doc comment de `StepHelpers` en `WelcomeScreen.tsx` — este paso
   * no tiene su propio botón "Crear" (lo tenía, pero quedaba pisado por el
   * "Continuar" del pie, que el usuario terminaba usando de todos modos y
   * el personaje nunca se guardaba). En su lugar, se engancha al botón
   * "Continuar" mientras está montado. */
  registerSubmit: (fn: (() => Promise<void>) | null) => void;
}

interface PendingAvatar {
  base64: string;
  extension: string;
  previewUrl: string;
}

/** Si ya hay un perfil (creado antes de llegar acá, o importado) no repite
 * el formulario de `IdentityPage.tsx`'s `EmptyState` — solo confirma que ya
 * existe y deja avanzar.
 *
 * Pedido explícito del usuario: además del nombre, dejar personalizar
 * imagen/descripción/tema/acento ACÁ MISMO, no solo el nombre — así el
 * personaje queda del todo personalizado desde el arranque en vez de tener
 * que volver después a Identidad/Apariencia. Como `setProfileAvatar`/
 * `profiles_update` necesitan un `id` de perfil ya existente (no hay forma
 * de mandar esto junto con la creación), todo se junta acá en estado local
 * y recién se manda en un solo golpe cuando se confirma: primero `create`
 * (da el `id`), después el avatar si eligió uno, después descripción/tema/
 * acento si tocó alguno — sin tocar el backend. */
export function CreateCharacterStep({ registerSubmit }: CreateCharacterStepProps) {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const isLoading = useProfileStore((s) => s.isLoading);
  const profiles = useProfileStore((s) => s.profiles);
  const createProfile = useProfileStore((s) => s.create);
  const updateProfile = useProfileStore((s) => s.update);

  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [theme, setTheme] = useState<ThemePresetId | null>(null);
  const [accentColor, setAccentColor] = useState<string | null>(null);
  const [avatar, setAvatar] = useState<PendingAvatar | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const firstProfile = profiles[0];

  async function handleCreate() {
    try {
      const profile = await createProfile(name.trim() || t("identity.defaultName"));
      if (avatar) {
        await setProfileAvatar(profile.id, avatar.base64, avatar.extension);
      }
      if (description.trim() || theme || accentColor) {
        await updateProfile(profile.id, {
          ...profileToUpdateInput(profile),
          description: description.trim() || null,
          theme,
          accentColor,
        });
      }
    } catch (err) {
      pushToast({ variant: "error", title: t("identity.saveError"), description: errorMessage(err) });
      // Re-lanzado a propósito: `WelcomeScreen`'s "Continuar" lo usa para
      // saber que NO tiene que avanzar de paso si esto falló.
      throw err;
    }
  }

  // Mientras no exista un perfil todavía, "Continuar" (en el pie del
  // wizard) queda enganchado a `handleCreate` — se desengancha solo al
  // desmontar (cambiar de paso) o en cuanto ya hay un perfil.
  useEffect(() => {
    if (firstProfile) {
      registerSubmit(null);
      return;
    }
    registerSubmit(handleCreate);
    return () => registerSubmit(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [firstProfile, name, description, theme, accentColor, avatar]);

  async function handlePickAvatar(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    const { base64, extension } = await readFileAsBase64(await resizeImageToFit(file, AVATAR_MAX_DIMENSION));
    setAvatar((prev) => {
      if (prev) URL.revokeObjectURL(prev.previewUrl);
      return { base64, extension, previewUrl: URL.createObjectURL(file) };
    });
  }

  if (isLoading) {
    return <LoadingState label={t("identity.loading")} />;
  }

  if (firstProfile) {
    return (
      <Card className="flex items-center gap-3 p-4">
        <Check className="h-5 w-5 shrink-0 text-accent" />
        <p className="text-sm text-text-primary">{t("onboarding.character.alreadyHave", { name: firstProfile.name })}</p>
      </Card>
    );
  }

  return (
    <Card className="flex flex-col gap-4 p-4">
      <div className="flex items-center gap-4">
        <button
          type="button"
          onClick={() => fileInputRef.current?.click()}
          className="group relative flex h-16 w-16 shrink-0 items-center justify-center overflow-hidden rounded-full bg-surface-secondary text-text-secondary"
          aria-label={t("identity.changeAvatar")}
        >
          {avatar ? (
            <img src={avatar.previewUrl} alt="" className="h-full w-full object-cover" />
          ) : (
            <span className="text-lg font-semibold">{(name || t("identity.defaultName")).slice(0, 1).toUpperCase()}</span>
          )}
          <span className="absolute inset-0 flex items-center justify-center bg-black/50 opacity-0 transition-opacity group-hover:opacity-100">
            <Camera className="h-5 w-5 text-white" />
          </span>
        </button>
        <input ref={fileInputRef} type="file" accept="image/*" className="hidden" onChange={(event) => void handlePickAvatar(event)} />
        <Field label={t("onboarding.character.nameLabel")} className="flex-1">
          <Input value={name} onChange={(event) => setName(event.target.value)} placeholder={t("identity.defaultName")} />
        </Field>
      </div>

      <Field label={t("identity.fields.description")}>
        <Textarea autoGrow rows={2} value={description} onChange={(event) => setDescription(event.target.value)} />
      </Field>

      <div className="flex flex-col gap-3 border-t border-border pt-4">
        <div>
          <p className="text-sm font-medium text-text-primary">{t("settings.appearance.themeTitle")}</p>
          <p className="text-xs text-text-secondary">{t("settings.appearance.themeDescription")}</p>
        </div>
        <ThemePresetPicker theme={theme} onThemeChange={setTheme} />
      </div>

      <div className="flex flex-col gap-3 border-t border-border pt-4">
        <div className="flex items-center justify-between gap-4">
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.appearance.customAccent")}</p>
            <p className="text-xs text-text-secondary">{t("settings.appearance.customAccentDescription")}</p>
          </div>
          <Switch
            label={t("settings.appearance.customAccent")}
            checked={accentColor !== null}
            onCheckedChange={(checked) => setAccentColor(checked ? (accentColor ?? DEFAULT_ACCENT_HEX) : null)}
          />
        </div>
        {accentColor !== null ? (
          <div className="flex items-center justify-between gap-4">
            <p className="text-sm text-text-primary">{t("identity.fields.accentColor")}</p>
            <ColorPicker value={accentColor} onChange={setAccentColor} />
          </div>
        ) : null}
      </div>
    </Card>
  );
}
