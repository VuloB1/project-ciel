import * as DropdownMenuPrimitive from "@radix-ui/react-dropdown-menu";
import type { ReactNode } from "react";

import { cn } from "@/lib/theme/cn";

export const DropdownMenu = DropdownMenuPrimitive.Root;
export const DropdownMenuTrigger = DropdownMenuPrimitive.Trigger;

export function DropdownMenuContent({ children, align = "start" }: { children: ReactNode; align?: "start" | "center" | "end" }) {
  return (
    <DropdownMenuPrimitive.Portal>
      <DropdownMenuPrimitive.Content
        align={align}
        sideOffset={6}
        // Con muchas entradas (p. ej. muchos personajes) la lista debe
        // acortarse al espacio libre hasta el borde de la ventana y hacer
        // scroll por dentro, en vez de salirse.
        collisionPadding={8}
        style={{ maxHeight: "var(--radix-dropdown-menu-content-available-height)" }}
        className="animate-pop-in z-50 min-w-[14rem] overflow-y-auto rounded-md border border-border bg-surface-secondary p-1 shadow-md"
      >
        {children}
      </DropdownMenuPrimitive.Content>
    </DropdownMenuPrimitive.Portal>
  );
}

export function DropdownMenuItem({
  children,
  onSelect,
  disabled,
  className,
}: {
  children: ReactNode;
  onSelect?: () => void;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <DropdownMenuPrimitive.Item
      onSelect={onSelect}
      disabled={disabled}
      className={cn(
        "flex cursor-pointer select-none items-center gap-2 rounded-sm px-2.5 py-1.5 text-sm text-text-primary outline-none",
        "data-[highlighted]:bg-surface data-[highlighted]:text-text-primary",
        "data-[disabled]:pointer-events-none data-[disabled]:opacity-50",
        className,
      )}
    >
      {children}
    </DropdownMenuPrimitive.Item>
  );
}

export function DropdownMenuSeparator() {
  return <DropdownMenuPrimitive.Separator className="my-1 h-px bg-border" />;
}

export function DropdownMenuLabel({ children }: { children: ReactNode }) {
  return <DropdownMenuPrimitive.Label className="px-2.5 py-1.5 text-xs text-text-secondary">{children}</DropdownMenuPrimitive.Label>;
}
