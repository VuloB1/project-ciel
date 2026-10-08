import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { AppContextMenu } from "@/app/AppContextMenu";
import { CommandPalette } from "@/app/CommandPalette";
import { Sidebar } from "@/app/Sidebar";
import { Titlebar } from "@/app/Titlebar";
import { ConfirmDialog } from "@/design-system/Dialog";
import { Toaster } from "@/design-system/Toast";
import { errorMessage } from "@/lib/errorMessage";
import { setCallActiveProfile } from "@/lib/ipc/call";
import { setDiscordActiveProfile } from "@/lib/ipc/discord";
import { listLlmProviders } from "@/lib/ipc/settings";
import { useAppEvent } from "@/lib/ipc/events";
import { importProfileFromPath, takePendingImportPath } from "@/lib/ipc/profiles";
import { setProactiveActiveProfile } from "@/lib/ipc/proactive";
import { setTwitchActiveProfile } from "@/lib/ipc/twitch";
import { notifyProactiveMessage } from "@/lib/notifications";
import { useActiveProfileTheme } from "@/lib/theme/useActiveProfileTheme";
import { useAppearanceSettings } from "@/lib/theme/useAppearanceSettings";
import { useAvatarStore } from "@/stores/avatarStore";
import { useCallStore } from "@/stores/callStore";
import { useChatStore } from "@/stores/chatStore";
import { useDiscordStore } from "@/stores/discordStore";
import { useMemoryStore } from "@/stores/memoryStore";
import { useMoodStore } from "@/stores/moodStore";
import { useNavigationStore, type SidebarTab } from "@/stores/navigationStore";
import { useProactiveStore } from "@/stores/proactiveStore";
import { useProfileStore } from "@/stores/profileStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useToastStore } from "@/stores/toastStore";
import { useTwitchStore } from "@/stores/twitchStore";
import { useVoiceStore } from "@/stores/voiceStore";

import { AvatarPage } from "@/features/avatar/AvatarPage";
import { CallPage } from "@/features/call/CallPage";
import { ChatPage } from "@/features/chat/ChatPage";
import { DiscordPage } from "@/features/discord/DiscordPage";
import { IdentityPage } from "@/features/identity/IdentityPage";
import { MemoryPage } from "@/features/memory/MemoryPage";
import { WelcomeScreen } from "@/features/onboarding/WelcomeScreen";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { useRestoreFlow } from "@/features/settings/useRestoreFlow";
import { TwitchPage } from "@/features/twitch/TwitchPage";
import { VoicePage } from "@/features/voice/VoicePage";

/** `"C:\\Users\\...\\Ciel.aivtprofile"` -> `"Ciel"` — solo para mostrar
 * algo legible en el diálogo de confirmación, no se usa para nada más. */
function fileDisplayName(path: string): string {
  const base = path.split(/[\\/]/).pop() ?? path;
  return base.replace(/\.aivt(profile|backup)$/i, "");
}

const isBackupFile = (path: string) => path.toLowerCase().endsWith(".aivtbackup");

const PAGES: Record<SidebarTab, () => JSX.Element> = {
  chat: ChatPage,
  identity: IdentityPage,
  memory: MemoryPage,
  voice: VoicePage,
  call: CallPage,
  twitch: TwitchPage,
  discord: DiscordPage,
  avatar: AvatarPage,
};

