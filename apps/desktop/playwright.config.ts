import { defineConfig } from "@playwright/test";

// End-to-end tests drive the built app (`pnpm build`) against `aztec start --local-network`.
export default defineConfig({
  testDir: "e2e",
  timeout: 12 * 60_000,
  expect: { timeout: 30_000 },
  workers: 1,
  reporter: [["list"]],
  outputDir: "dist/e2e-results",
});
