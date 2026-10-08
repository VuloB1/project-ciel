import * as DialogPrimitive from "@radix-ui/react-dialog";
import { Search, Settings as SettingsIcon } from "lucide-react";
import { useEffect, useMemo, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";

import type { IconComponent } from "@/design-system/PlatformIcons";
import { SECTIONS, SECTION_ICONS } from "@/features/settings/SettingsDialog";
import { cn } from "@/lib/theme/cn";
import { useNavigationStore } from "@/stores/navigationStore";

import { NAV_GROUPS } from "./Sidebar";

interface Command {
  id: string;
  label: string;
  icon: IconComponent;
  run: () => void;
}

/** Global `Ctrl+K` (`Cmd+K` on the off chance this ever runs on macOS) to
 * jump straight to any of the 8 sidebar tabs or any of the 9 Configuración
 * sub-sections, without clicking through — there was no keyboard shortcut
 * or global search anywhere in the app before this. Reuses `NAV_GROUPS`
 * (Sidebar.tsx) and `SECTIONS`/`SECTION_ICONS` (SettingsDialog.tsx) as the
 * single source of truth for what exists, instead of a second hardcoded
 * list that could drift out of sync. Mounted once, unconditionally, in
 * `App.tsx` — works from any tab, including while typing in Chat's
 * composer (keydown bubbles to `window` regardless of focus) and in
 * immersive mode (no sidebar visible at all). */
export function CommandPalette() {
  const { t } = useTranslation();
  const setActiveTab = useNavigationStore((s) => s.setActiveTab);
  const openSettings = useNavigationStore((s) => s.openSettings);
  const openSettingsSection = useNavigationStore((s) => s.openSettingsSection);

  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);

  useEffect(() => {
    function handleGlobalKeyDown(event: globalThis.KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setOpen((prev) => !prev);
      }
    }
    window.addEventListener("keydown", handleGlobalKeyDown);
    return () => window.removeEventListener("keydown", handleGlobalKeyDown);
  }, []);

  const commands = useMemo<Command[]>(() => {
    const tabCommands = NAV_GROUPS.flatMap((group) => group.items).map((item) => ({
      id: `tab:${item.tab}`,
      label: t(item.labelKey),
      icon: item.icon,
      run: () => setActiveTab(item.tab),
    }));
    const settingsSectionCommands = SECTIONS.map((section) => ({
      id: `settings:${section}`,
      label: `${t("sidebar.settings")} — ${t(`settings.tabs.${section}`)}`,
      icon: SECTION_ICONS[section],
      run: () => openSettingsSection(section),
    }));
    return [
      ...tabCommands,
      { id: "tab:settings", label: t("sidebar.settings"), icon: SettingsIcon, run: () => openSettings() },
      ...settingsSectionCommands,
    ];
  }, [t, setActiveTab, openSettings, openSettingsSection]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter((command) => command.label.toLowerCase().includes(q));
  }, [commands, query]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  function handleOpenChange(next: boolean) {
    setOpen(next);
    if (!next) {
      setQuery("");
      setSelectedIndex(0);
    }
  }

  function handleSelect(command: Command) {
    command.run();
    handleOpenChange(false);
  }

  function handleInputKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelectedIndex((i) => Math.min(i + 1, filtered.length - 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelectedIndex((i) => Math.max(i - 1, 0));
    } else if (event.key === "Enter") {
      event.preventDefault();
      const command = filtered[selectedIndex];
      if (command) handleSelect(command);
    }
  }

  return (
    <DialogPrimitive.Root open={open} onOpenChange={handleOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/60" />
        <DialogPrimitive.Content className="animate-dialog-pop-in fixed left-1/2 top-[15vh] z-50 w-full max-w-lg -translate-x-1/2 overflow-hidden rounded-lg border border-border bg-surface shadow-md focus:outline-none">
          <DialogPrimitive.Title className="sr-only">{t("commandPalette.placeholder")}</DialogPrimitive.Title>
          <DialogPrimitive.Description className="sr-only">{t("commandPalette.placeholder")}</DialogPrimitive.Description>

          <div className="flex items-center gap-2 border-b border-border px-4 py-3">
            <Search className="h-4 w-4 shrink-0 text-text-secondary" />
            <input
              autoFocus
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={handleInputKeyDown}
              placeholder={t("commandPalette.placeholder")}
              className="h-6 w-full border-0 bg-transparent text-sm text-text-primary placeholder:text-text-secondary focus:outline-none"
            />
          </div>

          <div className="max-h-80 overflow-y-auto p-2">
            {filtered.length === 0 ? (
              <p className="px-2.5 py-6 text-center text-sm text-text-secondary">{t("commandPalette.noResults")}</p>
            ) : (
              filtered.map((command, index) => {
                const Icon = command.icon;
                return (
                  <button
                    key={command.id}
                    type="button"
                    onClick={() => handleSelect(command)}
                    onMouseEnter={() => setSelectedIndex(index)}
                    className={cn(
                      "flex w-full items-center gap-2.5 rounded-md px-2.5 py-2 text-left text-sm transition-colors",
                      index === selectedIndex ? "bg-surface-secondary text-text-primary" : "text-text-secondary",
                    )}
                  >
                    <Icon className="h-4 w-4 shrink-0" />
                    <span className="truncate">{command.label}</span>
                  </button>
                );
              })
            )}
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
