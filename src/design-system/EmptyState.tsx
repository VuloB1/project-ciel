import type { ReactNode } from "react";

export interface EmptyStateProps {
  icon: ReactNode;
  title: string;
  description?: string;
  action?: ReactNode;
}

export function EmptyState({ icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-8 text-center">
      <div className="flex h-12 w-12 items-center justify-center rounded-full bg-surface-secondary text-text-secondary">
        {icon}
      </div>
      <h2 className="text-base font-medium text-text-primary">{title}</h2>
      {description ? <p className="max-w-sm text-sm text-text-secondary">{description}</p> : null}
      {action}
    </div>
  );
}
