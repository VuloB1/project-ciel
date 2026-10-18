import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { Camera } from "lucide-react";
import { useEffect, useRef, useState, type ChangeEvent } from "react";
import { useTranslation } from "react-i18next";

import { Card } from "@/design-system/Card";
import { Input } from "@/design-system/Input";
import { Select } from "@/design-system/Select";
import { Switch } from "@/design-system/Switch";
import { UserAvatarBadge } from "@/design-system/UserAvatarBadge";
import { errorMessage } from "@/lib/errorMessage";
import { AVATAR_MAX_DIMENSION, readFileAsBase64, resizeImageToFit } from "@/lib/files";
import { LANGUAGE_LABELS, SUPPORTED_LANGUAGES } from "@/lib/i18n";
import { getUserName, setUserAvatar, setUserName } from "@/lib/ipc/settings";
import { useSettingsStore } from "@/stores/settingsStore";
import { useToastStore } from "@/stores/toastStore";

export function GeneralSettings() {
  const { t } = useTranslation();
  const language = useSettingsStore((s) => s.language);
  const setLanguage = useSettingsStore((s) => s.setLanguage);
  const background = useSettingsStore((s) => s.background);
  const setBackground = useSettingsStore((s) => s.setBackground);
  const bumpUserAvatarVersion = useSettingsStore((s) => s.bumpUserAvatarVersion);
  const pushToast = useToastStore((s) => s.push);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // `null` = still loading the real OS state; the autostart plugin (not
  // `app_settings`) is the single source of truth here — see
  // aivt-service-settings::SettingsService doc comment.
  const [autostart, setAutostart] = useState<boolean | null>(null);

  useEffect(() => {
    isEnabled()
      .then(setAutostart)
      .catch(() => setAutostart(null));
  }, []);

  // The name characters call the user by; saved when the field loses focus or Enter is pressed.
  const [userName, setUserNameDraft] = useState("");
  const savedUserName = useRef("");
  useEffect(() => {
    getUserName()
      .then((name) => {
        savedUserName.current = name ?? "";
        setUserNameDraft(name ?? "");
      })
      .catch(() => undefined);
  }, []);

  async function saveUserName() {
    if (userName.trim() === savedUserName.current) return;
    try {
      const stored = (await setUserName(userName)) ?? "";
      savedUserName.current = stored;
      setUserNameDraft(stored);
      pushToast({ variant: "success", title: t("settings.general.userNameSaved") });
    } catch {
      pushToast({ variant: "error", title: t("settings.general.saveError") });
    }
  }

  async function handleAvatarChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    try {
      const { base64, extension } = await readFileAsBase64(await resizeImageToFit(file, AVATAR_MAX_DIMENSION));
      await setUserAvatar(base64, extension);
      bumpUserAvatarVersion();
      pushToast({ variant: "success", title: t("settings.general.avatarUpdated") });
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.general.avatarError"), description: errorMessage(err) });
    }
  }

  async function handleLanguageChange(value: string) {
    try {
      await setLanguage(value);
      pushToast({ variant: "success", title: t("settings.general.languageSaved") });
    } catch {
      pushToast({ variant: "error", title: t("settings.general.saveError") });
    }
  }

  async function handleAutostartChange(checked: boolean) {
    const previous = autostart;
    setAutostart(checked);
    try {
      if (checked) {
        await enable();
      } else {
        await disable();
      }
    } catch {
      setAutostart(previous);
      pushToast({ variant: "error", title: t("settings.general.saveError") });
    }
  }

  async function handleBackgroundChange(next: typeof background) {
    if (!next) return;
    try {
      await setBackground(next);
    } catch {
      pushToast({ variant: "error", title: t("settings.general.saveError") });
    }
  }

  return (
    <div className="flex max-w-xl flex-col gap-4">
      <Card className="flex flex-col gap-4 p-4">
        <div className="flex items-center gap-4">
          <button
            type="button"
            onClick={() => fileInputRef.current?.click()}
            className="group relative flex h-16 w-16 shrink-0 items-center justify-center overflow-hidden rounded-full"
            aria-label={t("settings.general.changeAvatar")}
          >
            <UserAvatarBadge size="lg" />
            <span className="absolute inset-0 flex items-center justify-center bg-black/50 opacity-0 transition-opacity group-hover:opacity-100">
              <Camera className="h-5 w-5 text-white" />
            </span>
          </button>
          <input ref={fileInputRef} type="file" accept="image/*" className="hidden" onChange={(event) => void handleAvatarChange(event)} />
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.general.avatar")}</p>
            <p className="text-xs text-text-secondary">{t("settings.general.avatarDescription")}</p>
          </div>
        </div>

        <div className="flex items-center justify-between gap-4 border-t border-border pt-4">
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.general.userName")}</p>
            <p className="text-xs text-text-secondary">{t("settings.general.userNameDescription")}</p>
          </div>
          <Input
            aria-label={t("settings.general.userName")}
            className="w-48"
            maxLength={40}
            placeholder={t("settings.general.userNamePlaceholder")}
            value={userName}
            onChange={(event) => setUserNameDraft(event.target.value)}
            onBlur={() => void saveUserName()}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
            }}
          />
        </div>

        <div className="flex items-center justify-between gap-4 border-t border-border pt-4">
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.general.language")}</p>
            <p className="text-xs text-text-secondary">{t("settings.general.languageDescription")}</p>
          </div>
          <Select
            ariaLabel={t("settings.general.language")}
            value={language}
            onValueChange={(value) => void handleLanguageChange(value)}
            options={SUPPORTED_LANGUAGES.map((lang) => ({ value: lang, label: LANGUAGE_LABELS[lang] ?? lang }))}
          />
        </div>

        <div className="flex items-center justify-between gap-4 border-t border-border pt-4">
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.general.startWithSystem")}</p>
            <p className="text-xs text-text-secondary">{t("settings.general.startWithSystemDescription")}</p>
          </div>
          <Switch
            label={t("settings.general.startWithSystem")}
            checked={autostart ?? false}
            disabled={autostart === null}
            onCheckedChange={(checked) => void handleAutostartChange(checked)}
          />
        </div>

        <div className="flex items-center justify-between gap-4 border-t border-border pt-4">
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.general.minimizeToTray")}</p>
            <p className="text-xs text-text-secondary">{t("settings.general.minimizeToTrayHint")}</p>
          </div>
          <Switch
            label={t("settings.general.minimizeToTray")}
            checked={background?.minimizeToTray ?? false}
            disabled={background === null}
            onCheckedChange={(checked) => void handleBackgroundChange(background && { ...background, minimizeToTray: checked })}
          />
        </div>

        <div className="flex items-center justify-between gap-4 border-t border-border pt-4">
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.general.notifyProactive")}</p>
            <p className="text-xs text-text-secondary">{t("settings.general.notifyProactiveHint")}</p>
          </div>
          <Switch
            label={t("settings.general.notifyProactive")}
            checked={background?.notifyProactive ?? false}
            disabled={background === null}
            onCheckedChange={(checked) => void handleBackgroundChange(background && { ...background, notifyProactive: checked })}
          />
        </div>

        <div className="flex items-center justify-between gap-4 border-t border-border pt-4">
          <div>
            <p className="text-sm font-medium text-text-primary">{t("settings.general.voiceWhileHidden")}</p>
            <p className="text-xs text-text-secondary">{t("settings.general.voiceWhileHiddenHint")}</p>
          </div>
          <Switch
            label={t("settings.general.voiceWhileHidden")}
            checked={background?.voiceWhileHidden ?? false}
            disabled={background === null}
            onCheckedChange={(checked) => void handleBackgroundChange(background && { ...background, voiceWhileHidden: checked })}
          />
        </div>
      </Card>
    </div>
  );
}
