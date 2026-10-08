import * as SwitchPrimitive from "@radix-ui/react-switch";

import { cn } from "@/lib/theme/cn";

export interface SwitchProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  /** Required — this is the switch's only accessible name. */
  label: string;
}

export function Switch({ checked, onCheckedChange, disabled, label }: SwitchProps) {
  return (
    <SwitchPrimitive.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      aria-label={label}
      className={cn(
        "relative h-6 w-11 shrink-0 rounded-full border border-control bg-surface-secondary transition-colors",
        "data-[state=checked]:bg-accent",
        "disabled:cursor-not-allowed disabled:opacity-50",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background",
      )}
    >
      {/* Unchecked thumb uses `text-primary` instead of a fixed white — that
          token is already chosen per-theme for readability against
          background/surface (e.g. near-black on the light "Paper" preset,
          near-white on every dark preset), so the thumb automatically stays
          dark-on-light or light-on-dark instead of going invisible on a
          light `surfaceSecondary` track (Paper's is literal white). Checked
          keeps `accent-foreground`, which is color-matched to the accent
          itself rather than the theme's overall brightness. */}
      <SwitchPrimitive.Thumb className="block h-5 w-5 translate-x-0.5 rounded-full bg-text-primary shadow transition-transform duration-150 data-[state=checked]:translate-x-[22px] data-[state=checked]:bg-accent-foreground" />
    </SwitchPrimitive.Root>
  );
}
