import { save } from "@tauri-apps/plugin-dialog";
import { BookOpen, Camera, Copy, Download, Eye, Info, Palette, Sparkles, Trash2, Upload, type LucideIcon } from "lucide-react";
import { useEffect, useRef, useState, type ChangeEvent } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { ColorPicker } from "@/design-system/ColorPicker";
import { ConfirmDialog } from "@/design-system/Dialog";
import { Field } from "@/design-system/Field";
import { Input } from "@/design-system/Input";
import { Switch } from "@/design-system/Switch";
import { Textarea } from "@/design-system/Textarea";
import { errorMessage } from "@/lib/errorMessage";
import { AVATAR_MAX_DIMENSION, readFileAsBase64, resizeImageToFit } from "@/lib/files";
import { invalidateCachedImages } from "@/lib/imageCache";
import { exportProfileToPath, previewProfilePrompt, setProfileAvatar } from "@/lib/ipc/profiles";
import { DEFAULT_ACCENT_HEX } from "@/lib/theme/accentTheme";
import { cn } from "@/lib/theme/cn";
import type { ThemePresetId } from "@/lib/theme/presets";
import { useAutosave } from "@/lib/useAutosave";
import { useProfileAvatar } from "@/lib/useProfileAvatar";
import { useMoodStore } from "@/stores/moodStore";
import { useProactiveStore } from "@/stores/proactiveStore";
import { useProfileStore } from "@/stores/profileStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useToastStore } from "@/stores/toastStore";
import { profileToUpdateInput, type Profile, type ProfileUpdateInput } from "@/types/profile";

import { MoodBadge } from "./MoodBadge";
import { useImportProfile } from "./useImportProfile";
import { PersonalitySliders } from "./PersonalitySliders";
import { ThemePresetPicker } from "./ThemePresetPicker";

type IdentitySection = "info" | "lore" | "personality" | "appearance" | "preview";

const IDENTITY_SECTIONS: IdentitySection[] = ["info", "lore", "personality", "appearance", "preview"];

const SECTION_ICONS: Record<IdentitySection, LucideIcon> = {
  info: Info,
  lore: BookOpen,
  personality: Sparkles,
  appearance: Palette,
  preview: Eye,
};


