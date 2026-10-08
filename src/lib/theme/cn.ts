type ClassValue = string | number | null | undefined | false;

/**
 * Minimal `clsx`-style class joiner. Not pulled in as a dependency since
 * this is all the design system needs — filters falsy values and joins
 * the rest with a space.
 */
export function cn(...values: ClassValue[]): string {
  return values.filter(Boolean).join(" ");
}
