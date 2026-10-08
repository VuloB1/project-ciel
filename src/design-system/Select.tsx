import * as SelectPrimitive from "@radix-ui/react-select";
import { Check, ChevronDown } from "lucide-react";

export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectProps {
  value: string;
  onValueChange: (value: string) => void;
  options: SelectOption[];
  disabled?: boolean;
  ariaLabel: string;
}

export function Select({ value, onValueChange, options, disabled, ariaLabel }: SelectProps) {
  return (
    <SelectPrimitive.Root value={value} onValueChange={onValueChange} disabled={disabled}>
      <SelectPrimitive.Trigger
        aria-label={ariaLabel}
        className="inline-flex h-9 min-w-[10rem] items-center justify-between gap-2 rounded-md border border-control bg-surface px-3 text-sm text-text-primary transition-colors hover:bg-surface-secondary disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
      >
        <SelectPrimitive.Value />
        <SelectPrimitive.Icon>
          <ChevronDown className="h-4 w-4 text-text-secondary" />
        </SelectPrimitive.Icon>
      </SelectPrimitive.Trigger>
      <SelectPrimitive.Portal>
        <SelectPrimitive.Content
          position="popper"
          sideOffset={4}
          // Sin esto la lista crece con sus opciones y se sale de la ventana
          // (p. ej. los cientos de voces de Fish Audio): Radix expone el alto
          // libre hasta el borde en esta variable; el Viewport hace el scroll.
          collisionPadding={8}
          style={{ maxHeight: "var(--radix-select-content-available-height)" }}
          className="animate-pop-in z-50 overflow-hidden rounded-md border border-border bg-surface-secondary shadow-md"
        >
          <SelectPrimitive.Viewport className="max-h-[inherit] overflow-y-auto p-1">
            {options.map((option) => (
              <SelectPrimitive.Item
                key={option.value}
                value={option.value}
                className="relative flex cursor-pointer select-none items-center rounded-sm px-6 py-1.5 text-sm text-text-primary outline-none data-[highlighted]:bg-surface"
              >
                <SelectPrimitive.ItemIndicator className="absolute left-1.5 inline-flex items-center">
                  <Check className="h-3.5 w-3.5 text-accent" />
                </SelectPrimitive.ItemIndicator>
                <SelectPrimitive.ItemText>{option.label}</SelectPrimitive.ItemText>
              </SelectPrimitive.Item>
            ))}
          </SelectPrimitive.Viewport>
        </SelectPrimitive.Content>
      </SelectPrimitive.Portal>
    </SelectPrimitive.Root>
  );
}
