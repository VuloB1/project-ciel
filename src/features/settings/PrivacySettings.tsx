import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Trash2, Users } from "lucide-react";

import { Badge, type BadgeVariant } from "@/design-system/Badge";
import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { Dialog, DialogContent, DialogTitle } from "@/design-system/Dialog";
import { EmptyState } from "@/design-system/EmptyState";
import { Field } from "@/design-system/Field";
import { IconButton } from "@/design-system/IconButton";
import { Input } from "@/design-system/Input";
import { LoadingState } from "@/design-system/LoadingState";
import { Select } from "@/design-system/Select";
import { Switch } from "@/design-system/Switch";
import { errorMessage } from "@/lib/errorMessage";
import { getChatAudioSettings, setChatAudioSettings, type ChatAudioSettings } from "@/lib/ipc/chat";
import { usePeopleStore } from "@/stores/peopleStore";
import { useToastStore } from "@/stores/toastStore";
import type { PersonRole } from "@/types/person";

const ADDABLE_ROLES: PersonRole[] = ["self", "nickname", "ai", "friend"];

/** Days-based retention options for saved chat audio, from most to least
 * frequent cleanup — pedido del usuario: opciones más frecuentes que 1 mes
 * para quien quiera liberar espacio más seguido. */
const RETENTION_DAY_OPTIONS: { days: number; labelKey: string }[] = [
  { days: 1, labelKey: "retentionOneDay" },
  { days: 7, labelKey: "retentionOneWeek" },
  { days: 30, labelKey: "retentionOneMonth" },
  { days: 60, labelKey: "retentionTwoMonths" },
  { days: 90, labelKey: "retentionThreeMonths" },
  { days: 180, labelKey: "retentionSixMonths" },
];

const ROLE_BADGE_VARIANT: Record<PersonRole, BadgeVariant> = {
  self: "success",
  nickname: "success",
  ai: "warning",
  friend: "neutral",
  twitch_chatter: "neutral",
  discord_chatter: "neutral",
};