export function App() {
  const { t, i18n } = useTranslation();
  const pushToast = useToastStore((s) => s.push);
  const activeTab = useNavigationStore((s) => s.activeTab);
  const loadSettings = useSettingsStore((s) => s.load);
  const loadProfiles = useProfileStore((s) => s.load);
  const loadVoiceMuted = useVoiceStore((s) => s.loadMuted);
  const activeProfileId = useProfileStore((s) => s.activeProfileId);
  const isSettingsLoading = useSettingsStore((s) => s.isLoading);
  const onboardingCompleted = useSettingsStore((s) => s.onboardingCompleted);
  // Ruta de un `.aivtprofile` que el usuario abrió desde el sistema
  // operativo (doble clic) — arranque en frío (`takePendingImportPath`,
  // consultado una sola vez al montar) o app ya corriendo
  // (`profile_import_requested` en `useAppEvent` más abajo). No se importa
  // directo: siempre se confirma primero (pedido del usuario).
  const [importPath, setImportPath] = useState<string | null>(null);
  // Un `.aivtbackup` abierto desde el sistema operativo pasa por el mismo
  // flujo de restauración en dos pasos que Configuración > Copia de seguridad
  // (valida, pide confirmación, y recién al confirmar reinicia y restaura).
  const restore = useRestoreFlow();
  useActiveProfileTheme();
  useAppearanceSettings();

  function handleOpenedFile(path: string) {
    if (isBackupFile(path)) void restore.begin(path);
    else setImportPath(path);
  }

  useEffect(() => {
    void loadSettings();
    void loadProfiles();
    // The Sidebar's global mute button (next to "mostrar ventana
    // flotante") needs the current mute state regardless of which tab is
    // open — it isn't scoped to any one page like `DiscordPage`'s own
    // `load()` call.
    void loadVoiceMuted();
    void takePendingImportPath().then((path) => {
      if (path) handleOpenedFile(path);
    });
  }, [loadSettings, loadProfiles, loadVoiceMuted]);

  useEffect(() => {
    // The avatar overlay window is a separate webview with its own store
    // instance and sets this from its URL instead — this only drives the
    // main window's own avatar preview/state.
    useAvatarStore.getState().setTrackedProfileId(activeProfileId);
    // The backend has no other way to know which character should answer
    // in Twitch chat / Discord / a Llamada — see `TwitchService`'s doc
    // comment.
    void setTwitchActiveProfile(activeProfileId);
    void setDiscordActiveProfile(activeProfileId);
    void setCallActiveProfile(activeProfileId);
    void setProactiveActiveProfile(activeProfileId);
  }, [activeProfileId]);

  useAppEvent((event) => {
    if (event.type === "heartbeat") {
      // Proves the EventBus -> Tauri emit -> frontend bridge end to end;
      // nothing user-facing reads heartbeats yet (sección 118 del brief).
      console.debug("[AIVT] heartbeat", event.timestamp);
      return;
    }
    if (event.type.startsWith("ai_response_")) {
      // Subscribed at the app level (not inside ChatPage) so streaming
      // state stays correct even if the user switches tabs mid-generation.
      useChatStore.getState().handleAppEvent(event);
      useAvatarStore.getState().handleAppEvent(event);
      return;
    }
    if (event.type === "chat_audio_saved") {
      // Fires from Chat's detached auto-play task once Configuración >
      // Privacidad's "guardar audio" persistence finishes writing a
      // message's clip to disk — can arrive well after the reply already
      // finished streaming and after the user switched tabs, so this is
      // subscribed at the app level for the same reason as ai_response_*
      // above, not inside ChatWindow.
      useChatStore.getState().handleAppEvent(event);
      return;
    }
    if (event.type === "memories_updated") {
      // Automatic consolidation runs in a detached background task with no
      // other way to reach a Memoria tab that's already been visited —
      // subscribed here (not inside MemoryPage) so it refreshes even while
      // the user is elsewhere in the app.
      useMemoryStore.getState().handleAppEvent(event);
      return;
    }
    if (event.type === "mood_updated") {
      // Fires from the same detached consolidation task as memories_updated
      // above, for the same reason — subscribed here so `MoodBadge` stays
      // current even while the user is on a different tab.
      useMoodStore.getState().handleAppEvent(event);
      return;
    }
    if (event.type === "proactive_message_sent") {
      // Fires from `ProactiveService`'s background loop, which can trigger
      // regardless of which tab the user has open — subscribed here so the
      // Sidebar's "no leído" indicator lights up even from another pestaña.
      useProactiveStore.getState().handleAppEvent(event);
      if (useSettingsStore.getState().background?.notifyProactive) {
        const profile = useProfileStore.getState().profiles.find((p) => p.id === event.profileId);
        if (profile) void notifyProactiveMessage(profile.id, profile.name, event.content);
      }
      return;
    }
    if (event.type.startsWith("voice_playback_")) {
      // Auto-play fires from the backend right after a chat response
      // finishes, regardless of which tab is open — subscribed at the app
      // level (not inside ChatWindow) for the same reason as ai_response_*.
      useVoiceStore.getState().handleAppEvent(event);
      useAvatarStore.getState().handleAppEvent(event);
      return;
    }
    if (event.type === "service_status_changed" || event.type.startsWith("twitch_message_")) {
      // The connection can run (and messages can arrive) while the user is
      // on a completely different tab — subscribed here for the same
      // reason as every other background-producer event above.
      useTwitchStore.getState().handleAppEvent(event);
    }
    if (event.type === "service_status_changed" || event.type.startsWith("discord_message_")) {
      // Same reasoning as Twitch above.
      useDiscordStore.getState().handleAppEvent(event);
    }
    if (event.type === "service_status_changed" || event.type.startsWith("call_")) {
      // Same reasoning as Twitch above — a call can be running (or the
      // character mid-reply) while the user is on a different tab.
      // `service_status_changed` is shared by every `ManagedService`; each
      // store filters it down to its own `service` name internally.
      useCallStore.getState().handleAppEvent(event);
    }
    if (event.type === "llm_provider_switched") {
      // El cambio automático de proveedor ocurre a mitad de un mensaje, con el
      // usuario en cualquier pestaña: se avisa a nivel de app y con el motivo.
      void announceProviderSwitch(event.fromProviderId, event.toProviderId, event.reason);
    }
    if (event.type === "profile_import_requested") {
      // Doble clic en un `.aivtprofile` con la app ya corriendo — mismo
      // diálogo de confirmación que el arranque en frío, ver `importPath`.
      handleOpenedFile(event.path);
    }
  });

  async function announceProviderSwitch(fromId: string, toId: string, reason: string) {
    let from = fromId;
    let to = toId;
    try {
      const { providers } = await listLlmProviders();
      from = providers.find((p) => p.id === fromId)?.label ?? fromId;
      to = providers.find((p) => p.id === toId)?.label ?? toId;
    } catch {
      // Sin etiquetas el aviso sale con los ids: mejor eso que ningún aviso.
    }
    pushToast({ variant: "info", title: t(`failover.${reason}`, { from, to, defaultValue: t("failover.generic", { from, to }) }) });
  }

  async function handleConfirmImport() {
    if (!importPath) return;
    const path = importPath;
    setImportPath(null);
    try {
      const imported = await importProfileFromPath(path);
      await useProfileStore.getState().load();
      pushToast({ variant: "success", title: t("onboarding.importSuccess", { name: imported.name }) });
    } catch (err) {
      pushToast({ variant: "error", title: t("onboarding.importError"), description: errorMessage(err) });
    }
  }

  const ActivePage = PAGES[activeTab];
  const immersiveMode = useNavigationStore((s) => s.immersiveMode);
  // Mientras `settingsStore.load()` no resuelve, `onboardingCompleted` sigue
  // `null` — se espera acá (main vacío, sin loader) en vez de mostrar el
  // layout normal un instante y después saltar al wizard, ya que la carga
  // es local y prácticamente instantánea.
  const isBootstrapping = isSettingsLoading || onboardingCompleted === null;
  const showWelcome = !isBootstrapping && onboardingCompleted === false;

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-background text-text-primary" lang={i18n.language}>
      <Titlebar />
      {showWelcome ? (
        <WelcomeScreen />
      ) : isBootstrapping ? (
        <div className="flex-1" />
      ) : (
        <div className="flex flex-1 overflow-hidden">
          {!immersiveMode ? <Sidebar /> : null}
          <main className="flex-1 overflow-hidden">
            <div key={activeTab} className="h-full animate-fade-in">
              <ActivePage />
            </div>
          </main>
        </div>
      )}
      <Toaster />
      <CommandPalette />
      <SettingsDialog />
      <AppContextMenu />
      {restore.dialog}
      <ConfirmDialog
        open={importPath !== null}
        onOpenChange={(open) => {
          if (!open) setImportPath(null);
        }}
        title={t("onboarding.importConfirmTitle", { name: importPath ? fileDisplayName(importPath) : "" })}
        description={t("onboarding.importConfirmDescription")}
        confirmLabel={t("onboarding.importConfirmAction")}
        cancelLabel={t("common.cancel")}
        onConfirm={() => void handleConfirmImport()}
      />
    </div>
  );
}
