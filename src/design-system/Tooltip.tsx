import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ReactNode } from "react";

export function TooltipProvider({ children }: { children: ReactNode }) {
  return <TooltipPrimitive.Provider delayDuration={300}>{children}</TooltipPrimitive.Provider>;
}

export function Tooltip({ content, children }: { content: ReactNode; children: ReactNode }) {
  return (
    <TooltipPrimitive.Root>
      <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
      <TooltipPrimitive.Portal>
        <TooltipPrimitive.Content
          side="right"
          sideOffset={8}
          className="animate-pop-in z-50 rounded-md border border-border bg-surface-secondary px-2.5 py-1.5 text-xs text-text-primary shadow-md"
        >
          {content}
          <TooltipPrimitive.Arrow className="fill-surface-secondary" />
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Portal>
    </TooltipPrimitive.Root>
  );
}
