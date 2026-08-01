import axe, { type AxeResults, type Result } from "axe-core";
import { expect } from "vitest";

const FAIL_IMPACTS = new Set(["serious", "critical"]);

/** Rules that are noisy under jsdom / dark chrome CSS variables. */
const DISABLED_RULES = {
  "color-contrast": { enabled: false },
} as const;

export function formatViolations(violations: Result[]): string {
  return violations
    .map((violation) => {
      const nodes = violation.nodes
        .map((node) => `  - ${node.target.join(" ")}: ${node.failureSummary ?? node.html}`)
        .join("\n");
      return `[${violation.impact}] ${violation.id}: ${violation.help}\n${nodes}`;
    })
    .join("\n\n");
}

export async function runAxe(container: Element = document.body): Promise<AxeResults> {
  return axe.run(container, {
    rules: { ...DISABLED_RULES },
  });
}

export async function expectNoSeriousA11yViolations(container: Element = document.body) {
  const results = await runAxe(container);
  const failing = results.violations.filter(
    (violation) => violation.impact != null && FAIL_IMPACTS.has(violation.impact),
  );
  expect(failing, formatViolations(failing)).toEqual([]);
}
