/** Every Rust command failure crosses IPC as `aivt_core::AppError`
 * (`{ module, code, message }`, see crates/core/src/error.rs) — Tauri
 * rejects the invoke() promise with that plain object, not a JS `Error`,
 * so `err.message` (or `String(err)`, which only ever produced the
 * useless "[object Object]") never worked for it. Toasts across the app
 * were silently swallowing the real reason a command failed until this
 * was fixed. */
export function errorMessage(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "object" && err !== null && "message" in err && typeof (err as { message: unknown }).message === "string") {
    return (err as { message: string }).message;
  }
  return String(err);
}
