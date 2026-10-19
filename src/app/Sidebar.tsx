import {
  BrainCircuit,
  Fingerprint,
  MessageCircle,
  Mic2,
  PanelLeft,
  PanelLeftClose,
  Phone,
  Settings,
  UserRound,
  Volume2,
  VolumeX,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { IconButton } from "@/design-system/IconButton";
import { DiscordIcon, TwitchIcon, type IconComponent } from "@/design-system/PlatformIcons";
import { errorMessage } from "@/lib/errorMessage";
import { cn } from "@/lib/theme/cn";
import { SeasonalSidebarPiece } from "@/features/seasonal/SeasonalDecor";
import { useCallStore } from "@/stores/callStore";
import { useDiscordStore } from "@/stores/discordStore";
import { useNavigationStore, type SidebarTab } from "@/stores/navigationStore";
import { useProactiveStore } from "@/stores/proactiveStore";
import { useProfileStore } from "@/stores/profileStore";
import { useToastStore } from "@/stores/toastStore";
import { useTwitchStore } from "@/stores/twitchStore";
import { useVoiceStore } from "@/stores/voiceStore";

import { ProfileSwitcher } from "./ProfileSwitcher";

export const NAV_GROUPS: { labelKey: string; items: { tab: SidebarTab; icon: IconComponent; labelKey: string }[] }[] = [
  {
    labelKey: "sidebar.groups.character",
    items: [
      { tab: "chat", icon: MessageCircle, labelKey: "sidebar.chat" },
      { tab: "identity", icon: Fingerprint, labelKey: "sidebar.identity" },
      { tab: "memory", icon: BrainCircuit, labelKey: "sidebar.memory" },
      { tab: "voice", icon: Mic2, labelKey: "sidebar.voice" },
      { tab: "call", icon: Phone, labelKey: "sidebar.call" },
      { tab: "avatar", icon: UserRound, labelKey: "sidebar.avatar" },
    ],
  },
  {
    labelKey: "sidebar.groups.platforms",
    items: [
      { tab: "twitch", icon: TwitchIcon, labelKey: "sidebar.twitch" },
      { tab: "discord", icon: DiscordIcon, labelKey: "sidebar.discord" },
    ],
  },
];

export function Sidebar() {
  const { t } = useTranslation();
  const activeTab = useNavigationStore((s) => s.activeTab);
  const setActiveTab = useNavigationStore((s) => s.setActiveTab);
  const isSettingsOpen = useNavigationStore((s) => s.isSettingsOpen);
  const openSettings = useNavigationStore((s) => s.openSettings);
  const collapsed = useNavigationStore((s) => s.sidebarCollapsed);
  const toggleCollapsed = useNavigationStore((s) => s.toggleSidebarCollapsed);
  const activeProfileId = useProfileStore((s) => s.activeProfileId);
  const hasUnreadProactiveMessage = useProactiveStore((s) =>
    activeProfileId ? Boolean(s.unreadConversationByProfile[activeProfileId]) : false,
  );
  // Pulsing dot on Twitch/Discord/Llamada while each is actually listening
  // or in a call — pedido del usuario para saber de un vistazo cuáles están
  // "en vivo" sin tener que abrir la pestaña.
  const twitchStatus = useTwitchStore((s) => s.status);
  const discordStatus = useDiscordStore((s) => s.status);
  const callStatus = useCallStore((s) => s.status);
  const LIVE_TABS: Partial<Record<SidebarTab, boolean>> = {
    twitch: twitchStatus === "running",
    discord: discordStatus === "running",
    call: callStatus === "running",
  };
  const voiceMuted = useVoiceStore((s) => s.muted);
  const setVoiceMuted = useVoiceStore((s) => s.setMuted);
  const pushToast = useToastStore((s) => s.push);

  /** Global mute — silences Chat, Twitch, Discord and Llamada at once and
   * stops whatever's playing right now (see `voice_set_muted`'s Rust doc
   * comment). Placed next to the avatar window controls rather than
   * inside any one tab, so it stays reachable no matter which page is
   * open. */
  async function handleToggleVoiceMuted() {
    if (voiceMuted === null) return;
    try {
      await setVoiceMuted(!voiceMuted);
    } catch (err) {
      pushToast({ variant: "error", title: t("voice.muteError"), description: errorMessage(err) });
    }
  }

  return (
    <aside className={cn("flex shrink-0 flex-col border-r border-border bg-surface", collapsed ? "w-14" : "w-60")}>
      <div className="px-2 pb-2 pt-3">
        <ProfileSwitcher collapsed={collapsed} />
      </div>

      <nav className="flex flex-1 flex-col gap-3 overflow-y-auto px-2 pb-4">
        {NAV_GROUPS.map((group) => (
          <div key={group.labelKey} className="flex flex-col gap-0.5">
            {!collapsed ? (
              <span className="px-3 pb-1 text-xs font-medium uppercase tracking-wide text-text-secondary">
                {t(group.labelKey)}
              </span>
            ) : null}
            {group.items.map(({ tab, icon: Icon, labelKey }) => {
              const isActive = activeTab === tab;
              const showUnreadDot = tab === "chat" && hasUnreadProactiveMessage;
              const showLiveDot = LIVE_TABS[tab] ?? false;
              return (
                <button
                  key={tab}
                  type="button"
                  onClick={() => setActiveTab(tab)}
                  aria-current={isActive ? "page" : undefined}
                  title={collapsed ? t(labelKey) : undefined}
                  className={cn(
                    "flex items-center gap-3 rounded-md px-3 py-2 text-left text-sm transition-colors duration-150",
                    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
                    collapsed && "justify-center px-0",
                    isActive
                      ? "bg-surface-secondary text-accent"
                      : "text-text-secondary hover:bg-surface-secondary hover:text-text-primary",
                  )}
                >
                  <span className="relative shrink-0">
                    <Icon className="h-4 w-4" />
                    {/* El personaje mandó un mensaje por su cuenta (modo
                        proactivo) mientras el usuario estaba en otra
                        pestaña — ver docs/ADR/007-proactive-mode.md. Como
                        punto absoluto cuando está colapsado (no hay lugar
                        para el punto inline de al lado del texto). */}
                    {collapsed && showUnreadDot ? (
                      <span aria-hidden="true" className="absolute -right-0.5 -top-0.5 h-1.5 w-1.5 rounded-full bg-accent" />
                    ) : null}
                    {collapsed && showLiveDot ? (
                      <span aria-hidden="true" className="absolute -right-0.5 -top-0.5 h-1.5 w-1.5 animate-pulse rounded-full bg-success" />
                    ) : null}
                  </span>
                  {!collapsed ? <span className="flex-1 truncate">{t(labelKey)}</span> : null}
                  {!collapsed && showUnreadDot ? (
                    <span aria-hidden="true" className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent" />
                  ) : null}
                  {!collapsed && showLiveDot ? (
                    <span aria-hidden="true" className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-success" />
                  ) : null}
                </button>
              );
            })}
          </div>
        ))}
      </nav>

      <SeasonalSidebarPiece collapsed={collapsed} />

      <div
        className={cn(
          "flex items-center gap-2 border-t border-border px-3 py-2.5",
          collapsed ? "flex-col" : "justify-between",
        )}
      >
        <button
          type="button"
          onClick={() => openSettings()}
          aria-pressed={isSettingsOpen}
          title={collapsed ? t("sidebar.settings") : undefined}
          className={cn(
            "flex min-w-0 items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors duration-150",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
            collapsed ? "justify-center" : "flex-1",
            isSettingsOpen
              ? "bg-surface-secondary text-accent"
              : "text-text-secondary hover:bg-surface-secondary hover:text-text-primary",
          )}
        >
          <Settings className="h-4 w-4 shrink-0" />
          {!collapsed ? <span className="truncate">{t("sidebar.settings")}</span> : null}
        </button>
        <IconButton
          icon={collapsed ? <PanelLeft className="h-4 w-4" /> : <PanelLeftClose className="h-4 w-4" />}
          label={collapsed ? t("sidebar.expand") : t("sidebar.collapse")}
          title={collapsed ? t("sidebar.expand") : t("sidebar.collapse")}
          onClick={toggleCollapsed}
        />
        <IconButton
          icon={voiceMuted ? <VolumeX className="h-4 w-4" /> : <Volume2 className="h-4 w-4" />}
          label={voiceMuted ? t("voice.unmuteAll") : t("voice.muteAll")}
          title={voiceMuted ? t("voice.unmuteAll") : t("voice.muteAll")}
          active={voiceMuted ?? false}
          disabled={voiceMuted === null}
          onClick={() => void handleToggleVoiceMuted()}
        />
      </div>
    </aside>
  );
}
