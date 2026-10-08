import { Mic2 } from "lucide-react";
import { useTranslation } from "react-i18next";

import { EmptyState } from "@/design-system/EmptyState";
import { LoadingState } from "@/design-system/LoadingState";
import { useProfileStore } from "@/stores/profileStore";

import { VoiceEditor } from "./VoiceEditor";

export function VoicePage() {
  const { t } = useTranslation();
  const isLoading = useProfileStore((s) => s.isLoading);
  const profiles = useProfileStore((s) => s.profiles);
  const activeProfileId = useProfileStore((s) => s.activeProfileId);

  const activeProfile = profiles.find((p) => p.id === activeProfileId) ?? null;

  if (isLoading) {
    return <LoadingState label={t("voice.loading")} />;
  }

  if (!activeProfile) {
    return (
      <EmptyState
        icon={<Mic2 className="h-6 w-6" />}
        title={t("voice.emptyTitle")}
        description={t("voice.emptyDescription")}
      />
    );
  }

  return <VoiceEditor key={activeProfile.id} profile={activeProfile} />;
}
