import type { LucideIcon } from "lucide-react";
import type { SVGProps } from "react";

/** Minimalist line-art marks for Twitch/Discord, in the same stroke style
 * as the rest of the app's icon set (lucide-react doesn't ship brand
 * glyphs) — pedido del usuario para que las pestañas de Plataformas se
 * sientan parte de la misma interfaz en vez de destacar con otro estilo. */

/** A lucide icon component isn't structurally the same as one of these
 * (it's a `ForwardRefExoticComponent`, these are plain function
 * components) — anywhere both kinds of icon can appear in the same list
 * (Sidebar's `NAV_GROUPS`, the Command Palette) needs this union instead
 * of `LucideIcon` alone. */
export type IconComponent = LucideIcon | typeof TwitchIcon;

export function TwitchIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      <rect x="4" y="3" width="15" height="14" rx="2" />
      <path d="M8 17v3l3-3" />
      <line x1="10" y1="7" x2="10" y2="11" />
      <line x1="14.5" y1="7" x2="14.5" y2="11" />
    </svg>
  );
}

export function DiscordIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      <path d="M8.5 4.5c-2 .6-3.4 1.6-3.4 1.6-1.6 3-2 7.2-1.7 10.9 0 0 1.6 1.6 4.1 2l.9-1.7" />
      <path d="M15.5 4.5c2 .6 3.4 1.6 3.4 1.6 1.6 3 2 7.2 1.7 10.9 0 0-1.6 1.6-4.1 2l-.9-1.7" />
      <path d="M5.1 6.1c3.2-1.5 10.6-1.5 13.8 0" />
      <ellipse cx="9" cy="13" rx="1.3" ry="1.6" />
      <ellipse cx="15" cy="13" rx="1.3" ry="1.6" />
    </svg>
  );
}
