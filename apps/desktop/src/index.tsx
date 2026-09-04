import { render } from "solid-js/web";
import { App } from "./App";

// Startup budget instrumentation (prompt.md §10: cold start < 1.5 s).
// Marks script-eval time; App's first commit completes the measurement.
(window as unknown as Record<string, number>).__redocEvalMs = Math.round(performance.now());

const root = document.getElementById("root");

if (root) {
  render(() => <App />, root);
}
