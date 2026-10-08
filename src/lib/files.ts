export interface ReadFileResult {
  base64: string;
  extension: string;
  mimeType: string;
}

/** Tope al subir fotos de perfil/usuario (se muestran a ≤128 px) y las
 * imágenes del avatar reactivo (se ven grandes en la página de Avatar/OBS).
 * Un PNG de 1,6 MB viajaba por IPC y se decodificaba en cada burbuja. */
export const AVATAR_MAX_DIMENSION = 512;
export const AVATAR_STATE_MAX_DIMENSION = 1024;

const CANVAS_SUPPORTED_TYPES = new Set(["image/png", "image/jpeg", "image/webp"]);

/** Pure scaling math, kept separate from the canvas/bitmap I/O in
 * `resizeImageToFit` so it's testable without a real browser rendering
 * engine (jsdom has no `createImageBitmap`/canvas support). Never
 * upscales — `maxDimension` larger than both `width` and `height` returns
 * them unchanged. */
export function computeFitDimensions(
  width: number,
  height: number,
  maxDimension: number,
): { width: number; height: number } {
  const scale = Math.min(1, maxDimension / Math.max(width, height));
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  };
}

/** Downscales `file` so neither dimension exceeds `maxDimension`, keeping
 * aspect ratio — chat images get sent to the LLM as base64 in every
 * request that references them, so a 4000px photo would needlessly bloat
 * every request. Never upscales (returns `file` unchanged if it's already
 * within bounds), and skips animated GIFs entirely — redrawing one onto a
 * canvas would flatten it to its first frame, which is a worse trade than
 * just sending the original size for the rare case someone attaches one. */
export async function resizeImageToFit(file: File, maxDimension: number): Promise<File> {
  if (file.type === "image/gif" || !file.type.startsWith("image/")) {
    return file;
  }

  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(file);
  } catch {
    // Some format the browser can't decode into a bitmap — fall back to
    // sending it as-is rather than failing the whole attach flow.
    return file;
  }

  try {
    const { width, height } = bitmap;
    const target = computeFitDimensions(width, height, maxDimension);
    if (target.width === width && target.height === height) {
      return file;
    }

    const canvas = document.createElement("canvas");
    canvas.width = target.width;
    canvas.height = target.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) return file;
    ctx.drawImage(bitmap, 0, 0, canvas.width, canvas.height);

    const mimeType = CANVAS_SUPPORTED_TYPES.has(file.type) ? file.type : "image/jpeg";
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, mimeType, 0.9));
    if (!blob) return file;

    const extension = mimeType.split("/")[1] ?? "jpg";
    const baseName = file.name.replace(/\.[^./\\]+$/, "");
    return new File([blob], `${baseName}.${extension}`, { type: mimeType });
  } finally {
    bitmap.close();
  }
}

/** Reads a `File` (e.g. from `<input type="file">`) as base64, stripping
 * the `data:...;base64,` prefix `FileReader` adds. */
export function readFileAsBase64(file: File): Promise<ReadFileResult> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("No se pudo leer el archivo"));
    reader.onload = () => {
      const result = reader.result;
      if (typeof result !== "string") {
        reject(new Error("Formato de archivo inesperado"));
        return;
      }
      const commaIndex = result.indexOf(",");
      const base64 = commaIndex >= 0 ? result.slice(commaIndex + 1) : result;
      const extension = file.name.split(".").pop() ?? "png";
      resolve({ base64, extension, mimeType: file.type || "application/octet-stream" });
    };
    reader.readAsDataURL(file);
  });
}
