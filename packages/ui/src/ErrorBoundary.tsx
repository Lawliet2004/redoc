import { ParentProps, Show, createSignal, onMount } from "solid-js";
import { Button } from "./Button";

interface ErrorBoundaryProps {
  fallback?: (error: Error, reset: () => void) => any;
  onError?: (error: Error) => void;
}

export function ErrorBoundary(props: ParentProps<ErrorBoundaryProps>) {
  const [error, setError] = createSignal<Error | null>(null);

  const reset = () => setError(null);

  onMount(() => {
    const handler = (event: ErrorEvent) => {
      event.preventDefault();
      const err = event.error instanceof Error ? event.error : new Error(String(event.error));
      setError(err);
      props.onError?.(err);
    };
    const rejectionHandler = (event: PromiseRejectionEvent) => {
      event.preventDefault();
      const err = event.reason instanceof Error ? event.reason : new Error(String(event.reason));
      setError(err);
    };
    window.addEventListener("error", handler);
    window.addEventListener("unhandledrejection", rejectionHandler);
    return () => {
      window.removeEventListener("error", handler);
      window.removeEventListener("unhandledrejection", rejectionHandler);
    };
  });

  const handleReset = () => {
    reset();
  };

  return (
    <Show
      when={!error()}
      fallback={
        props.fallback ? (
          props.fallback(error()!, handleReset)
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
              {error()?.message || "An unexpected error occurred"}
            </div>
            <div style={{ display: "flex", gap: "var(--space-2, 8px)", "margin-top": "var(--space-2, 8px)" }}>
              <Button variant="primary" onClick={handleReset}>
                Try Again
              </Button>
              <Button variant="secondary" onClick={() => window.location.reload()}>
                Reload App
              </Button>
            </div>
          </div>
        )
      }
    >
      {props.children}
    </Show>
  );
}
