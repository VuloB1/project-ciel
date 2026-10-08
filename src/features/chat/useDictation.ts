import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "@/lib/errorMessage";
import { useAppEvent } from "@/lib/ipc/events";
import { cancelDictation, startDictation, stopDictation } from "@/lib/ipc/stt";
import { useToastStore } from "@/stores/toastStore";

/** `holding`: el botón sigue pulsado (al soltar se detiene). `locked`: la
 * grabación sigue sin tener el botón pulsado y se termina con "Detener". */
export type DictationPhase = "idle" | "holding" | "locked" | "transcribing";

/** Cuánto hay que deslizar hacia arriba (px) para fijar la grabación. */
const LOCK_DISTANCE = 56;
/** Una pulsación más corta que esto cuenta como "toque": deja la grabación
 * fijada en vez de detenerla al instante. */
const TAP_MS = 350;
/** Ventana tras un `pointerup` en la que se descarta el `click` que lo sigue. */
const IGNORE_CLICK_MS = 400;
/** Mismo tope que el backend (5 minutos) — al llegar se detiene sola. */
const MAX_SECONDS = 300;

interface Options {
  profileId: string;
  disabled: boolean;
  /** Recibe el texto transcrito (nunca vacío). */
  onText: (text: string) => void;
}

export function useDictation({ profileId, disabled, onText }: Options) {
  const { t } = useTranslation();
  const pushToast = useToastStore((s) => s.push);

  const [phase, setPhase] = useState<DictationPhase>("idle");
  const [seconds, setSeconds] = useState(0);
  /** 0..1: cuánto falta para que el deslizamiento fije la grabación. */
  const [lockProgress, setLockProgress] = useState(0);
  /** Texto provisional de lo dicho hasta ahora (se reemplaza entero cada vez). */
  const [partialText, setPartialText] = useState("");

  const phaseRef = useRef<DictationPhase>("idle");
  const startPromiseRef = useRef<Promise<boolean> | null>(null);
  const pointerStartYRef = useRef(0);
  const pointerDownAtRef = useRef(0);
  // El `click` que llega justo después del `pointerup` que fijó la grabación
  // no debe contar como "detener". Caduca a los pocos ms: si el navegador no
  // llega a emitir ese `click`, no debe comerse el siguiente clic de verdad.
  const ignoreClickUntilRef = useRef(0);
  // `true` solo entre el pointerdown que EMPEZÓ la grabación y su pointerup;
  // un pulsado posterior sobre "Detener" no empieza gesto y no debe ignorarse.
  const gestureStartedHereRef = useRef(false);
  const onTextRef = useRef(onText);
  onTextRef.current = onText;

  const updatePhase = useCallback((next: DictationPhase) => {
    phaseRef.current = next;
    setPhase(next);
  }, []);

  /** Pide al backend empezar a grabar; `false` si falló (ya avisó con un toast). */
  const begin = useCallback((): Promise<boolean> => {
    setPartialText("");
    setSeconds(0);
    setLockProgress(0);
    const promise = startDictation()
      .then(() => true)
      .catch((err) => {
        updatePhase("idle");
        pushToast({ variant: "error", title: t("chat.dictation.startError"), description: errorMessage(err) });
        return false;
      });
    startPromiseRef.current = promise;
    return promise;
  }, [pushToast, t, updatePhase]);

  const finish = useCallback(async () => {
    if (phaseRef.current !== "holding" && phaseRef.current !== "locked") return;
    updatePhase("transcribing");
    setLockProgress(0);
    const started = (await startPromiseRef.current) ?? false;
    if (!started) return;
    try {
      const text = (await stopDictation(profileId)).trim();
      if (text) {
        onTextRef.current(text);
      } else {
        pushToast({ variant: "info", title: t("chat.dictation.empty") });
      }
    } catch (err) {
      pushToast({ variant: "error", title: t("chat.dictation.error"), description: errorMessage(err) });
    } finally {
      setPartialText("");
      updatePhase("idle");
    }
  }, [profileId, pushToast, t, updatePhase]);

  const cancel = useCallback(() => {
    if (phaseRef.current !== "holding" && phaseRef.current !== "locked") return;
    updatePhase("idle");
    setLockProgress(0);
    setPartialText("");
    void cancelDictation();
  }, [updatePhase]);

  // Texto en vivo: el backend manda la transcripción provisional cada ~1,5 s.
  useAppEvent((event) => {
    if (event.type === "dictation_partial" && (phaseRef.current === "holding" || phaseRef.current === "locked")) {
      setPartialText(event.text);
    }
  });

  // Cronómetro mientras se graba + tope de duración.
  useEffect(() => {
    if (phase !== "holding" && phase !== "locked") return;
    const startedAt = Date.now();
    const timer = setInterval(() => {
      const elapsed = Math.floor((Date.now() - startedAt) / 1000);
      setSeconds(elapsed);
      if (elapsed >= MAX_SECONDS) void finish();
    }, 250);
    return () => clearInterval(timer);
  }, [phase, finish]);

  // Escape cancela una grabación en curso.
  useEffect(() => {
    if (phase !== "holding" && phase !== "locked") return;
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") cancel();
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [phase, cancel]);

  // Al cerrar el chat o cambiar de conversación no se deja el micrófono abierto.
  useEffect(() => {
    return () => {
      if (phaseRef.current === "holding" || phaseRef.current === "locked") void cancelDictation();
    };
  }, []);

  function onPointerDown(event: ReactPointerEvent<HTMLButtonElement>) {
    if (disabled || phaseRef.current !== "idle" || event.button !== 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    pointerStartYRef.current = event.clientY;
    pointerDownAtRef.current = Date.now();
    gestureStartedHereRef.current = true;
    updatePhase("holding");
    void begin();
  }

  function onPointerMove(event: ReactPointerEvent<HTMLButtonElement>) {
    if (phaseRef.current !== "holding") return;
    const dy = pointerStartYRef.current - event.clientY;
    const progress = Math.max(0, Math.min(1, dy / LOCK_DISTANCE));
    setLockProgress(progress);
    if (progress >= 1) {
      // Fijada: soltar el botón ya no detiene la grabación.
      updatePhase("locked");
      setLockProgress(0);
    }
  }

  function onPointerUp() {
    if (!gestureStartedHereRef.current) return;
    gestureStartedHereRef.current = false;
    if (phaseRef.current === "holding") {
      if (Date.now() - pointerDownAtRef.current < TAP_MS) {
        // Un toque rápido: deja la grabación fijada, manos libres.
        updatePhase("locked");
        setLockProgress(0);
        ignoreClickUntilRef.current = Date.now() + IGNORE_CLICK_MS;
      } else {
        void finish();
        ignoreClickUntilRef.current = Date.now() + IGNORE_CLICK_MS;
      }
    } else if (phaseRef.current === "locked") {
      // Soltó después de deslizar: la grabación sigue.
      ignoreClickUntilRef.current = Date.now() + IGNORE_CLICK_MS;
    }
  }

  function onPointerCancel() {
    gestureStartedHereRef.current = false;
    if (phaseRef.current === "holding") void finish();
  }

  /** `click` real: con teclado (Espacio/Enter) o sobre el botón "Detener". */
  function onClick() {
    if (Date.now() < ignoreClickUntilRef.current) return;
    if (disabled) return;
    if (phaseRef.current === "idle") {
      updatePhase("locked");
      void begin();
    } else if (phaseRef.current === "locked") {
      void finish();
    }
  }

  return {
    phase,
    seconds,
    lockProgress,
    partialText,
    isRecording: phase === "holding" || phase === "locked",
    finish,
    cancel,
    buttonHandlers: { onPointerDown, onPointerMove, onPointerUp, onPointerCancel, onClick },
  };
}
