import { resolve } from "node:path";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "electron-vite";

const shared = { "@shared": resolve(import.meta.dirname, "src/shared") };

export default defineConfig({
  main: {
    resolve: { alias: shared },
    build: {
      outDir: "dist/main",
      rollupOptions: {
        // The engine runs in its own `utilityProcess`: the PXE, proving and vault logic stay out of the main process.
        input: {
          index: resolve(import.meta.dirname, "src/main/index.ts"),
          engine: resolve(import.meta.dirname, "src/engine/index.ts"),
        },
      },
    },
  },
  preload: {
    resolve: { alias: shared },
    build: {
      outDir: "dist/preload",
      // Sandboxed preloads cannot load ES modules or node_modules: bundle everything into one CommonJS file.
      externalizeDeps: false,
      rollupOptions: { output: { format: "cjs", entryFileNames: "[name].cjs" } },
    },
  },
  renderer: {
    root: "src/renderer",
    resolve: { alias: { ...shared, "@": resolve(import.meta.dirname, "src/renderer/src") } },
    plugins: [react(), tailwindcss()],
    build: { outDir: resolve(import.meta.dirname, "dist/renderer"), minify: true },
  },
});
