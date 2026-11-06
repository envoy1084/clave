import { describe, expect, it } from "vitest";

import { peerIdOf } from "../src/peer-id.ts";

describe("peerIdOf", () => {
  it("matches the peer id the keeper reports for its identity", () => {
    // From a real `tecpan-keeper run` ready line.
    expect(peerIdOf("0x78c72670e3ec97d3ace307d19d65e79a53623b375c8156c3e3a6228e0556ee3d")).toBe(
      "12D3KooWHwqDpuKQk5eTnsiTzCC2BHeQH3bM6JihH416RaEGHhSc",
    );
  });

  it("returns nothing for malformed identities", () => {
    expect(peerIdOf("0x1234")).toBe("");
    expect(peerIdOf("not hex")).toBe("");
  });
});
