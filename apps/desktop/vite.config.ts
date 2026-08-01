import { defineConfig } from "vite";
import solidPlugin from "vite-plugin-solid";
import path from "path";

export default defineConfig({
  envPrefix: "VITE_",
  plugins: [
    solidPlugin({
      extensions: [".tsx", ".ts", ".jsx", ".js"],
    }),
  ],
  resolve: {
    alias: {
      "@redoc/ui/theme.css": path.resolve(__dirname, "../../packages/ui/src/theme.css"),
      "@redoc/ui": path.resolve(__dirname, "../../packages/ui/src/index.ts"),
      "@redoc/icons": path.resolve(__dirname, "../../packages/icons/src/index.tsx"),
      "@redoc/api-client": path.resolve(__dirname, "../../packages/api-client/src/index.ts"),
      "@redoc/editor-common": path.resolve(__dirname, "../../packages/editor-common/src/index.ts"),
      "@redoc/doc-editor": path.resolve(__dirname, "../../packages/doc-editor/src/index.ts"),
      "@redoc/sheet-editor": path.resolve(__dirname, "../../packages/sheet-editor/src/index.ts"),
      "@redoc/slide-editor": path.resolve(__dirname, "../../packages/slide-editor/src/index.ts"),
      "@redoc/utils": path.resolve(__dirname, "../../packages/utils/src/index.ts"),
    },
  },
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: "esnext",
    outDir: "dist",
  },
});
