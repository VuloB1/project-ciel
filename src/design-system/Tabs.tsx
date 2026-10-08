import * as TabsPrimitive from "@radix-ui/react-tabs";
import type { ComponentPropsWithoutRef, ReactNode } from "react";

import { cn } from "@/lib/theme/cn";

export const Tabs = TabsPrimitive.Root;

/** Radix only mounts the active panel, so a fresh `TabsContent` mounts every
 * time the user switches tabs — the same "animate on mount" trick `App.tsx`
 * uses for its own section switch (see `animate-fade-in` in globals.css). */
export function TabsContent({ className, ...props }: ComponentPropsWithoutRef<typeof TabsPrimitive.Content>) {
  return <TabsPrimitive.Content className={cn("animate-fade-in", className)} {...props} />;
}

export function TabsList({ children }: { children: ReactNode }) {
  return <TabsPrimitive.List className="flex gap-1 overflow-x-auto border-b border-border px-4">{children}</TabsPrimitive.List>;
}

export function TabsTrigger({ value, children }: { value: string; children: ReactNode }) {
  return (
    <TabsPrimitive.Trigger
      value={value}
      className={cn(
        "shrink-0 border-b-2 border-transparent px-3 py-2.5 text-sm text-text-secondary transition-colors",
        "hover:text-text-primary",
        "data-[state=active]:border-accent data-[state=active]:text-text-primary",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background",
      )}
    >
      {children}
    </TabsPrimitive.Trigger>
  );
}
