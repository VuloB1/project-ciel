import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Card } from "@/design-system/Card";
import { Input } from "@/design-system/Input";
import { errorMessage } from "@/lib/errorMessage";
import { getUserName, setUserName } from "@/lib/ipc/settings";
import { useToastStore } from "@/stores/toastStore";

interface UserNameStepProps {
  /** Ver el doc comment de `StepHelpers` en `WelcomeScreen.tsx`: el nombre se guarda al pulsar
   * "Continuar", no al salir del campo, para no perderlo si se avanza sin salir antes. */
  registerSubmit: (fn: (() => Promise<void>) | null) => void;
}

/** Optional: the name characters will call the user by (also editable in Configuración > General). */
export function UserNameStep({ registerSubmit }: UserNameStepProps) {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const [name, setName] = useState("");

  useEffect(() => {
    getUserName()
      .then((stored) => setName(stored ?? ""))
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    registerSubmit(async () => {
      try {
        await setUserName(name);
      } catch (err) {
        pushToast({ variant: "error", title: t("settings.general.saveError"), description: errorMessage(err) });
        throw err; // the footer's "Continuar" then stays on this step
      }
    });
    return () => registerSubmit(null);
  }, [name, registerSubmit, pushToast, t]);

  return (
    <Card className="p-4">
      <Input
        autoFocus
        aria-label={t("settings.general.userName")}
        maxLength={40}
        placeholder={t("settings.general.userNamePlaceholder")}
        value={name}
        onChange={(event) => setName(event.target.value)}
      />
    </Card>
  );
}
