import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";

import { cn } from "@/lib/theme/cn";

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon: ReactNode;
  /** Required — this is the button's only accessible name. */
  label: string;
  active?: boolean;
  /** Esquinas con el radio grande del tema (el mismo que las burbujas del chat),
   * que sigue el ajuste de redondez — no un círculo fijo. */
  round?: boolean;
  /** `lg` = 40 px (junto a botones grandes, como los del cuadro de envío). */
  size?: "md" | "lg";
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  ({ icon, label, active, round, size = "md", className, ...props }, ref) => (
    <button
      ref={ref}
      aria-label={label}
      className={cn(
        size === "lg" ? "h-10 w-10" : "h-9 w-9",
        round ? "rounded-lg" : "rounded-md",
        "inline-flex shrink-0 items-center justify-center text-text-secondary transition duration-150",
        "hover:bg-surface-secondary hover:text-text-primary active:scale-90",
        "disabled:cursor-not-allowed disabled:opacity-50",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
        active && "bg-surface-secondary text-text-primary",
        className,
      )}
      {...props}
    >
      {icon}
    </button>
  ),
);
IconButton.displayName = "IconButton";
