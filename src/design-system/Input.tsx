import { forwardRef, type InputHTMLAttributes } from "react";

import { cn } from "@/lib/theme/cn";

export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  ({ className, ...props }, ref) => (
    <input
      ref={ref}
      className={cn(
        "h-9 w-full rounded-md border border-control bg-surface px-3 text-sm text-text-primary placeholder:text-text-secondary",
        "transition-colors hover:bg-surface-secondary focus-visible:bg-surface-secondary",
        "disabled:cursor-not-allowed disabled:opacity-50",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
        className,
      )}
      {...props}
    />
  ),
);
Input.displayName = "Input";
