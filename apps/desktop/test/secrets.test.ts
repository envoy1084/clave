import { generateEpochTaggingSecret, generateVaultProtocolSecrets } from "@tecpan/vault-wallet";
import { describe, expect, it } from "vitest";

import { decodeSecrets, encodeSecrets } from "../src/engine/secrets.ts";

describe("shared secrets codec", () => {
  it("round-trips and fits the keeper's 1 KiB limit", async () => {
    const secrets = {
      protocol: await generateVaultProtocolSecrets(),
      epochTaggingSecret: await generateEpochTaggingSecret(),
    };
    const encoded = encodeSecrets(secrets);
    expect(encoded).toMatch(/^0x[0-9a-f]+$/u);
    expect((encoded.length - 2) / 2).toBeLessThanOrEqual(1024);
    const decoded = decodeSecrets(encoded);
    expect(decoded.protocol.nullifierHidingSecretKey.toString()).toBe(
      secrets.protocol.nullifierHidingSecretKey.toString(),
    );
    expect(decoded.protocol.outgoingViewingSecretKey.toString()).toBe(
      secrets.protocol.outgoingViewingSecretKey.toString(),
    );
    expect(decoded.protocol.taggingSecretKey.toString()).toBe(
      secrets.protocol.taggingSecretKey.toString(),
    );
    expect(
      decoded.protocol.messageSigningPublicKey.equals(secrets.protocol.messageSigningPublicKey),
    ).toBe(true);
    expect(decoded.protocol.fallbackPublicKey.equals(secrets.protocol.fallbackPublicKey)).toBe(
      true,
    );
    expect(decoded.epochTaggingSecret.equals(secrets.epochTaggingSecret)).toBe(true);
  });
});
