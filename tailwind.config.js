/** @type {import('tailwindcss').Config} */
export default {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        background: "rgb(var(--color-background) / <alpha-value>)",
        surface: "rgb(var(--color-surface) / <alpha-value>)",
        "surface-secondary": "rgb(var(--color-surface-secondary) / <alpha-value>)",
        border: "rgb(var(--color-border) / <alpha-value>)",
        control: "rgb(var(--color-control-border) / <alpha-value>)",
        "text-primary": "rgb(var(--color-text-primary) / <alpha-value>)",
        "text-secondary": "rgb(var(--color-text-secondary) / <alpha-value>)",
        accent: "rgb(var(--color-accent) / <alpha-value>)",
        "accent-secondary": "rgb(var(--color-accent-secondary) / <alpha-value>)",
        "accent-foreground": "rgb(var(--color-accent-foreground) / <alpha-value>)",
        success: "rgb(var(--color-success) / <alpha-value>)",
        warning: "rgb(var(--color-warning) / <alpha-value>)",
        error: "rgb(var(--color-error) / <alpha-value>)",
        "error-foreground": "rgb(var(--color-error-foreground) / <alpha-value>)",
      },
      borderRadius: {
        sm: "var(--radius-sm)",
        md: "var(--radius-md)",
        lg: "var(--radius-lg)",
      },
      fontFamily: {
        sans: ["var(--font-family-app)", "Inter", "Segoe UI", "system-ui", "sans-serif"],
      },
      // Escalado por "Tamaño de fuente" (Apariencia > Preferencias de la
      // app) — solo el TEXTO cambia con --font-scale, no el spacing/iconos
      // (que en Tailwind también son rem), a propósito: escalar el
      // font-size del <html> en cambio haría que un 140% infle paddings e
      // íconos junto con el texto y rompiera layouts. Mismos valores base
      // que el default de Tailwind, multiplicados por la variable.
      fontSize: {
        xs: ["calc(0.75rem * var(--font-scale, 1))", { lineHeight: "calc(1rem * var(--font-scale, 1))" }],
        sm: ["calc(0.875rem * var(--font-scale, 1))", { lineHeight: "calc(1.25rem * var(--font-scale, 1))" }],
        base: ["calc(1rem * var(--font-scale, 1))", { lineHeight: "calc(1.5rem * var(--font-scale, 1))" }],
        lg: ["calc(1.125rem * var(--font-scale, 1))", { lineHeight: "calc(1.75rem * var(--font-scale, 1))" }],
        xl: ["calc(1.25rem * var(--font-scale, 1))", { lineHeight: "calc(1.75rem * var(--font-scale, 1))" }],
        "2xl": ["calc(1.5rem * var(--font-scale, 1))", { lineHeight: "calc(2rem * var(--font-scale, 1))" }],
      },
    },
  },
  plugins: [],
};
