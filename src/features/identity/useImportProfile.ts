import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";

import { errorMessage } from "@/lib/errorMessage";
import { importProfileFromPath } from "@/lib/ipc/profiles";
import { useProfileStore } from "@/stores/profileStore";
import { useToastStore } from "@/stores/toastStore";

/** Pick a `.aivtprofile` file, import it as a new profile and make it the
 * active one. Shared by the Identidad header and its empty state. */
export function useImportProfile() {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);

  return async function importProfile() {
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: "AIVT Profile", extensions: ["aivtprofile", "json"] }],
      });
      if (!path || Array.isArray(path)) return;

      const imported = await importProfileFromPath(path);
      useProfileStore.setState((state) => ({
        profiles: [imported, ...state.profiles],
        activeProfileId: imported.id,
      }));
      pushToast({ variant: "success", title: t("profileSwitcher.imported") });
    } catch (err) {
      pushToast({ variant: "error", title: t("profileSwitcher.importError"), description: errorMessage(err) });
    }
  };
}
