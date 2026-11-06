import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import { AZTEC_VERSION } from "../src/index.ts";

describe("AZTEC_VERSION", () => {
  it("matches the toolchain pinned in .aztecrc", async () => {
    const pinned = await readFile(new URL("../../../.aztecrc", import.meta.url), "utf8");
    expect(AZTEC_VERSION).toBe(pinned.trim());
  });
});
