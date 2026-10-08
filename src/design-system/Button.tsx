import { forwardRef, type ButtonHTMLAttributes } from "react";

import { cn } from "@/lib/theme/cn";

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
type ButtonSize = "sm" | "md";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

const variantClasses: Record<ButtonVariant, string> = {
  primary: "bg-accent text-accent-foreground hover:bg-accent-secondary",
  secondary: "bg-surface-secondary text-text-primary hover:brightness-110",
  ghost: "bg-transparent text-text-secondary hover:bg-surface-secondary hover:text-text-primary",
  danger: "bg-error text-error-foreground hover:opacity-90",
};

const sizeClasses: Record<ButtonSize, string> = {
  sm: "h-8 px-3 text-sm",
  md: "h-10 px-4 text-sm",
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant = "primary", size = "md", ...props }, ref) => (
    <button
      ref={ref}
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-md font-medium transition duration-150",
        "active:scale-[0.97]",
        "disabled:cursor-not-allowed disabled:opacity-50",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background",
        variantClasses[variant],
        sizeClasses[size],
        className,
      )}
      {...props}
    />
  ),
);
Button.displayName = "Button";
