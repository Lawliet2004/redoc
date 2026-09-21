import { ErrorBoundary as SolidErrorBoundary, ParentProps } from "solid-js";
import { Button } from "./Button";

interface ErrorBoundaryProps {
  fallback?: (error: Error, reset: () => void) => any;
  onError?: (error: Error) => void;
}

export function ErrorBoundary(props: ParentProps<ErrorBoundaryProps>) {
  return (
    <SolidErrorBoundary
      fallback={(err, reset) => {
        const error = err instanceof Error ? err : new Error(String(err));
        props.onError?.(error);
        return props.fallback ? (
          props.fallback(error, reset)
        ) : (
          <div
            role="alert"
            style={{
              display: "flex",
              "flex-direction": "column",
              "align-items": "center",
              "justify-content": "center",
              gap: "var(--space-4, 16px)",
              padding: "var(--space-8, 32px)",
              "text-align": "center",
              "min-height": "200px",
            }}
          >
            <div
              style={{
                "font-size": "var(--font-3xl, 30px)",
                color: "var(--text-secondary)",
              }}
            >
              ⚠️
            </div>
            <div
              style={{
                "font-size": "var(--font-lg, 16px)",
                "font-weight": "600",
                color: "var(--text-primary)",
              }}
            >
              Something went wrong
            </div>
            <div
              style={{
                "font-size": "var(--font-sm, 13px)",
                color: "var(--text-secondary)",
                "max-width": "400px",
                "word-break": "break-word",
              }}
            >
              {error.message || "An unexpected error occurred"}
            </div>
            <div style={{ display: "flex", gap: "var(--space-2, 8px)", "margin-top": "var(--space-2, 8px)" }}>
              <Button variant="primary" onClick={reset}>
                Try Again
              </Button>
              <Button variant="secondary" onClick={() => window.location.reload()}>
                Reload App
              </Button>
            </div>
          </div>
        );
      }}
    >
      {props.children}
    </SolidErrorBoundary>
  );
}
