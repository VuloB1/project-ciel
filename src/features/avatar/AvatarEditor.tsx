import { Camera, Copy } from "lucide-react";
import { useEffect, useRef, useState, type ChangeEvent } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { Input } from "@/design-system/Input";
import { Switch } from "@/design-system/Switch";
import { errorMessage } from "@/lib/errorMessage";
import { AVATAR_STATE_MAX_DIMENSION, readFileAsBase64, resizeImageToFit } from "@/lib/files";
import { invalidateCachedImages } from "@/lib/imageCache";
import {
  getAvatarServerStatus,
  setAvatarServerEnabled,
  setAvatarStateImage,
  type AvatarServerStatus,
  type AvatarVisualState,
} from "@/lib/ipc/avatar";
import { useAvatarStateImages } from "@/lib/useAvatarStateImage";
import { useAvatarStore } from "@/stores/avatarStore";
import { useProfileStore } from "@/stores/profileStore";
import { useToastStore } from "@/stores/toastStore";
import type { Profile } from "@/types/profile";

const STATES: AvatarVisualState[] = ["idle", "thinking", "talking"];

function StateUploadSlot({ profile, state, imageUrl }: { profile: Profile; state: AvatarVisualState; imageUrl: string | null }) {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const refreshProfiles = useProfileStore((s) => s.load);
  const fileInputRef = useRef<HTMLInputElement>(null);

  async function handleChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    try {
      const { base64, extension } = await readFileAsBase64(await resizeImageToFit(file, AVATAR_STATE_MAX_DIMENSION));
      await setAvatarStateImage(profile.id, state, base64, extension);
      invalidateCachedImages(`profile:${profile.id}:${state}:`);
      await refreshProfiles();
      pushToast({ variant: "success", title: t("avatar.stateImageUpdated") });
    } catch (err) {
      pushToast({ variant: "error", title: t("avatar.stateImageError"), description: errorMessage(err) });
    }
  }

  return (
    <div className="flex flex-col items-center gap-2">
      <button
        type="button"
        onClick={() => fileInputRef.current?.click()}
        className="group relative flex h-28 w-28 items-center justify-center overflow-hidden rounded-lg border border-border bg-surface-secondary text-text-secondary"
        aria-label={t(`avatar.states.${state}`)}
      >
        {imageUrl ? (
          <img src={imageUrl} alt="" className="h-full w-full object-contain" />
        ) : (
          <span className="text-xs">{t("avatar.noImage")}</span>
        )}
        <span className="absolute inset-0 flex items-center justify-center bg-black/50 opacity-0 transition-opacity group-hover:opacity-100">
          <Camera className="h-5 w-5 text-white" />
        </span>
      </button>
      <input ref={fileInputRef} type="file" accept="image/*" className="hidden" onChange={(event) => void handleChange(event)} />
      <p className="text-xs text-text-secondary">{t(`avatar.states.${state}`)}</p>
    </div>
  );
}

export function AvatarEditor({ profile }: { profile: Profile }) {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const liveState = useAvatarStore((s) => s.visualState);
  const liveTrackedId = useAvatarStore((s) => s.trackedProfileId);
  const isLive = liveTrackedId === profile.id;

  const images = useAvatarStateImages(profile);
  const [serverStatus, setServerStatus] = useState<AvatarServerStatus | null>(null);

  useEffect(() => {
    let cancelled = false;
    getAvatarServerStatus()
      .then((status) => {
        if (!cancelled) setServerStatus(status);
      })
      .catch(() => {
        if (!cancelled) setServerStatus(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleToggleServer(enabled: boolean) {
    try {
      setServerStatus(await setAvatarServerEnabled(enabled));
    } catch (err) {
      pushToast({ variant: "error", title: t("avatar.obsServerError"), description: errorMessage(err) });
    }
  }

  async function handleCopyUrl(url: string) {
    try {
      await navigator.clipboard.writeText(url);
      pushToast({ variant: "success", title: t("avatar.obsCopied") });
    } catch (err) {
      pushToast({ variant: "error", title: t("avatar.obsServerError"), description: errorMessage(err) });
    }
  }

  const obsUrl = serverStatus?.enabled ? `http://127.0.0.1:${serverStatus.port}/overlay?profileId=${profile.id}` : null;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex items-center justify-between gap-4 border-b border-border px-6 py-4">
        <div className="min-w-0">
          <h1 className="truncate text-lg font-semibold text-text-primary">{profile.name}</h1>
          <p className="text-sm text-text-secondary">{t("avatar.subtitle")}</p>
        </div>
        <div className="flex shrink-0 flex-col items-end gap-1.5">
          <div className="flex items-center gap-2">
            <span className="text-xs font-medium text-text-secondary" title={t("avatar.obsHint")}>
              {t("avatar.obsSectionTitle")}
            </span>
            <Switch
              label={t("avatar.obsEnable")}
              checked={serverStatus?.enabled ?? false}
              onCheckedChange={(checked) => void handleToggleServer(checked)}
            />
          </div>
          {obsUrl ? (
            <div className="flex items-center gap-1.5">
              <Input
                readOnly
                value={obsUrl}
                onFocus={(event) => event.currentTarget.select()}
                className="h-8 w-56 font-mono text-[10px]"
              />
              <Button size="sm" variant="secondary" aria-label={t("avatar.obsCopyUrl")} title={t("avatar.obsCopyUrl")} onClick={() => void handleCopyUrl(obsUrl)}>
                <Copy className="h-3.5 w-3.5" />
              </Button>
            </div>
          ) : null}
        </div>
      </header>

      <div className="flex-1 overflow-y-auto p-6">
        <div className="flex max-w-2xl flex-col gap-4">
          <Card className="flex flex-col gap-4 p-4">
            <div className="flex flex-col gap-3">
              <div className="flex items-center justify-between">
                <p className="text-sm font-medium text-text-primary">{t("avatar.statesTitle")}</p>
                {isLive ? (
                  <span className="rounded-full bg-accent/10 px-2.5 py-0.5 text-xs font-medium text-accent">
                    {t("avatar.live", { state: t(`avatar.states.${liveState}`) })}
                  </span>
                ) : null}
              </div>
              <p className="text-xs text-text-secondary">{t("avatar.statesHint")}</p>
              <div className="flex justify-center gap-6">
                {STATES.map((state) => (
                  <StateUploadSlot key={state} profile={profile} state={state} imageUrl={images[state]} />
                ))}
              </div>
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}
