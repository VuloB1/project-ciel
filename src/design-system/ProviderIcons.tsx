import type { SVGProps } from "react";

/** Minimalist line-art marks for los proveedores de IA sugeridos, en el
 * mismo estilo de trazo que `PlatformIcons.tsx` (Twitch/Discord) — formas
 * abstractas propias, no una reproducción de cada logo real, para que la
 * lista de proveedores sea reconocible de un vistazo sin depender de texto. */

export function OpenAiIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round" {...props}>
      <circle cx="12" cy="12" r="2" />
      <circle cx="12" cy="6" r="2" />
      <circle cx="16.9" cy="9" r="2" />
      <circle cx="16.9" cy="15" r="2" />
      <circle cx="12" cy="18" r="2" />
      <circle cx="7.1" cy="15" r="2" />
      <circle cx="7.1" cy="9" r="2" />
    </svg>
  );
}

export function AnthropicIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" {...props}>
      <line x1="12" y1="2.5" x2="12" y2="7.5" />
      <line x1="12" y1="16.5" x2="12" y2="21.5" />
      <line x1="2.5" y1="12" x2="7.5" y2="12" />
      <line x1="16.5" y1="12" x2="21.5" y2="12" />
      <line x1="5.5" y1="5.5" x2="9" y2="9" />
      <line x1="15" y1="15" x2="18.5" y2="18.5" />
      <line x1="18.5" y1="5.5" x2="15" y2="9" />
      <line x1="9" y1="15" x2="5.5" y2="18.5" />
    </svg>
  );
}

export function GeminiIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round" {...props}>
      <rect x="5.5" y="5.5" width="7" height="7" rx="1.5" transform="rotate(45 9 9)" />
      <rect x="11.5" y="11.5" width="7" height="7" rx="1.5" transform="rotate(45 15 15)" />
    </svg>
  );
}

/** Barras de un ecualizador de audio — motivo abstracto de "voz/sonido",
 * no el wordmark real de ElevenLabs. */
export function ElevenLabsIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round" {...props}>
      <line x1="4" y1="10" x2="4" y2="14" />
      <line x1="8.5" y1="6" x2="8.5" y2="18" />
      <line x1="12" y1="3" x2="12" y2="21" />
      <line x1="15.5" y1="6" x2="15.5" y2="18" />
      <line x1="20" y1="10" x2="20" y2="14" />
    </svg>
  );
}

/** Cuatro puntos en grilla — forma abstracta propia, no el logo real de
 * Google (que además usa color, algo que este set de íconos evita). */
export function GoogleIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round" {...props}>
      <circle cx="8" cy="8" r="2.3" />
      <circle cx="16" cy="8" r="2.3" />
      <circle cx="8" cy="16" r="2.3" />
      <circle cx="16" cy="16" r="2.3" />
    </svg>
  );
}

/** Nube genérica — Azure Speech es un servicio en la nube; se evita a
 * propósito la cinta angular real del logo de Azure. */
export function AzureIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round" {...props}>
      <path d="M7 18h10a4 4 0 0 0 .5-7.97A5.5 5.5 0 0 0 7.16 8.05 4.5 4.5 0 0 0 7 18Z" />
    </svg>
  );
}
