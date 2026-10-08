import { Fingerprint, Upload } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { EmptyState } from "@/design-system/EmptyState";
import { LoadingState } from "@/design-system/LoadingState";
import { useProfileStore } from "@/stores/profileStore";

import { IdentityEditor } from "./IdentityEditor";
import { useImportProfile } from "./useImportProfile";

export function IdentityPage() {
  const { t } = useTranslation();
  const isLoading = useProfileStore((s) => s.isLoading);
  const profiles = useProfileStore((s) => s.profiles);
  const activeProfileId = useProfileStore((s) => s.activeProfileId);
  const createProfile = useProfileStore((s) => s.create);
  const importProfile = useImportProfile();

  const activeProfile = profiles.find((p) => p.id === activeProfileId) ?? null;

  if (isLoading) {
    return <LoadingState label={t("identity.loading")} />;
  }

  if (!activeProfile) {
    return (
      <EmptyState
        icon={<Fingerprint className="h-6 w-6" />}
        title={t("identity.emptyTitle")}
        description={t("identity.emptyDescription")}
        action={
          <div className="flex gap-2">
            <Button onClick={() => void createProfile(t("identity.defaultName"))}>{t("identity.createFirst")}</Button>
            <Button variant="ghost" onClick={() => void importProfile()}>
              <Upload className="h-4 w-4" />
              {t("identity.actions.import")}
            </Button>
          </div>
        }
      />
    );
  }

  return <IdentityEditor key={activeProfile.id} profile={activeProfile} />;
}
