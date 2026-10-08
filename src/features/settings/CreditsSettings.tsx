import { getName, getVersion } from "@tauri-apps/api/app";
import { Coffee, ExternalLink, FileText, Github, Globe, Instagram, Scale, Sparkles, type LucideIcon } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Card } from "@/design-system/Card";
import { openExternalUrl, openLegalDocument } from "@/lib/ipc/settings";
import { cn } from "@/lib/theme/cn";

const AUTHOR_NAME = "Vulito";

/** Enlace de Ko-fi del autor: si se deja vacío, el botón de
 * apoyo no se muestra (un botón que no lleva a ningún lado es peor que ninguno). */
const KOFI_URL = "https://ko-fi.com/vulito";

const BUILT_WITH = ["Tauri 2", "Rust", "React", "TypeScript"];

function XIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" className={className}>
      <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z" />
    </svg>
  );
}

type AuthorLink = { url: string; label: string; icon: ReactNode };

const iconClass = "h-4 w-4 shrink-0";
const lucide = (Icon: LucideIcon) => <Icon className={iconClass} />;

const MAIN_LINK: AuthorLink = { url: "https://vulito.carrd.co/", label: "vulito.carrd.co", icon: lucide(Globe) };

const OTHER_LINKS: AuthorLink[] = [
  { url: "https://x.com/VulitoVT", label: "@VulitoVT", icon: <XIcon className={iconClass} /> },
  { url: "https://x.com/VulitoVRC", label: "@VulitoVRC", icon: <XIcon className={iconClass} /> },
  { url: "https://www.instagram.com/vulitovt/", label: "@vulitovt", icon: lucide(Instagram) },
  { url: "https://github.com/VuloB1", label: "VuloB1", icon: lucide(Github) },
];

const legalButtonClass = cn(
  "flex items-center gap-2.5 rounded-md border border-border bg-surface-secondary/50 px-3 py-2 text-left text-sm text-text-primary transition-colors",
  "hover:bg-surface-secondary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
);

function SectionLabel({ children }: { children: ReactNode }) {
  return <p className="text-xs font-medium uppercase tracking-wider text-text-secondary">{children}</p>;
}

function LinkRow({ link, primary = false }: { link: AuthorLink; primary?: boolean }) {
  return (
    <button
      type="button"
      onClick={() => void openExternalUrl(link.url)}
      className={cn(
        "group flex items-center gap-2.5 rounded-md border px-3 py-2 text-left text-sm transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
        primary
          ? "border-accent/40 bg-accent/10 text-text-primary hover:bg-accent/20"
          : "border-border bg-surface-secondary/50 text-text-primary hover:bg-surface-secondary",
      )}
    >
      <span className={cn("transition-colors", primary ? "text-accent" : "text-text-secondary group-hover:text-accent")}>
        {link.icon}
      </span>
      <span className="min-w-0 flex-1 truncate">{link.label}</span>
      <ExternalLink className="h-3.5 w-3.5 shrink-0 text-text-secondary opacity-0 transition-opacity group-hover:opacity-100" />
    </button>
  );
}

