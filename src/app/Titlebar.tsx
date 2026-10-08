import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

const appWindow = getCurrentWindow();

function MinimizeIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
      <path d="M0 5H10" stroke="currentColor" />
    </svg>
  );
}

function MaximizeIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
      <rect x="0.5" y="0.5" width="9" height="9" stroke="currentColor" />
    </svg>
  );
}

function RestoreIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
      <rect x="2.5" y="0.5" width="7" height="7" stroke="currentColor" />
      <rect x="0.5" y="2.5" width="7" height="7" stroke="currentColor" />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
      <path d="M0.5 0.5L9.5 9.5M9.5 0.5L0.5 9.5" stroke="currentColor" />
    </svg>
  );
}

export function Titlebar() {
  const { t } = useTranslation();
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void appWindow.isMaximized().then((value) => {
      if (!cancelled) setIsMaximized(value);
    });
    void appWindow
      .onResized(() => {
        void appWindow.isMaximized().then((value) => {
          if (!cancelled) setIsMaximized(value);
        });
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return (
    <header
      data-tauri-drag-region
      className="flex h-8 shrink-0 select-none items-center justify-between border-b border-border bg-surface"
    >
      <span data-tauri-drag-region className="flex min-w-0 items-center gap-2 truncate pl-3 text-xs font-medium text-text-secondary">
        <img src="/app-icon.png" alt="" draggable={false} className="h-4 w-4 shrink-0 rounded-[3px]" />
        {t("app.name")}
      </span>
      <div className="flex h-full shrink-0">
        <button
          type="button"
          aria-label={t("titlebar.minimize")}
          onClick={() => void appWindow.minimize()}
          className="inline-flex h-full w-11 items-center justify-center text-text-secondary transition-colors hover:bg-surface-secondary hover:text-text-primary"
        >
          <MinimizeIcon />
        </button>
        <button
          type="button"
          aria-label={isMaximized ? t("titlebar.restore") : t("titlebar.maximize")}
          onClick={() => void appWindow.toggleMaximize()}
          className="inline-flex h-full w-11 items-center justify-center text-text-secondary transition-colors hover:bg-surface-secondary hover:text-text-primary"
        >
          {isMaximized ? <RestoreIcon /> : <MaximizeIcon />}
        </button>
        <button
          type="button"
          aria-label={t("titlebar.close")}
          onClick={() => void appWindow.close()}
          className="inline-flex h-full w-11 items-center justify-center text-text-secondary transition-colors hover:bg-error hover:text-error-foreground"
        >
          <CloseIcon />
        </button>
      </div>
    </header>
  );
}
