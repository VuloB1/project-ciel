import * as PopoverPrimitive from "@radix-ui/react-popover";
import type { ReactNode } from "react";

import { cn } from "@/lib/theme/cn";

export const Popover = PopoverPrimitive.Root;
export const PopoverTrigger = PopoverPrimitive.Trigger;

/** Unlike `DropdownMenuContent`, this has no `role="menu"`/roving-focus
 * semantics — the right primitive for arbitrary interactive content
 * (sliders, text inputs, drag handles) rather than a list of actions. */
export function PopoverContent({
  children,
  align = "start",
  className,
}: {
  children: ReactNode;
  align?: "start" | "center" | "end";
  className?: string;
}) {
  return (
    <PopoverPrimitive.Portal>
      <PopoverPrimitive.Content
        align={align}
        sideOffset={6}
        // Mismo criterio que `Select`/`DropdownMenuContent`: nunca más alto
        // que el espacio libre hasta el borde de la ventana.
        collisionPadding={8}
        style={{ maxHeight: "var(--radix-popover-content-available-height)" }}
        className={cn(
          "animate-pop-in z-50 overflow-y-auto rounded-md border border-border bg-surface-secondary p-3 shadow-md",
          className,
        )}
      >
        {children}
      </PopoverPrimitive.Content>
    </PopoverPrimitive.Portal>
  );
}
