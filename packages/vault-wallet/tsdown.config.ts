import defineConfig from "klarity/tsdown/node";

export default defineConfig({
  entry: { index: "src/index.ts" },
  // Klarity's preset suppresses tsdown's TS 7.0 "experimental API" notice; tsdown 0.23.7 adds a deprecation notice
  // for the same generator. Keep TypeScript 7.0 (7.1 is still beta) and suppress both until the upgrade.
  suppressWarnings: [
    /^TypeScript 7\.0 does not yet have a stable API and is experimental\. Some options will be unavailable\.$/,
    /^The `tsgo` generator on TypeScript 7\.0 is deprecated/,
  ],
  target: "node24",
});
