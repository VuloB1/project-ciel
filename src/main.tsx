import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@/lib/i18n";
import "@/lib/theme/fonts";
import "./globals.css";

import { ErrorBoundary } from "./app/ErrorBoundary";
import { App } from "./App";

const rootElement = document.getElementById("root");
if (!rootElement) {
  throw new Error("Root element not found");
}

createRoot(rootElement).render(
  <StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </StrictMode>,
);
