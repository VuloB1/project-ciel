import { UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";

import { EmptyState } from "@/design-system/EmptyState";
import { LoadingState } from "@/design-system/LoadingState";
import { useProfileStore } from "@/stores/profileStore";

import { AvatarEditor } from "./AvatarEditor";

export function AvatarPage() {
  const { t } = useTranslation();
  const isLoading = useProfileStore((s) => s.isLoading);
  const profiles = useProfileStore((s) => s.profiles);
  const activeProfileId = useProfileStore((s) => s.activeProfileId);

  const activeProfile = profiles.find((p) => p.id === activeProfileId) ?? null;

  if (isLoading) {
    return <LoadingState label={t("avatar.loading")} />;
  }

  if (!activeProfile) {
    return (
      <EmptyState
        icon={<UserRound className="h-6 w-6" />}
        title={t("avatar.emptyTitle")}
        description={t("avatar.emptyDescription")}
      />
    );
  }

  return <AvatarEditor key={activeProfile.id} profile={activeProfile} />;
}