export function IdentityEditor({ profile }: { profile: Profile }) {
  const { t } = useTranslation();
  const updateProfile = useProfileStore((s) => s.update);
  const duplicateProfile = useProfileStore((s) => s.duplicate);
  const removeProfile = useProfileStore((s) => s.remove);
  const importProfile = useImportProfile();
  const resetMood = useMoodStore((s) => s.reset);
  const proactiveSettings = useProactiveStore((s) => s.settingsByProfile[profile.id]);
  const loadProactiveSettings = useProactiveStore((s) => s.load);
  const setProactiveEnabled = useProactiveStore((s) => s.setEnabled);
  const pushToast = useToastStore((s) => s.push);
  const language = useSettingsStore((s) => s.language);

  const [activeSection, setActiveSection] = useState<IdentitySection>("info");
  const [draft, setDraft] = useState<ProfileUpdateInput>(() => profileToUpdateInput(profile));
  const [promptPreview, setPromptPreview] = useState("");
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  /** Tildado por defecto — pedido del usuario tras perder su historial en
   * un export/import de prueba: mejor que quien exporta tenga que
   * destildarlo a propósito (ej. para compartir el personaje sin sus
   * chats) que perderlo sin darse cuenta. */
  const [exportIncludeChats, setExportIncludeChats] = useState(true);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const avatarDataUrl = useProfileAvatar(profile.id, profile.avatarPath);

  const { isSaving } = useAutosave(
    draft,
    async (next) => {
      await updateProfile(profile.id, next);
    },
    { onError: (err) => pushToast({ variant: "error", title: t("identity.saveError"), description: errorMessage(err) }) },
  );

  useEffect(() => {
    setDraft(profileToUpdateInput(profile));
    // Only re-sync the draft when switching to a *different* profile —
    // not on every store refresh of the same one, which would clobber
    // in-progress edits. `IdentityEditor` is keyed by `profile.id` (see
    // `IdentityPage.tsx`), so in practice this never actually re-fires —
    // a different profile fully remounts this component instead.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [profile.id]);

  useEffect(() => {
    const timeout = setTimeout(() => {
      previewProfilePrompt(draft)
        .then(setPromptPreview)
        .catch(() => setPromptPreview(""));
    }, 300);
    return () => clearTimeout(timeout);
    // `language` isn't part of `draft` (it's a global app setting, not a
    // profile field) but the backend composes the preview in whatever
    // language is active right now — switching it in Configuración while
    // this tab is open needs to re-fetch, or the preview keeps showing
    // whatever language was active on the last `draft` edit.
  }, [draft, language]);

  useEffect(() => {
    void loadProactiveSettings(profile.id);
  }, [profile.id, loadProactiveSettings]);

  function update<K extends keyof ProfileUpdateInput>(key: K, value: ProfileUpdateInput[K]) {
    setDraft((prev) => ({ ...prev, [key]: value }));
  }

  /** Tema visual + acento personalizado (ADR-009/008) — a diferencia del
   * resto de los campos de este editor, guardan de inmediato en vez de
   * esperar el debounce de `useAutosave`: `useActiveProfileTheme` lee el
   * tema/acento directo de `profileStore`, así que arrastrar el selector de
   * color (que dispara `onChange` muy seguido) tiene que verse al instante,
   * no con el delay de cada round-trip. `setDraft` también se actualiza acá
   * para que un autoguardado posterior de otro campo (nombre, lore, etc.)
   * no mande un tema/acento desactualizado. */
  async function persistTheme(patch: Partial<Pick<ProfileUpdateInput, "theme" | "accentColor">>) {
    setDraft((prev) => ({ ...prev, ...patch }));
    useProfileStore.setState((state) => ({
      profiles: state.profiles.map((p) => (p.id === profile.id ? { ...p, ...patch } : p)),
    }));
    try {
      await updateProfile(profile.id, { ...draft, ...patch });
    } catch (err) {
      pushToast({ variant: "error", title: t("identity.saveError"), description: errorMessage(err) });
    }
  }

  async function handleAvatarChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    try {
      const { base64, extension } = await readFileAsBase64(await resizeImageToFit(file, AVATAR_MAX_DIMENSION));
      await setProfileAvatar(profile.id, base64, extension);
      invalidateCachedImages(`profile:${profile.id}:`);
      await useProfileStore.getState().load();
      pushToast({ variant: "success", title: t("identity.avatarUpdated") });
    } catch (err) {
      pushToast({ variant: "error", title: t("identity.avatarError"), description: errorMessage(err) });
    }
  }

  async function handleDuplicate() {
    try {
      await duplicateProfile(profile.id, t("identity.duplicateSuffix", { name: profile.name }));
      pushToast({ variant: "success", title: t("identity.duplicated") });
    } catch (err) {
      pushToast({ variant: "error", title: t("identity.duplicateError"), description: errorMessage(err) });
    }
  }

  async function handleConfirmExport() {
    try {
      const path = await save({
        defaultPath: `${profile.name}.aivtprofile`,
        filters: [{ name: "Project Ciel Profile", extensions: ["aivtprofile"] }],
      });
      if (!path) return;
      await exportProfileToPath(profile.id, path, exportIncludeChats);
      pushToast({ variant: "success", title: t("identity.exported") });
    } catch (err) {
      pushToast({ variant: "error", title: t("identity.exportError"), description: errorMessage(err) });
    }
  }

  async function handleDelete() {
    try {
      await removeProfile(profile.id);
      pushToast({ variant: "success", title: t("identity.deleted") });
    } catch (err) {
      pushToast({ variant: "error", title: t("identity.deleteError"), description: errorMessage(err) });
    }
  }

  async function handleResetMood() {
    try {
      await resetMood(profile.id);
      pushToast({ variant: "success", title: t("mood.resetSuccess") });
    } catch (err) {
      pushToast({ variant: "error", title: t("mood.resetError"), description: errorMessage(err) });
    }
  }

  async function handleToggleProactive(enabled: boolean) {
    try {
      await setProactiveEnabled(profile.id, enabled);
    } catch (err) {
      pushToast({ variant: "error", title: t("proactive.toggleError"), description: errorMessage(err) });
    }
  }

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex items-center gap-4 border-b border-border px-6 py-4">
        <button
          type="button"
          onClick={() => fileInputRef.current?.click()}
          className="group relative flex h-16 w-16 shrink-0 items-center justify-center overflow-hidden rounded-full bg-surface-secondary text-text-secondary"
          aria-label={t("identity.changeAvatar")}
        >
          {avatarDataUrl ? (
            <img src={avatarDataUrl} alt="" className="h-full w-full object-cover" />
          ) : (
            <span className="text-lg font-semibold">{profile.name.slice(0, 1).toUpperCase()}</span>
          )}
          <span className="absolute inset-0 flex items-center justify-center bg-black/50 opacity-0 transition-opacity group-hover:opacity-100">
            <Camera className="h-5 w-5 text-white" />
          </span>
        </button>
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          className="hidden"
          onChange={(event) => void handleAvatarChange(event)}
        />

        <div className="min-w-0 flex-1">
          <h1 className="truncate text-lg font-semibold text-text-primary">{draft.name || t("identity.untitled")}</h1>
          <p className="text-sm text-text-secondary">{t("identity.subtitle")}</p>
        </div>

        <div className="flex shrink-0 items-center gap-2">
          {isSaving ? <p className="text-xs text-text-secondary">{t("identity.saving")}</p> : null}
          <Button variant="ghost" size="sm" onClick={() => void handleDuplicate()}>
            <Copy className="h-4 w-4" />
            {t("identity.actions.duplicate")}
          </Button>
          <Button variant="ghost" size="sm" onClick={() => void importProfile()}>
            <Upload className="h-4 w-4" />
            {t("identity.actions.import")}
          </Button>
          <Button variant="ghost" size="sm" onClick={() => setExportOpen(true)}>
            <Download className="h-4 w-4" />
            {t("identity.actions.export")}
          </Button>
          <Button variant="ghost" size="sm" onClick={() => setDeleteOpen(true)}>
            <Trash2 className="h-4 w-4" />
            {t("identity.actions.delete")}
          </Button>
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        <div className="flex w-56 shrink-0 flex-col overflow-y-auto border-r border-border py-2">
          {IDENTITY_SECTIONS.map((section) => {
            const Icon = SECTION_ICONS[section];
            return (
              <button
                key={section}
                type="button"
                onClick={() => setActiveSection(section)}
                className={cn(
                  "mx-2 flex items-center gap-2.5 rounded-md px-2.5 py-2 text-left text-sm transition-colors",
                  section === activeSection
                    ? "bg-surface-secondary font-medium text-text-primary"
                    : "text-text-secondary hover:bg-surface-secondary hover:text-text-primary",
                )}
              >
                <Icon className="h-4 w-4 shrink-0" />
                <span className="truncate">{t(`identity.tabs.${section}`)}</span>
              </button>
            );
          })}
        </div>

        <div className="flex-1 overflow-y-auto p-6">
          {activeSection === "info" ? (
            <Card className="grid max-w-2xl grid-cols-1 gap-4 p-4 sm:grid-cols-2">
              <Field label={t("identity.fields.name")}>
                <Input value={draft.name} onChange={(event) => update("name", event.target.value)} />
              </Field>
              <Field label={t("identity.fields.nickname")}>
                <Input value={draft.nickname ?? ""} onChange={(event) => update("nickname", event.target.value || null)} />
              </Field>
              <Field label={t("identity.fields.pronouns")}>
                <Input value={draft.pronouns ?? ""} onChange={(event) => update("pronouns", event.target.value || null)} />
              </Field>
              <Field label={t("identity.fields.ageFictional")}>
                <Input value={draft.ageFictional ?? ""} onChange={(event) => update("ageFictional", event.target.value || null)} />
              </Field>
              <Field label={t("identity.fields.occupation")}>
                <Input value={draft.occupation ?? ""} onChange={(event) => update("occupation", event.target.value || null)} />
              </Field>
              <Field label={t("identity.fields.species")}>
                <Input value={draft.species ?? ""} onChange={(event) => update("species", event.target.value || null)} />
              </Field>
              <Field label={t("identity.fields.description")} className="sm:col-span-2">
                <Textarea autoGrow rows={3} value={draft.description ?? ""} onChange={(event) => update("description", event.target.value || null)} />
              </Field>
            </Card>
          ) : null}

          {activeSection === "lore" ? (
            <Card className="flex max-w-2xl flex-col gap-4 p-4">
              <Field label={t("identity.fields.lore")}>
                <Textarea
                  autoGrow
                  rows={8}
                  value={draft.identity.lore}
                  onChange={(event) => update("identity", { ...draft.identity, lore: event.target.value })}
                />
              </Field>
              <Field label={t("identity.fields.relationshipToUser")} hint={t("identity.fields.relationshipToUserHint")}>
                <Textarea
                  autoGrow
                  rows={3}
                  value={draft.identity.relationshipToUser}
                  onChange={(event) => update("identity", { ...draft.identity, relationshipToUser: event.target.value })}
                />
              </Field>
              <Field label={t("identity.fields.userRole")} hint={t("identity.fields.userRoleHint")}>
                <Textarea
                  autoGrow
                  rows={3}
                  value={draft.identity.userRole}
                  onChange={(event) => update("identity", { ...draft.identity, userRole: event.target.value })}
                />
              </Field>
              <Field label={t("identity.fields.speakingStyle")}>
                <Textarea
                  autoGrow
                  rows={2}
                  value={draft.identity.speakingStyle}
                  onChange={(event) => update("identity", { ...draft.identity, speakingStyle: event.target.value })}
                />
              </Field>
              <Field label={t("identity.fields.specialInstructions")}>
                <Textarea
                  autoGrow
                  rows={2}
                  value={draft.identity.specialInstructions}
                  onChange={(event) => update("identity", { ...draft.identity, specialInstructions: event.target.value })}
                />
              </Field>
            </Card>
          ) : null}

          {activeSection === "personality" ? (
            <Card className="flex max-w-3xl flex-col gap-4 p-4">
              <PersonalitySliders value={draft.personality} onChange={(next) => update("personality", next)} />

              <div className="flex items-center justify-between gap-4 border-t border-border pt-4">
                <div>
                  <p className="text-sm font-medium text-text-primary">{t("proactive.label")}</p>
                  <p className="mt-1 text-xs text-text-secondary">{t("proactive.hint")}</p>
                </div>
                <Switch
                  label={t("proactive.label")}
                  checked={proactiveSettings?.enabled ?? false}
                  onCheckedChange={(value) => void handleToggleProactive(value)}
                />
              </div>

              <div className="flex items-center justify-between gap-4 border-t border-border pt-4">
                <div>
                  <div className="flex items-center gap-2">
                    <p className="text-sm font-medium text-text-primary">{t("mood.label")}</p>
                    <MoodBadge profileId={profile.id} />
                  </div>
                  <p className="mt-1 text-xs text-text-secondary">{t("mood.hint")}</p>
                </div>
                <Button variant="ghost" size="sm" onClick={() => void handleResetMood()}>
                  {t("mood.resetButton")}
                </Button>
              </div>
            </Card>
          ) : null}

          {activeSection === "appearance" ? (
            <Card className="flex max-w-2xl flex-col gap-4 p-4">
              <div className="flex flex-col gap-3">
                <div>
                  <p className="text-sm font-medium text-text-primary">{t("settings.appearance.themeTitle")}</p>
                  <p className="text-xs text-text-secondary">{t("settings.appearance.themeDescription")}</p>
                </div>
                <ThemePresetPicker theme={draft.theme} onThemeChange={(id: ThemePresetId | null) => void persistTheme({ theme: id })} />
              </div>

              <div className="flex flex-col gap-3 border-t border-border pt-4">
                <div className="flex items-center justify-between gap-4">
                  <div>
                    <p className="text-sm font-medium text-text-primary">{t("settings.appearance.customAccent")}</p>
                    <p className="text-xs text-text-secondary">{t("settings.appearance.customAccentDescription")}</p>
                  </div>
                  <Switch
                    label={t("settings.appearance.customAccent")}
                    checked={draft.accentColor !== null}
                    onCheckedChange={(checked) => void persistTheme({ accentColor: checked ? (draft.accentColor ?? DEFAULT_ACCENT_HEX) : null })}
                  />
                </div>
                {draft.accentColor !== null ? (
                  <div className="flex items-center justify-between gap-4">
                    <p className="text-sm text-text-primary">{t("identity.fields.accentColor")}</p>
                    <ColorPicker value={draft.accentColor} onChange={(hex) => void persistTheme({ accentColor: hex })} />
                  </div>
                ) : null}
              </div>
            </Card>
          ) : null}

          {activeSection === "preview" ? (
            <Card className="max-w-3xl whitespace-pre-wrap p-4 font-mono text-xs leading-relaxed text-text-primary">
              {promptPreview || t("identity.previewEmpty")}
            </Card>
          ) : null}
        </div>
      </div>

      <ConfirmDialog
        open={deleteOpen}
        onOpenChange={setDeleteOpen}
        title={t("identity.deleteConfirmTitle", { name: profile.name })}
        description={t("identity.deleteConfirmDescription")}
        confirmLabel={t("identity.actions.delete")}
        cancelLabel={t("common.cancel")}
        danger
        onConfirm={() => void handleDelete()}
      />

      <ConfirmDialog
        open={exportOpen}
        onOpenChange={setExportOpen}
        title={t("identity.exportConfirmTitle", { name: profile.name })}
        description={t("identity.exportConfirmDescription")}
        confirmLabel={t("identity.actions.export")}
        cancelLabel={t("common.cancel")}
        onConfirm={() => void handleConfirmExport()}
      >
        <div className="flex items-center gap-2">
          <span className="text-sm text-text-primary">{t("identity.exportIncludeChatsLabel")}</span>
          <Switch label={t("identity.exportIncludeChatsLabel")} checked={exportIncludeChats} onCheckedChange={setExportIncludeChats} />
        </div>
      </ConfirmDialog>
    </div>
  );
}
