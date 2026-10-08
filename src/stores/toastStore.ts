import { create } from "zustand";

export type ToastVariant = "success" | "info" | "warning" | "error";

export interface ToastItem {
  id: string;
  variant: ToastVariant;
  title: string;
  description?: string;
}

interface ToastState {
  toasts: ToastItem[];
  push: (toast: Omit<ToastItem, "id">) => void;
  dismiss: (id: string) => void;
}

/**
 * App-wide toast queue (sección 91 del brief: sistema interno de
 * notificaciones, nunca `alert()`). `Toaster` (design-system/Toast.tsx)
 * renders whatever is in `toasts`; any part of the app pushes onto it via
 * `useToastStore.getState().push(...)`.
 */
export const useToastStore = create<ToastState>((set) => ({
  toasts: [],
  push: (toast) =>
    set((state) => ({
      toasts: [...state.toasts, { ...toast, id: crypto.randomUUID() }],
    })),
  dismiss: (id) => set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) })),
}));
