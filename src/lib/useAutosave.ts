import { useEffect, useRef, useState } from "react";

/** How long to wait after the last change before persisting — long enough
 * that dragging a `Slider` (which fires `onValueChange` continuously) only
 * saves once, on release, short enough that switching tabs right after an
 * edit is the only realistic way to still have a pending save (handled by
 * the unmount flush below, not by this timeout). */
const DEFAULT_DEBOUNCE_MS = 700;

/** Wraps the pending value so `T` itself can be anything (including
 * `null`/`undefined`) without ambiguity over whether a save is pending. */
type Pending<T> = { value: T } | null;

/**
 * Autosaves `value` a fixed delay after it stops changing — the shared
 * mechanism behind Identidad/Voz/Proveedores IA/TTS's move away from an
 * explicit "Guardar" button (see docs/DEVELOPMENT_STATUS.md, limpieza de
 * guardado post-Fase 11), matching the silent per-field autosave already
 * used by Llamada/Twitch/Discord/Apariencia. `save` should persist the
 * *whole* value the same way an explicit Save button already did — there's
 * no partial-patch concept in any of the IPC calls this wraps.
 *
 * Pass `enabled: false` while a screen's own initial `fetch().then(setX)`
 * hasn't landed yet (every caller today loads its draft async on mount) —
 * every caller of this hook has that shape, so this isn't optional in
 * practice. Never fires for the value in place at the moment `enabled`
 * turns (or starts) true — that value just came from the backend, so
 * saving it back would be a pointless round-trip. Any edit still pending
 * when the component unmounts (e.g. the user switches away from this tab
 * inside the debounce window) is flushed immediately instead of dropped.
 */
export function useAutosave<T>(
  value: T,
  save: (value: T) => Promise<void>,
  options: { debounceMs?: number; enabled?: boolean; onError: (err: unknown) => void },
): { isSaving: boolean; flush: () => void } {
  const { debounceMs = DEFAULT_DEBOUNCE_MS, enabled = true, onError } = options;

  const [isSaving, setIsSaving] = useState(false);
  const saveRef = useRef(save);
  saveRef.current = save;
  const onErrorRef = useRef(onError);
  onErrorRef.current = onError;

  const pendingRef = useRef<Pending<T>>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  /** Whether the *previous* effect run was enabled — a false→true edge
   * (including the very first run, since this starts `false`) means
   * `value` is a freshly loaded baseline, not an edit to save. */
  const wasEnabledRef = useRef(false);

  function runSave(next: T) {
    pendingRef.current = null;
    setIsSaving(true);
    saveRef
      .current(next)
      .catch((err) => onErrorRef.current(err))
      .finally(() => setIsSaving(false));
  }

  function flush() {
    clearTimeout(timeoutRef.current);
    const pending = pendingRef.current;
    if (pending) runSave(pending.value);
  }

  useEffect(() => {
    if (!enabled) {
      wasEnabledRef.current = false;
      pendingRef.current = null;
      clearTimeout(timeoutRef.current);
      return;
    }

    const justEnabled = !wasEnabledRef.current;
    wasEnabledRef.current = true;
    if (justEnabled) {
      // `value` is the freshly loaded baseline, not an edit.
      pendingRef.current = null;
      clearTimeout(timeoutRef.current);
      return;
    }

    pendingRef.current = { value };
    clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => {
      const pending = pendingRef.current;
      if (pending) runSave(pending.value);
    }, debounceMs);

    return () => clearTimeout(timeoutRef.current);
    // `debounceMs` is a stable option, not something callers vary per
    // render, so it deliberately isn't a dependency here.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value, enabled]);

  // Flush on unmount so a still-pending edit isn't lost by navigating away
  // right before the debounce would have fired on its own.
  useEffect(() => {
    return () => {
      clearTimeout(timeoutRef.current);
      const pending = pendingRef.current;
      if (pending) {
        pendingRef.current = null;
        void saveRef.current(pending.value).catch((err) => onErrorRef.current(err));
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { isSaving, flush };
}
