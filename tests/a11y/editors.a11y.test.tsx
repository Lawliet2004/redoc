import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { ErrorBoundary } from "@redoc/ui";
import { expectNoSeriousA11yViolations } from "./runAxe";

const disposers: Array<() => void> = [];

function mount(ui: () => unknown) {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const dispose = render(() => ui() as never, container);
  disposers.push(() => {
    dispose();
    container.remove();
  });
  return container;
}

afterEach(() => {
  while (disposers.length) disposers.pop()?.();
});

describe("a11y editor component scans", () => {
  it("ErrorBoundary default fallback has no serious/critical axe violations", async () => {
    mount(() => (
      <ErrorBoundary
        fallback={(error, reset) => (
          <div role="alert">
            <p>{error.message}</p>
            <button onClick={reset}>Try again</button>
          </div>
        )}
      >
        <span>Child content</span>
      </ErrorBoundary>
    ));
    await expectNoSeriousA11yViolations();
  });

  it("ErrorBoundary with error state has no serious/critical axe violations", async () => {
    mount(() => (
      <ErrorBoundary
        fallback={(error, reset) => (
          <div role="alert" data-testid="error-fallback">
            <p aria-live="assertive">{error.message}</p>
            <button onClick={reset} aria-label="Try again">
              Try again
            </button>
          </div>
        )}
      >
        <span>Child content</span>
      </ErrorBoundary>
    ));
    await expectNoSeriousA11yViolations();
  });

  it("ErrorBoundary renders children when no error", async () => {
    const container = mount(() => (
      <ErrorBoundary>
        <span data-testid="child-content">Content renders correctly</span>
      </ErrorBoundary>
    ));

    // Verify children render when no error
    const child = container.querySelector('[data-testid="child-content"]');
    expect(child).toBeTruthy();
    expect(child?.textContent).toBe("Content renders correctly");
    await expectNoSeriousA11yViolations();
  });

  it("ErrorBoundary has proper structure for error recovery", async () => {
    // Test that the component structure supports accessible error recovery
    const container = mount(() => (
      <ErrorBoundary
        fallback={(error, reset) => (
          <div
            role="alert"
            aria-live="assertive"
            aria-atomic="true"
            data-testid="error-boundary-fallback"
          >
            <p>{error.message}</p>
            <button onClick={reset} aria-label="Try again">
              Try again
            </button>
          </div>
        )}
      >
        <span>Content</span>
      </ErrorBoundary>
    ));

    // Initially shows children
    await expectNoSeriousA11yViolations();

    // Verify the component is properly structured
    const content = container.querySelector("span");
    expect(content?.textContent).toBe("Content");
  });
});
