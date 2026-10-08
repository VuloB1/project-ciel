import { Loader2 } from "lucide-react";

export function LoadingState({ label }: { label?: string }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 text-text-secondary">
      <Loader2 className="h-5 w-5 animate-spin" />
      {label ? <p className="text-sm">{label}</p> : null}
    </div>
  );
}
