import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
  info: ErrorInfo | null;
}

/**
 * Last-resort fallback so a render-time crash shows the error instead of
 * silently unmounting to a blank window (React removes the tree on an
 * uncaught error with no boundary above it).
 */
export class ErrorBoundary extends Component<Props, State> {
  override state: State = { error: null, info: null };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    this.setState({ error, info });
    console.error("[AIVT] uncaught render error", error, info);
  }

  override render(): ReactNode {
    const { error, info } = this.state;
    if (!error) return this.props.children;

    return (
      <div
        style={{
          height: "100%",
          overflow: "auto",
          padding: "24px",
          background: "rgb(13 13 17)",
          color: "rgb(237 237 241)",
          fontFamily: "'Segoe UI', system-ui, sans-serif",
        }}
      >
        <h1 style={{ color: "rgb(255 69 58)", fontSize: "18px", marginBottom: "12px" }}>
          Error inesperado en la interfaz
        </h1>
        <pre style={{ whiteSpace: "pre-wrap", fontSize: "13px", lineHeight: 1.5 }}>
          {error.name}: {error.message}
          {"\n\n"}
          {error.stack}
          {info?.componentStack ? `\n\nComponent stack:${info.componentStack}` : ""}
        </pre>
      </div>
    );
  }
}
