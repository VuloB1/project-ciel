import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./locales/en/common.json";
import es from "./locales/es/common.json";
import ja from "./locales/ja/common.json";
import ko from "./locales/ko/common.json";
import pt from "./locales/pt/common.json";
import zh from "./locales/zh/common.json";

export const SUPPORTED_LANGUAGES = ["es", "en", "pt", "ja", "zh", "ko"] as const;
export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

// Nombres nativos — no se traducen (cada idioma se muestra en sí mismo,
// como en cualquier selector de idioma). Compartido entre
// `GeneralSettings.tsx` y el paso de idioma del onboarding.
export const LANGUAGE_LABELS: Record<SupportedLanguage, string> = {
  es: "Español",
  en: "English",
  pt: "Português",
  ja: "日本語",
  zh: "中文",
  ko: "한국어",
};

void i18n.use(initReactI18next).init({
  resources: {
    es: { common: es },
    en: { common: en },
    pt: { common: pt },
    ja: { common: ja },
    zh: { common: zh },
    ko: { common: ko },
  },
  lng: "es",
  fallbackLng: "es",
  defaultNS: "common",
  interpolation: { escapeValue: false },
});

export default i18n;
