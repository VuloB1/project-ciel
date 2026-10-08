import { ChevronsUpDown, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/design-system/DropdownMenu";
import { ProfileAvatarBadge } from "@/design-system/ProfileAvatarBadge";
import { MoodBadge } from "@/features/identity/MoodBadge";
import { cn } from "@/lib/theme/cn";
import { useAvatarStore } from "@/stores/avatarStore";
import { useNavigationStore } from "@/stores/navigationStore";
import { useProfileStore } from "@/stores/profileStore";

/** Global "who am I talking to" selector (sección 105 del brief). */
export function ProfileSwitcher({ collapsed = false }: { collapsed?: boolean }) {
  const { t } = useTranslation();
  const profiles = useProfileStore((s) => s.profiles);
  const activeProfileId = useProfileStore((s) => s.activeProfileId);
  const setActiveProfileId = useProfileStore((s) => s.setActiveProfileId);
  const createProfile = useProfileStore((s) => s.create);
  const setActiveTab = useNavigationStore((s) => s.setActiveTab);
  const visualState = useAvatarStore((s) => s.visualState);

  const activeProfile = profiles.find((p) => p.id === activeProfileId) ?? null;

  async function handleCreate() {
    await createProfile(t("identity.defaultName"));
    setActiveTab("identity");
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          title={collapsed ? (activeProfile?.name ?? t("app.name")) : undefined}
          className={cn(
            "flex w-full items-center gap-2 rounded-md py-2 text-left transition-colors hover:bg-surface-secondary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
            collapsed ? "justify-center px-0" : "px-2",
          )}
        >
          <div className="relative shrink-0">
            <ProfileAvatarBadge profile={activeProfile} size="md" />
            {/* Refleja `avatarStore.visualState` — idle no muestra nada
                para no meter ruido visual permanente; solo aparece durante
                los tramos transitorios (pensando/hablando), que ya vienen
                cableados por `App.tsx` a través de `setTrackedProfileId`. */}
            {activeProfile && visualState !== "idle" ? (
              <span
                aria-hidden="true"
                className={cn(
                  "absolute -bottom-0.5 -right-0.5 h-2.5 w-2.5 animate-pulse rounded-full border-2 border-surface",
                  visualState === "talking" ? "bg-success" : "bg-accent",
                )}
              />
            ) : null}
          </div>
          {!collapsed ? (
            <>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <p className="truncate text-sm font-semibold text-text-primary">{activeProfile?.name ?? t("app.name")}</p>
                  {activeProfile && <MoodBadge profileId={activeProfile.id} />}
                </div>
                <p className="truncate text-xs text-text-secondary">
                  {activeProfile ? t("profileSwitcher.subtitle") : t("profileSwitcher.noProfile")}
                </p>
              </div>
              <ChevronsUpDown className="h-4 w-4 shrink-0 text-text-secondary" />
            </>
          ) : null}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        {profiles.length > 0 ? (
          <>
            {profiles.map((profile) => (
              <DropdownMenuItem key={profile.id} onSelect={() => setActiveProfileId(profile.id)}>
                <ProfileAvatarBadge profile={profile} size="sm" />
                <span className="truncate">{profile.name}</span>
              </DropdownMenuItem>
            ))}
            <DropdownMenuSeparator />
          </>
        ) : null}
        <DropdownMenuItem onSelect={() => void handleCreate()}>
          <Plus className="h-4 w-4" />
          {t("profileSwitcher.create")}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
