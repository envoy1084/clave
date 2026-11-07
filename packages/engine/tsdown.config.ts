import defineConfig from "klarity/tsdown/node";

export default defineConfig({
  entry: { claved: "src/claved.ts" },
  // A program, not a library: no declarations. See packages/vault-wallet for the TS 7.0 notices.
  dts: false,
  suppressWarnings: [
    /^TypeScript 7\.0 does not yet have a stable API and is experimental\. Some options will be unavailable\.$/,
    /^The `tsgo` generator on TypeScript 7\.0 is deprecated/,
  ],
  target: "node24",
});
