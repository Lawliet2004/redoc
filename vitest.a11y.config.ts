import path from "node:path";
import { defineConfig } from "vitest/config";
import solidPlugin from "vite-plugin-solid";

export default defineConfig({
  plugins: [
    solidPlugin({
      extensions: [".tsx", ".ts", ".jsx", ".js"],
    }),
  ],
  resolve: {
    alias: {
      "@redoc/ui/theme.css": path.resolve(__dirname, "packages/ui/src/theme.css"),
      "@redoc/ui": path.resolve(__dirname, "packages/ui/src/index.ts"),
      "@redoc/icons": path.resolve(__dirname, "packages/icons/src/index.tsx"),
      "@redoc/api-client": path.resolve(__dirname, "packages/api-client/src/index.ts"),
      "@redoc/editor-common": path.resolve(__dirname, "packages/editor-common/src/index.ts"),
      "@redoc/utils": path.resolve(__dirname, "packages/utils/src/index.ts"),
    },
  },
  test: {
    environment: "jsdom",
    include: ["tests/a11y/**/*.test.tsx"],
    setupFiles: ["tests/a11y/setup.ts"],
  },
});
