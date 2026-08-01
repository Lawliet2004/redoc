import { defineConfig } from "vitest/config";

/** Unit tests only — a11y suite uses vitest.a11y.config.ts via `pnpm a11y:check`. */
export default defineConfig({
  test: {
    exclude: ["**/node_modules/**", "**/dist/**", "**/tests/a11y/**", "**/cypress/**"],
  },
});
