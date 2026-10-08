import { Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { DialogClose, DialogContent, DialogTitle } from "@/design-system/Dialog";
import { EmptyState } from "@/design-system/EmptyState";
import { LoadingState } from "@/design-system/LoadingState";
import { Switch } from "@/design-system/Switch";
import { errorMessage } from "@/lib/errorMessage";
import { applyMemoryFusion, proposeMemoryFusion, type FusionProposal } from "@/lib/ipc/memory";
import { useMemoryStore } from "@/stores/memoryStore";
import { useToastStore } from "@/stores/toastStore";

/** Opened from `MemorySettings.tsx`'s "Fusionar memorias" button — same
 * shape as `MemoryPage.tsx`'s `MemoryFormDialog` (renders only
 * `DialogContent`, the caller supplies the `Dialog` root and controls
 * visibility). Fetches proposals once on mount (`propose_fusion` is
 * read-only — nothing is written until the user reviews and confirms),
 * lets the user drop individual groups via a switch, then applies only
 * the ones still checked. */
export function MemoryFusionDialog({ profileId, onClose }: { profileId: string; onClose: () => void }) {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const loadMemories = useMemoryStore((s) => s.load);

  const [isLoading, setIsLoading] = useState(true);
  const [proposals, setProposals] = useState<FusionProposal[]>([]);
  const [accepted, setAccepted] = useState<boolean[]>([]);
  const [isApplying, setIsApplying] = useState(false);

  useEffect(() => {
    proposeMemoryFusion(profileId)
      .then((result) => {
        setProposals(result);
        setAccepted(result.map(() => true));
        setIsLoading(false);
      })
      .catch((err) => {
        pushToast({ variant: "error", title: t("settings.memory.fusionError"), description: errorMessage(err) });
        onClose();
      });
    // Solo al montar — este diálogo se remonta desde cero cada vez que se
    // abre (ver `MemorySettings.tsx`), así que no hace falta re-disparar.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const acceptedCount = accepted.filter(Boolean).length;

  function toggle(index: number, value: boolean) {
    setAccepted((prev) => prev.map((a, i) => (i === index ? value : a)));
  }

  async function handleConfirm() {
    const groups = proposals
      .filter((_, index) => accepted[index])
      .map((proposal) => ({
        originalIds: proposal.originals.map((m) => m.id),
        title: proposal.title,
        summary: proposal.summary ?? undefined,
        content: proposal.content,
        category: proposal.category,
        importance: proposal.importance,
      }));
    if (groups.length === 0) {
      onClose();
      return;
    }
    setIsApplying(true);
    try {
      await applyMemoryFusion(profileId, groups);
      await loadMemories(profileId);
      pushToast({ variant: "success", title: t("settings.memory.fusionApplied", { count: groups.length }) });
      onClose();
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.memory.fusionError"), description: errorMessage(err) });
    } finally {
      setIsApplying(false);
    }
  }

  return (
    <DialogContent>
      <DialogTitle className="text-base font-medium text-text-primary">{t("settings.memory.fusionDialogTitle")}</DialogTitle>

      {isLoading ? (
        <div className="py-6">
          <LoadingState label={t("settings.memory.fusionAnalyzing")} />
        </div>
      ) : proposals.length === 0 ? (
        <div className="py-2">
          <EmptyState
            icon={<Sparkles className="h-6 w-6" />}
            title={t("settings.memory.fusionEmptyTitle")}
            description={t("settings.memory.fusionEmptyDescription")}
          />
        </div>
      ) : (
        <div className="-mx-1 mt-4 flex max-h-[60vh] flex-col gap-3 overflow-y-auto px-1">
          {proposals.map((proposal, index) => (
            <Card key={index} className="flex items-start justify-between gap-3 p-3">
              <div className="flex min-w-0 flex-col gap-1">
                {proposal.originals.map((memory) => (
                  <p key={memory.id} className="truncate text-xs text-text-secondary line-through">
                    {memory.title}
                  </p>
                ))}
                <p className="text-sm font-medium text-text-primary">→ {proposal.title}</p>
                <p className="text-xs text-text-secondary">{proposal.summary ?? proposal.content}</p>
              </div>
              <Switch
                label={t("settings.memory.fusionIncludeGroup")}
                checked={accepted[index] ?? false}
                onCheckedChange={(value) => toggle(index, value)}
              />
            </Card>
          ))}
        </div>
      )}

      <div className="mt-5 flex justify-end gap-2">
        <DialogClose asChild>
          <Button variant="ghost">{t("common.cancel")}</Button>
        </DialogClose>
        {proposals.length > 0 ? (
          <Button onClick={() => void handleConfirm()} disabled={isApplying || acceptedCount === 0}>
            {isApplying ? t("identity.saving") : t("settings.memory.fusionConfirm", { count: acceptedCount })}
          </Button>
        ) : null}
      </div>
    </DialogContent>
  );
}
