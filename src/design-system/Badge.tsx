import type { ReactNode } from "react";

import { cn } from "@/lib/theme/cn";

export type BadgeVariant = "neutral" | "success" | "warning" | "error";

const variantClasses: Record<BadgeVariant, string> = {
  neutral: "bg-surface-secondary text-text-secondary",
  success: "bg-success/10 text-success",
  warning: "bg-warning/10 text-warning",
  error: "bg-error/10 text-error",
};

export function Badge({
  variant = "neutral",
  className,
  children,
}: {
  variant?: BadgeVariant;
  className?: string;
  children: ReactNode;
}) {
  return (
    <span className={cn("inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium", variantClasses[variant], className)}>
      {children}
    </span>
  );
}
