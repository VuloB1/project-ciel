import * as DialogPrimitive from "@radix-ui/react-dialog";
import type { ReactNode } from "react";

import { Button } from "@/design-system/Button";
import { cn } from "@/lib/theme/cn";

export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;

const DIALOG_SIZES = { md: "max-w-md", lg: "max-w-xl", xl: "max-w-4xl" };

export function DialogContent({ children, size = "md" }: { children: ReactNode; size?: keyof typeof DIALOG_SIZES }) {
  return (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/60" />
      <DialogPrimitive.Content
        className={cn(
          "animate-dialog-pop-in fixed left-1/2 top-1/2 z-50 flex max-h-[calc(100vh-2rem)] w-full -translate-x-1/2 -translate-y-1/2 flex-col rounded-lg border border-border bg-surface p-5 shadow-md focus:outline-none",
          DIALOG_SIZES[size],
        )}
      >
        {children}
      </DialogPrimitive.Content>
    </DialogPrimitive.Portal>
  );
}

export const DialogTitle = DialogPrimitive.Title;
export const DialogDescription = DialogPrimitive.Description;
export const DialogClose = DialogPrimitive.Close;

export interface ConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  confirmLabel: string;
  cancelLabel: string;
  onConfirm: () => void;
  danger?: boolean;
  /** Extra content between the description and the action buttons — e.g.
   * an opt-in toggle for a related action (see `ConversationList`'s
   * "extraer memorias antes de eliminar"). Omit for a plain confirm. */
  children?: ReactNode;
}

/** A11y-friendly stand-in for `window.confirm()` (sección 91 del brief:
 * nunca diálogos nativos del navegador para la UI de la app). */
export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel,
  cancelLabel,
  onConfirm,
  danger,
  children,
}: ConfirmDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogTitle className="text-base font-medium text-text-primary">{title}</DialogTitle>
        <DialogDescription className="mt-1.5 text-sm text-text-secondary">{description}</DialogDescription>
        {children ? <div className="mt-4">{children}</div> : null}
        <div className="mt-5 flex justify-end gap-2">
          <DialogClose asChild>
            <Button variant="ghost">{cancelLabel}</Button>
          </DialogClose>
          <Button
            variant={danger ? "danger" : "primary"}
            onClick={() => {
              onConfirm();
              onOpenChange(false);
            }}
          >
            {confirmLabel}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