export function PrivacySettings() {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const { people, twitchAutoLearnEnabled, discordAutoLearnEnabled, isLoading, load, create, remove, setTwitchAutoLearn, setDiscordAutoLearn } =
    usePeopleStore();

  const [isAddOpen, setIsAddOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const [newRole, setNewRole] = useState<PersonRole>("friend");
  const [isSaving, setIsSaving] = useState(false);

  const [chatAudio, setChatAudio] = useState<ChatAudioSettings | null>(null);

  useEffect(() => {
    void load();
    getChatAudioSettings()
      .then(setChatAudio)
      .catch((err) => pushToast({ variant: "error", title: t("settings.privacy.chatAudio.loadError"), description: errorMessage(err) }));
    // Loaded once on mount, same as the rest of this page's data.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [load]);

  async function handleToggleTwitch(next: boolean) {
    try {
      await setTwitchAutoLearn(next);
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.privacy.saveError"), description: errorMessage(err) });
    }
  }

  async function handleToggleDiscord(next: boolean) {
    try {
      await setDiscordAutoLearn(next);
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.privacy.saveError"), description: errorMessage(err) });
    }
  }

  async function handleChatAudioChange(next: ChatAudioSettings) {
    setChatAudio(next);
    try {
      const saved = await setChatAudioSettings(next);
      setChatAudio(saved);
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.privacy.chatAudio.saveError"), description: errorMessage(err) });
    }
  }

  async function handleCreate() {
    if (!newName.trim()) return;
    setIsSaving(true);
    try {
      await create({ canonicalName: newName.trim(), role: newRole });
      setNewName("");
      setNewRole("friend");
      setIsAddOpen(false);
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.privacy.saveError"), description: errorMessage(err) });
    } finally {
      setIsSaving(false);
    }
  }

  async function handleDelete(id: string) {
    try {
      await remove(id);
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.privacy.saveError"), description: errorMessage(err) });
    }
  }

  if (isLoading) {
    return <LoadingState label={t("settings.privacy.loading")} />;
  }

  return (
    <div className="flex max-w-xl flex-col gap-4">
      {chatAudio ? (
        <Card className="flex flex-col gap-3 p-4">
          <div className="flex items-center justify-between gap-4">
            <div>
              <p className="text-sm font-medium text-text-primary">{t("settings.privacy.chatAudio.title")}</p>
              <p className="mt-1 text-xs text-text-secondary">{t("settings.privacy.chatAudio.hint")}</p>
            </div>
            <Switch
              checked={chatAudio.saveEnabled}
              onCheckedChange={(next) => void handleChatAudioChange({ ...chatAudio, saveEnabled: next })}
              label={t("settings.privacy.chatAudio.title")}
            />
          </div>
          {chatAudio.saveEnabled ? (
            <div className="flex items-center justify-between gap-4">
              <p className="text-sm text-text-primary">{t("settings.privacy.chatAudio.retentionLabel")}</p>
              <Select
                ariaLabel={t("settings.privacy.chatAudio.retentionLabel")}
                value={String(chatAudio.retentionDays)}
                onValueChange={(value) => void handleChatAudioChange({ ...chatAudio, retentionDays: Number(value) })}
                options={RETENTION_DAY_OPTIONS.map(({ days, labelKey }) => ({
                  value: String(days),
                  label: t(`settings.privacy.chatAudio.${labelKey}`),
                }))}
              />
            </div>
          ) : null}
        </Card>
      ) : null}

      <Card className="flex flex-col gap-4 p-4">
        <div>
          <p className="text-sm font-medium text-text-primary">{t("settings.privacy.title")}</p>
          <p className="text-xs text-text-secondary">{t("settings.privacy.description")}</p>
        </div>

        <div className="flex flex-col gap-4 border-t border-border pt-4">
          <div className="flex items-center justify-between gap-4">
            <div>
              <p className="text-sm font-medium text-text-primary">{t("settings.privacy.twitchAutoLearn")}</p>
              <p className="mt-1 text-xs text-text-secondary">{t("settings.privacy.twitchAutoLearnHint")}</p>
            </div>
            <Switch checked={twitchAutoLearnEnabled} onCheckedChange={(next) => void handleToggleTwitch(next)} label={t("settings.privacy.twitchAutoLearn")} />
          </div>

          <div className="flex items-center justify-between gap-4">
            <div>
              <p className="text-sm font-medium text-text-primary">{t("settings.privacy.discordAutoLearn")}</p>
              <p className="mt-1 text-xs text-text-secondary">{t("settings.privacy.discordAutoLearnHint")}</p>
            </div>
            <Switch checked={discordAutoLearnEnabled} onCheckedChange={(next) => void handleToggleDiscord(next)} label={t("settings.privacy.discordAutoLearn")} />
          </div>
        </div>

        <div className="flex flex-col gap-3 border-t border-border pt-4">
          <div className="flex items-center justify-between gap-4">
            <p className="text-sm font-medium text-text-primary">{t("settings.privacy.peopleTitle")}</p>
            <Button size="sm" variant="secondary" onClick={() => setIsAddOpen(true)}>
              {t("settings.privacy.addPerson")}
            </Button>
          </div>

          {people.length === 0 ? (
            <EmptyState icon={<Users className="h-6 w-6" />} title={t("settings.privacy.empty")} description={t("settings.privacy.emptyHint")} />
          ) : (
            // The rounded border lives on this wrapper, with `overflow-hidden`
            // clipping the inner scrollbar to its corners — a scrollbar drawn
            // directly on a `rounded-lg` element ignores that radius and pokes out
            // past the corner, which is what looked broken here. `min-h-0` on the
            // inner flex item is required for `overflow-y-auto` to actually
            // scroll instead of just growing to fit its content — a flex item's
            // default `min-height: auto` would otherwise stop it from ever
            // shrinking below the list's full height, so the outer wrapper just
            // clipped it as dead space instead of letting it scroll.
            <div className="overflow-hidden rounded-lg border border-border">
              <div className="flex max-h-80 min-h-0 flex-col divide-y divide-border overflow-y-auto">
                {people.map((person) => (
                  <div key={person.id} className="flex items-center justify-between gap-4 px-3 py-1.5">
                    <div className="flex items-center gap-2">
                      <span className="text-sm text-text-primary">{person.canonicalName}</span>
                      <Badge variant={ROLE_BADGE_VARIANT[person.role]}>{t(`settings.privacy.role.${person.role}`)}</Badge>
                    </div>
                    <IconButton icon={<Trash2 className="h-4 w-4" />} label={t("settings.privacy.delete")} onClick={() => void handleDelete(person.id)} />
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </Card>

      <Dialog open={isAddOpen} onOpenChange={setIsAddOpen}>
        <DialogContent>
          <DialogTitle className="text-base font-medium text-text-primary">{t("settings.privacy.addPerson")}</DialogTitle>
          <div className="mt-4 flex flex-col gap-4">
            <Field label={t("settings.privacy.name")}>
              <Input value={newName} onChange={(event) => setNewName(event.target.value)} />
            </Field>
            <Field label={t("settings.privacy.roleLabel")}>
              <Select
                ariaLabel={t("settings.privacy.roleLabel")}
                value={newRole}
                onValueChange={(value) => setNewRole(value as PersonRole)}
                options={ADDABLE_ROLES.map((role) => ({ value: role, label: t(`settings.privacy.role.${role}`) }))}
              />
            </Field>
          </div>
          <div className="mt-5 flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setIsAddOpen(false)}>
              {t("settings.privacy.cancel")}
            </Button>
            <Button disabled={!newName.trim() || isSaving} onClick={() => void handleCreate()}>
              {t("settings.privacy.save")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
