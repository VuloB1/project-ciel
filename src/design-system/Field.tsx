import type { ReactNode } from "react";

export interface FieldProps {
  /** Omitido cuando el texto de arriba (título de sección, etc.) ya deja
   * claro qué es este control — ver TtsSettings.tsx's selector de motor.
   * El nombre accesible en ese caso lo sigue dando el `ariaLabel` del
   * control hijo (`Select`/`Input`), no este `label`. */
  label?: string;
  hint?: string;
  className?: string;
  children: ReactNode;
}

export function Field({ label, hint, className, children }: FieldProps) {
  return (
    <div className={className}>
      {label ? <label className="mb-1.5 block text-sm font-medium text-text-primary">{label}</label> : null}
      {children}
      {hint ? <p className="mt-1 text-xs text-text-secondary">{hint}</p> : null}
    </div>
  );
}