export function CreditsSettings() {
  const { t, i18n } = useTranslation();
  const [appName, setAppName] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([getName(), getVersion()])
      .then(([name, version]) => {
        setAppName(name);
        setAppVersion(version);
      })
      .catch(() => {
        setAppName(null);
        setAppVersion(null);
      });
  }, []);

  return (
    <div className="flex max-w-xl flex-col gap-4">
      <Card className="flex flex-col overflow-hidden p-0">
        <section className="flex flex-col items-center gap-3 bg-[radial-gradient(ellipse_at_top,rgb(var(--color-accent)/0.16),transparent_70%)] px-6 pb-7 pt-8 text-center">
          <img
            src="/app-icon.png"
            alt=""
            draggable={false}
            className="h-24 w-24 select-none drop-shadow-[0_0_22px_rgb(var(--color-accent)/0.4)]"
          />
          <div className="flex flex-col items-center gap-1.5">
            <h2 className="text-2xl font-semibold text-text-primary">{appName ?? "Project Ciel"}</h2>
            <span className="rounded-full border border-border bg-surface-secondary px-2.5 py-0.5 text-xs text-text-secondary">
              {appVersion ? t("settings.credits.version", { version: appVersion }) : t("settings.credits.loading")}
            </span>
          </div>
          <p className="max-w-sm text-sm text-text-secondary">{t("settings.credits.tagline")}</p>

          <div className="mt-1 flex flex-col items-center gap-2">
            <SectionLabel>{t("settings.credits.builtWith")}</SectionLabel>
            <div className="flex flex-wrap justify-center gap-1.5">
              {BUILT_WITH.map((tech) => (
                <span key={tech} className="rounded-full border border-border bg-surface px-2.5 py-0.5 text-xs text-text-primary">
                  {tech}
                </span>
              ))}
            </div>
          </div>

          <p className="flex items-center gap-1.5 text-xs text-text-secondary">
            <Sparkles className="h-3.5 w-3.5 text-accent" />
            {t("settings.credits.madeWithClaude")}
          </p>
        </section>

        <section className="flex flex-col gap-3 border-t border-border p-5">
          <div className="flex items-center gap-4">
            <img
              src="/vulito.webp"
              alt=""
              draggable={false}
              className="h-16 w-16 shrink-0 select-none rounded-full border border-border object-cover shadow-sm"
            />
            <div className="flex flex-col gap-0.5">
              <SectionLabel>{t("settings.credits.authorTitle")}</SectionLabel>
              <p className="text-lg font-semibold text-text-primary">{AUTHOR_NAME}</p>
            </div>
          </div>
          <LinkRow link={{ ...MAIN_LINK, label: `${t("settings.credits.website")} · ${MAIN_LINK.label}` }} primary />
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            {OTHER_LINKS.map((link) => (
              <LinkRow key={link.url} link={link} />
            ))}
          </div>

          {KOFI_URL ? (
            <div className="mt-1 flex flex-col gap-2 rounded-md border border-border bg-surface-secondary/50 p-3">
              <p className="text-xs text-text-secondary">{t("settings.credits.supportDescription")}</p>
              <button
                type="button"
                onClick={() => void openExternalUrl(KOFI_URL)}
                className="flex items-center justify-center gap-2 rounded-md bg-accent px-3 py-2 text-sm font-medium text-accent-foreground transition-colors hover:bg-accent-secondary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
              >
                <Coffee className="h-4 w-4" />
                {t("settings.credits.supportAction")}
              </button>
            </div>
          ) : null}
        </section>

        <section className="flex flex-col gap-3 border-t border-border p-5">
          <SectionLabel>{t("settings.credits.thanksTitle")}</SectionLabel>
          <div className="flex items-start gap-3">
            <img
              src="/shumdrador.webp"
              alt=""
              draggable={false}
              className="h-10 w-10 shrink-0 select-none rounded-full border border-border object-cover"
            />
            <div className="flex min-w-0 flex-1 flex-col gap-1">
              <div className="flex items-center justify-between gap-2">
                <p className="text-sm font-medium text-text-primary">Shumdrador</p>
                <button
                  type="button"
                  onClick={() => void openExternalUrl("https://x.com/Shum_vrc")}
                  className="group flex items-center gap-1.5 rounded-md px-1.5 py-1 text-xs text-text-secondary transition-colors hover:bg-surface-secondary hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
                >
                  <XIcon className="h-3.5 w-3.5" />
                  @Shum_vrc
                </button>
              </div>
              <p className="text-xs text-text-secondary">{t("settings.credits.koreanReview")}</p>
            </div>
          </div>
        </section>

        <section className="flex flex-col gap-3 border-t border-border p-5">
          <SectionLabel>{t("settings.credits.legalTitle")}</SectionLabel>
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            <button
              type="button"
              onClick={() => void openLegalDocument("eula", i18n.language.slice(0, 2))}
              className={legalButtonClass}
            >
              <Scale className="h-4 w-4 shrink-0 text-text-secondary" />
              {t("settings.credits.eula")}
            </button>
            <button type="button" onClick={() => void openLegalDocument("third_party", i18n.language.slice(0, 2))} className={legalButtonClass}>
              <FileText className="h-4 w-4 shrink-0 text-text-secondary" />
              {t("settings.credits.thirdParty")}
            </button>
          </div>
        </section>
      </Card>
    </div>
  );
}
