import {
  Activity as ActivityIcon,
  Bot,
  DatabaseBackup,
  Globe,
  HeartPulse,
  Info,
  type LucideIcon,
  Mic,
  BrainCircuit,
  Palette,
  Settings as SettingsIcon,
  Shield,
  Volume2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/design-system/Dialog";
import { cn } from "@/lib/theme/cn";
import { useNavigationStore } from "@/stores/navigationStore";

import { ActivityPage } from "../activity/ActivityPage";
import { AiProvidersSettings } from "./AiProvidersSettings";
import { AppearanceSettingsSection } from "./AppearanceSettings";
import { BackupSettings } from "./BackupSettings";
import { CreditsSettings } from "./CreditsSettings";
import { DiagnosticsSettings } from "./DiagnosticsSettings";
import { GeneralSettings } from "./GeneralSettings";
import { MemorySettings } from "./MemorySettings";
import { PrivacySettings } from "./PrivacySettings";
import { SttSettings } from "./SttSettings";
import { TtsSettings } from "./TtsSettings";
import { WebSearchSettings } from "./WebSearchSettings";

export const SECTIONS = [
  "general",
  "appearance",
  "aiProviders",
  "stt",
  "tts",
  "webSearch",
  "memory",
  "privacy",
  "backup",
  "diagnostics",
  "activity",
  "credits",
] as const;

export type SettingsSection = (typeof SECTIONS)[number];

export const SECTION_ICONS: Record<SettingsSection, LucideIcon> = {
  general: SettingsIcon,
  appearance: Palette,
  aiProviders: Bot,
  stt: Mic,
  tts: Volume2,
  webSearch: Globe,
  memory: BrainCircuit,
  privacy: Shield,
  backup: DatabaseBackup,
  diagnostics: HeartPulse,
  activity: ActivityIcon,
  credits: Info,
};

const SECTION_COMPONENTS: Record<SettingsSection, () => JSX.Element | null> = {
  general: GeneralSettings,
  appearance: AppearanceSettingsSection,
  aiProviders: AiProvidersSettings,
  tts: TtsSettings,
  stt: SttSettings,
  webSearch: WebSearchSettings,
  memory: MemorySettings,
  privacy: PrivacySettings,
  backup: BackupSettings,
  diagnostics: DiagnosticsSettings,
  activity: ActivityPage,
  credits: CreditsSettings,
};

/** Ventana flotante en vez de pestaña propia — mismo espíritu que
 * `CallSettingsDialog` (CallPage.tsx): pedido del usuario de que
 * Configuración se vea y se abra igual que los ajustes de la Llamada.
 * Montado una sola vez en `App.tsx`; su visibilidad vive en
 * `navigationStore` (`isSettingsOpen`) para que tanto el botón de
 * engranaje de la Sidebar como el Command Palette puedan abrirla. */
export function SettingsDialog() {
  const { t } = useTranslation();
  const isOpen = useNavigationStore((s) => s.isSettingsOpen);
  const closeSettings = useNavigationStore((s) => s.closeSettings);
  const pendingSettingsSection = useNavigationStore((s) => s.pendingSettingsSection);
  const clearPendingSettingsSection = useNavigationStore((s) => s.clearPendingSettingsSection);

  const [activeSection, setActiveSection] = useState<SettingsSection>("general");

  useEffect(() => {
    if (!pendingSettingsSection) return;
    setActiveSection(pendingSettingsSection);
    clearPendingSettingsSection();
  }, [pendingSettingsSection, clearPendingSettingsSection]);

  const ActiveContent = SECTION_COMPONENTS[activeSection];

  return (
    <Dialog open={isOpen} onOpenChange={(open) => (open ? undefined : closeSettings())}>
      <DialogContent size="xl">
        <DialogTitle className="text-base font-medium text-text-primary">{t("sidebar.settings")}</DialogTitle>

        <div className="mt-4 flex h-[65vh] overflow-hidden rounded-md border border-border">
          <div className="flex w-56 shrink-0 flex-col overflow-y-auto border-r border-border py-2">
            {SECTIONS.map((section) => {
              const Icon = SECTION_ICONS[section];
              return (
                <button
                  key={section}
                  type="button"
                  onClick={() => setActiveSection(section)}
                  className={cn(
                    "mx-2 flex items-center gap-2.5 rounded-md px-2.5 py-2 text-left text-sm transition-colors",
                    section === activeSection
                      ? "bg-surface-secondary font-medium text-text-primary"
                      : "text-text-secondary hover:bg-surface-secondary hover:text-text-primary",
                  )}
                >
                  <Icon className="h-4 w-4 shrink-0" />
                  <span className="truncate">{t(`settings.tabs.${section}`)}</span>
                </button>
              );
            })}
          </div>

          <div key={activeSection} className="flex-1 overflow-y-auto p-5 animate-fade-in">
            <ActiveContent />
          </div>
        </div>

        <div className="mt-5 flex justify-end">
          <DialogClose asChild>
            <Button variant="secondary">{t("common.close")}</Button>
          </DialogClose>
        </div>
      </DialogContent>
    </Dialog>
  );
}
