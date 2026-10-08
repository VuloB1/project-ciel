import * as ToastPrimitive from "@radix-ui/react-toast";
import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "lucide-react";
import type { ComponentType } from "react";

import { cn } from "@/lib/theme/cn";
import { useToastStore, type ToastVariant } from "@/stores/toastStore";

const variantIcon: Record<ToastVariant, ComponentType<{ className?: string }>> = {
  success: CheckCircle2,
  info: Info,
  warning: AlertTriangle,
  error: XCircle,
};

const variantColor: Record<ToastVariant, string> = {
  success: "text-success",
  info: "text-accent",
  warning: "text-warning",
  error: "text-error",
};

export function Toaster() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  return (
    <ToastPrimitive.Provider swipeDirection="right" duration={5000}>
      {toasts.map((toast) => {
        const Icon = variantIcon[toast.variant];
        return (
          <ToastPrimitive.Root
            key={toast.id}
            className="animate-pop-in flex items-start gap-3 rounded-md border border-border bg-surface-secondary p-3 shadow-md"
            onOpenChange={(open) => {
              if (!open) dismiss(toast.id);
            }}
          >
            <Icon className={cn("mt-0.5 h-4 w-4 shrink-0", variantColor[toast.variant])} />
            <div className="flex-1">
              <ToastPrimitive.Title className="text-sm font-medium text-text-primary">
                {toast.title}
              </ToastPrimitive.Title>
              {toast.description ? (
                <ToastPrimitive.Description className="mt-0.5 text-xs text-text-secondary">
                  {toast.description}
                </ToastPrimitive.Description>
              ) : null}
            </div>
            <ToastPrimitive.Close aria-label="Cerrar" className="text-text-secondary hover:text-text-primary">
              <X className="h-4 w-4" />
            </ToastPrimitive.Close>
          </ToastPrimitive.Root>
        );
      })}
      <ToastPrimitive.Viewport className="fixed bottom-4 right-4 z-50 flex w-80 flex-col gap-2 outline-none" />
    </ToastPrimitive.Provider>
  );
}
