import defineNodeVitestConfig from "klarity/vitest/node";

// Runs against `aztec start --local-network` (AZTEC_NODE_URL, default http://localhost:8080). Tests in a file share
// one deployed vault and run in order.
export default defineNodeVitestConfig({
  test: {
    include: ["test/e2e/**/*.test.ts"],
    fileParallelism: false,
    sequence: { concurrent: false },
    testTimeout: 600_000,
    hookTimeout: 900_000,
  },
});
