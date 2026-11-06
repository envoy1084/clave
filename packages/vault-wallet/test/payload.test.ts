import { Fr } from "@aztec-labs/foundation/curves/bn254";
import { FunctionCall, FunctionSelector, FunctionType } from "@aztec-labs/stdlib/abi";
import { AuthWitness } from "@aztec-labs/stdlib/auth-witness";
import { AztecAddress } from "@aztec-labs/stdlib/aztec-address";
import { Capsule, ExecutionPayload, HashedValues } from "@aztec-labs/stdlib/tx";
import { describe, expect, it } from "vitest";

import { MAX_PAYLOAD_BYTES, decodePayload, encodePayload } from "../src/vault/payload.ts";

const call = async (args: number) =>
  FunctionCall.from({
    name: "transfer",
    to: await AztecAddress.random(),
    selector: FunctionSelector.fromField(new Fr(0x1234)),
    type: FunctionType.PRIVATE,
    hideMsgSender: false,
    isStatic: false,
    args: Array.from({ length: args }, (_, i) => new Fr(i + 1)),
    returnType: undefined,
  });

describe("proposal payload codec", () => {
  it("round-trips every part of an execution payload", async () => {
    const payload = new ExecutionPayload(
      [await call(3), await call(1)],
      [new AuthWitness(Fr.random(), [Fr.random(), Fr.random()])],
      [new Capsule(await AztecAddress.random(), Fr.random(), [Fr.random()])],
      [await HashedValues.fromArgs([Fr.random(), Fr.random()])],
      await AztecAddress.random(),
    );
    const decoded = decodePayload(encodePayload(payload));
    expect(decoded.calls.map((c) => c.toJSON())).toEqual(payload.calls.map((c) => c.toJSON()));
    expect(decoded.authWitnesses.map(String)).toEqual(payload.authWitnesses.map(String));
    expect(decoded.capsules.map(String)).toEqual(payload.capsules.map(String));
    expect(decoded.extraHashedArgs.map((h) => h.hash.toString())).toEqual(
      payload.extraHashedArgs.map((h) => h.hash.toString()),
    );
    expect(decoded.feePayer?.toString()).toBe(payload.feePayer?.toString());
    expect(encodePayload(decoded).equals(encodePayload(payload))).toBe(true);
  });

  it("rejects payloads larger than a proposal can carry", async () => {
    const big = new ExecutionPayload(
      await Promise.all(Array.from({ length: 400 }, () => call(16))),
      [],
      [],
    );
    expect(() => encodePayload(big)).toThrow(String(MAX_PAYLOAD_BYTES));
  });
});
