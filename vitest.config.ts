import { defineConfig } from "vitest/config";

/** Unit tests only — a11y suite uses vitest.a11y.config.ts via `pnpm a11y:check`. */
export default defineConfig({
  test: {
    environment: "jsdom",
    include: [
      "packages/**/*.test.ts",
      "packages/**/*.test.tsx",
      "tests/perf/**/*.test.ts",
    ],
    exclude: ["**/node_modules/**", "**/dist/**", "**/tests/a11y/**", "**/cypress/**", "**/tests/e2e/**"],
    coverage: {
      provider: "v8",
      reporter: ["text", "json", "html", "lcov"],
      include: ["packages/**/src/**/*.ts", "packages/**/src/**/*.tsx"],
      exclude: [
        "**/*.test.ts",
        "**/*.test.tsx",
        "**/*.d.ts",
        "**/dist/**",
        "**/node_modules/**",
        "**/generated.ts", // Auto-generated bindings
      ],
      thresholds: {
        statements: 60,
        branches: 50,
        functions: 60,
        lines: 60,
      },
    },
  },
});
