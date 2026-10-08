import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";

import { Input } from "@/design-system/Input";
import { Popover, PopoverContent, PopoverTrigger } from "@/design-system/Popover";
import { cn } from "@/lib/theme/cn";

interface Rgb {
  r: number;
  g: number;
  b: number;
}

interface Hsv {
  h: number; // 0-360
  s: number; // 0-1
  v: number; // 0-1
}

const SV_WIDTH = 200;
const SV_HEIGHT = 120;

function clamp01(n: number): number {
  return Math.min(1, Math.max(0, n));
}

function normalizeHex(hex: string): string | null {
  const cleaned = hex.trim().replace(/^#/, "");
  const expanded = cleaned.length === 3 ? cleaned.split("").map((c) => c + c).join("") : cleaned;
  return /^[0-9a-fA-F]{6}$/.test(expanded) ? `#${expanded.toLowerCase()}` : null;
}

function hexToRgb(hex: string): Rgb {
  const normalized = normalizeHex(hex) ?? "#000000";
  const value = normalized.slice(1);
  return {
    r: parseInt(value.slice(0, 2), 16),
    g: parseInt(value.slice(2, 4), 16),
    b: parseInt(value.slice(4, 6), 16),
  };
}

function rgbToHex({ r, g, b }: Rgb): string {
  const toHex = (n: number) => Math.round(clamp01(n / 255) * 255).toString(16).padStart(2, "0");
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

function rgbToHsv({ r, g, b }: Rgb): Hsv {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const d = max - min;
  let h = 0;
  if (d !== 0) {
    switch (max) {
      case rn:
        h = ((gn - bn) / d) % 6;
        break;
      case gn:
        h = (bn - rn) / d + 2;
        break;
      default:
        h = (rn - gn) / d + 4;
    }
    h *= 60;
    if (h < 0) h += 360;
  }
  return { h, s: max === 0 ? 0 : d / max, v: max };
}

function hsvToRgb({ h, s, v }: Hsv): Rgb {
  const c = v * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = v - c;
  let rp = 0;
  let gp = 0;
  let bp = 0;
  if (h < 60) {
    rp = c;
    gp = x;
  } else if (h < 120) {
    rp = x;
    gp = c;
  } else if (h < 180) {
    gp = c;
    bp = x;
  } else if (h < 240) {
    gp = x;
    bp = c;
  } else if (h < 300) {
    rp = x;
    bp = c;
  } else {
    rp = c;
    bp = x;
  }
  return { r: (rp + m) * 255, g: (gp + m) * 255, b: (bp + m) * 255 };
}

function hexToHsv(hex: string): Hsv {
  return rgbToHsv(hexToRgb(hex));
}

/** Perceived brightness (YIQ, 0-255) — same formula as
 * `accentTheme.ts`'s `perceivedBrightness`, duplicated locally since that
 * one isn't exported (a different module, computing a different thing:
 * theme foreground color, not a drag-handle ring). Used to keep the pad
 * and hue-slider handles visible against whatever color they're currently
 * sitting on — a plain white ring disappears against a near-white pick. */
function isLightColor({ r, g, b }: Rgb): boolean {
  return (r * 299 + g * 587 + b * 114) / 1000 >= 128;
}

/**
 * Custom HSV color picker — replaces the native `<input type="color">`
 * (a bare Chromium/OS popup we have zero styling control over, and can't
 * make coherent with the rest of the app) with a small trigger swatch plus
 * a popover panel (saturation/value pad + hue slider + hex field) built
 * from our own design tokens.
 *
 * Dragging the pad or the hue slider only updates LOCAL state for a live
 * preview inside the popover — `onChange` (and whatever expensive work the
 * caller does with it, e.g. persisting + recomputing the whole app's theme
 * CSS variables) fires only once, on pointer release. Calling it on every
 * drag tick was the actual cause of the "se lagea al arrastrar" the user
 * reported: every intermediate color repainted every themed element in the
 * app, not just this control.
 */
export function ColorPicker({ value, onChange, className }: { value: string; onChange: (hex: string) => void; className?: string }) {
  const [open, setOpen] = useState(false);
  const [hsv, setHsv] = useState<Hsv>(() => hexToHsv(value));
  const [hexDraft, setHexDraft] = useState(value);
  const svRectRef = useRef<DOMRect | null>(null);
  const hueRectRef = useRef<DOMRect | null>(null);

  // Re-sync from the committed value whenever the popover (re)opens — e.g.
  // another control (a theme preset) changed the color while this was
  // closed. Never resyncs mid-drag: dragging only ever happens while open,
  // and this only runs on the `open` transition, not on every `value`
  // change during that drag (this effect isn't in `onChange`'s call path).
  useEffect(() => {
    if (open) {
      setHsv(hexToHsv(value));
      setHexDraft(value);
    }
  }, [open, value]);

  const previewHex = rgbToHex(hsvToRgb(hsv));
  // The pad handle always sits on the currently-drafted color itself — its
  // local backdrop IS `previewHex`. The hue handle's local backdrop is
  // always the pure, fully-saturated hue at that slider position (e.g.
  // bright yellow around h=60), regardless of the pad's current s/v.
  const svHandleIsLight = isLightColor(hsvToRgb(hsv));
  const hueHandleIsLight = isLightColor(hsvToRgb({ h: hsv.h, s: 1, v: 1 }));

  function commitHsv(next: Hsv) {
    const hex = rgbToHex(hsvToRgb(next));
    setHexDraft(hex);
    onChange(hex);
  }

  function svPoint(event: ReactPointerEvent<HTMLDivElement> | PointerEvent, rect: DOMRect) {
    return {
      s: clamp01((event.clientX - rect.left) / rect.width),
      v: clamp01(1 - (event.clientY - rect.top) / rect.height),
    };
  }

  function handleSvPointerDown(event: ReactPointerEvent<HTMLDivElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    svRectRef.current = rect;
    event.currentTarget.setPointerCapture(event.pointerId);
    const { s, v } = svPoint(event, rect);
    setHsv((prev) => ({ ...prev, s, v }));
  }

  function handleSvPointerMove(event: ReactPointerEvent<HTMLDivElement>) {
    const rect = svRectRef.current;
    if (!rect) return;
    const { s, v } = svPoint(event, rect);
    setHsv((prev) => ({ ...prev, s, v }));
  }

  function handleSvPointerUp(event: ReactPointerEvent<HTMLDivElement>) {
    const rect = svRectRef.current;
    // Cleared regardless of whether `rect` was set — without this, the pad
    // kept tracking the cursor on every hover (no button held) after the
    // first real drag, since `handleSvPointerMove` only gates on this ref
    // being non-null, and it was never reset once a drag had happened once.
    svRectRef.current = null;
    if (!rect) return;
    const { s, v } = svPoint(event, rect);
    // `commitHsv` has side effects (calls `onChange`, which persists and
    // repaints the whole app's theme) — it must NOT run inside a `setState`
    // updater function. React may invoke an updater more than once
    // (StrictMode does, deliberately, to catch exactly this kind of
    // impurity) and did: this used to double-fire the commit on every
    // release. `next` is computed directly instead, from the `hsv` already
    // in scope (unaffected by an SV-pad drag) plus the fresh pointer
    // coordinates, so `setHsv`/`commitHsv` are two independent, single-call
    // statements.
    const next = { ...hsv, s, v };
    setHsv(next);
    commitHsv(next);
  }

  function hueFraction(event: ReactPointerEvent<HTMLDivElement> | PointerEvent, rect: DOMRect) {
    return clamp01((event.clientX - rect.left) / rect.width) * 360;
  }

  function handleHuePointerDown(event: ReactPointerEvent<HTMLDivElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    hueRectRef.current = rect;
    event.currentTarget.setPointerCapture(event.pointerId);
    const h = hueFraction(event, rect);
    setHsv((prev) => ({ ...prev, h }));
  }

  function handleHuePointerMove(event: ReactPointerEvent<HTMLDivElement>) {
    const rect = hueRectRef.current;
    if (!rect) return;
    const h = hueFraction(event, rect);
    setHsv((prev) => ({ ...prev, h }));
  }

  function handleHuePointerUp(event: ReactPointerEvent<HTMLDivElement>) {
    const rect = hueRectRef.current;
    hueRectRef.current = null; // same reasoning as `handleSvPointerUp`'s reset
    if (!rect) return;
    const h = hueFraction(event, rect);
    // Same reasoning as `handleSvPointerUp` above.
    const next = { ...hsv, h };
    setHsv(next);
    commitHsv(next);
  }

  function commitHexDraft() {
    const normalized = normalizeHex(hexDraft);
    if (!normalized) {
      setHexDraft(value); // invalid — revert instead of committing garbage
      return;
    }
    setHsv(hexToHsv(normalized));
    setHexDraft(normalized);
    onChange(normalized);
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className={cn(
            "h-8 w-8 shrink-0 rounded-md border-2 border-border transition active:scale-90",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
            className,
          )}
          style={{ backgroundColor: value }}
          aria-label={value}
        />
      </PopoverTrigger>
      <PopoverContent align="end" className="flex flex-col gap-3">
        {/* Square corners on purpose — a rounded corner here would clip the
            most saturated/extreme colors (pure white, pure black, pure
            hue), making them awkward or impossible to click exactly. */}
        <div
          className="relative touch-none select-none border border-border"
          style={{
            width: SV_WIDTH,
            height: SV_HEIGHT,
            backgroundColor: `hsl(${hsv.h}, 100%, 50%)`,
            backgroundImage:
              "linear-gradient(to top, #000, rgba(0,0,0,0)), linear-gradient(to right, #fff, rgba(255,255,255,0))",
          }}
          onPointerDown={handleSvPointerDown}
          onPointerMove={handleSvPointerMove}
          onPointerUp={handleSvPointerUp}
          onPointerCancel={handleSvPointerUp}
        >
          <div
            className={cn(
              "pointer-events-none absolute h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 shadow",
              svHandleIsLight ? "border-black" : "border-white",
            )}
            style={{ left: `${hsv.s * 100}%`, top: `${(1 - hsv.v) * 100}%` }}
          />
        </div>

        <div
          className="relative h-3 touch-none select-none rounded-full"
          style={{ background: "linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00)" }}
          onPointerDown={handleHuePointerDown}
          onPointerMove={handleHuePointerMove}
          onPointerUp={handleHuePointerUp}
          onPointerCancel={handleHuePointerUp}
        >
          <div
            className={cn(
              "pointer-events-none absolute top-1/2 h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 shadow",
              hueHandleIsLight ? "border-black" : "border-white",
            )}
            style={{ left: `${(hsv.h / 360) * 100}%`, backgroundColor: `hsl(${hsv.h}, 100%, 50%)` }}
          />
        </div>

        <div className="flex items-center gap-2">
          <div
            className="h-8 w-8 shrink-0 rounded-md border-2 border-border"
            style={{ backgroundColor: previewHex }}
          />
          <Input
            value={hexDraft}
            onChange={(event) => setHexDraft(event.target.value)}
            onBlur={commitHexDraft}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
            }}
            maxLength={7}
            className="h-8 flex-1 font-mono text-xs uppercase"
          />
        </div>
      </PopoverContent>
    </Popover>
  );
}
