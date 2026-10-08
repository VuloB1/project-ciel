import { MessageCircle } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { EmptyState } from "@/design-system/EmptyState";
import { LoadingState } from "@/design-system/LoadingState";
import { ProfileAvatarBadge } from "@/design-system/ProfileAvatarBadge";
import { useChatStore } from "@/stores/chatStore";
import { useNavigationStore } from "@/stores/navigationStore";
import { useProactiveStore } from "@/stores/proactiveStore";
import { useProfileStore } from "@/stores/profileStore";

import { ChatWindow } from "./ChatWindow";
import { ConversationList } from "./ConversationList";

export function ChatPage() {
  const { t } = useTranslation();
  const profiles = useProfileStore((s) => s.profiles);
  const activeProfileId = useProfileStore((s) => s.activeProfileId);
  const setActiveTab = useNavigationStore((s) => s.setActiveTab);

  const activeProfile = profiles.find((p) => p.id === activeProfileId) ?? null;

  const conversations = useChatStore((s) => (activeProfile ? s.conversationsByProfile[activeProfile.id] : undefined));
  const isLoadingConversations = useChatStore((s) => s.isLoadingConversations);
  const activeConversationId = useChatStore((s) => s.activeConversationId);
  const setActiveConversationId = useChatStore((s) => s.setActiveConversationId);
  const loadConversations = useChatStore((s) => s.loadConversations);
  const createNewConversation = useChatStore((s) => s.createNewConversation);
  const clearProactiveUnread = useProactiveStore((s) => s.clearUnread);

  useEffect(() => {
    if (activeProfile) {
      void loadConversations(activeProfile.id);
    }
    setActiveConversationId(null);
    // Reload only when the active profile actually changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeProfile?.id]);

  useEffect(() => {
    if (activeProfile && activeConversationId) {
      clearProactiveUnread(activeProfile.id, activeConversationId);
    }
  }, [activeProfile, activeConversationId, clearProactiveUnread]);

  if (!activeProfile) {
    return (
      <EmptyState
        icon={<MessageCircle className="h-6 w-6" />}
        title={t("placeholder.chat.noProfileTitle")}
        description={t("placeholder.chat.noProfileDescription")}
        action={<Button onClick={() => setActiveTab("identity")}>{t("placeholder.chat.goToIdentity")}</Button>}
      />
    );
  }

  const activeConversation = conversations?.find((c) => c.id === activeConversationId) ?? null;
  const mostRecentConversation = conversations?.[0] ?? null;

  return (
    <div className="flex h-full overflow-hidden">
      <ConversationList profileId={activeProfile.id} />
      {isLoadingConversations && !conversations ? (
        <LoadingState label={t("chat.conversations.loading")} />
      ) : activeConversation ? (
        <ChatWindow key={activeConversation.id} profile={activeProfile} conversationId={activeConversation.id} />
      ) : (
        <div className="flex h-full flex-1 flex-col items-center justify-center gap-4 px-8 text-center">
          <ProfileAvatarBadge profile={activeProfile} size="lg" />
          <div>
            <h2 className="text-base font-medium text-text-primary">
              {t("chat.noConversationTitle", { name: activeProfile.name })}
            </h2>
            <p className="mt-1 max-w-sm text-sm text-text-secondary">{t("chat.noConversationDescription")}</p>
          </div>
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => void createNewConversation(activeProfile.id)}>
              {t("chat.conversations.new")}
            </Button>
            {mostRecentConversation ? (
              <Button onClick={() => setActiveConversationId(mostRecentConversation.id)}>
                {t("chat.continueLast", { title: mostRecentConversation.title })}
              </Button>
            ) : null}
          </div>
        </div>
      )}
    </div>
  );
}
