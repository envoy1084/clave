import { FunctionCall } from "@aztec-labs/stdlib/abi";
import { AuthWitness } from "@aztec-labs/stdlib/auth-witness";
import { AztecAddress } from "@aztec-labs/stdlib/aztec-address";
import { Capsule, ExecutionPayload, HashedValues } from "@aztec-labs/stdlib/tx";

/** Largest payload a proposal carries (the wire format's bound). */
export const MAX_PAYLOAD_BYTES = 64 * 1024;

interface EncodedPayload {
  readonly v: 0;
  readonly calls: unknown[];
  readonly authWitnesses: string[];
  readonly capsules: string[];
  readonly extraHashedArgs: string[];
  readonly feePayer?: string;
}

/**
 * Serialises an execution payload for a proposal: the calls through `FunctionCall`'s JSON schema, everything else
 * through each type's own byte encoding. Members re-derive the outer hash from the decoded payload before signing, so
 * a payload that does not decode to exactly what was proposed cannot collect approvals.
 */
export function encodePayload(payload: ExecutionPayload): Buffer {
  const encoded: EncodedPayload = {
    v: 0,
    calls: payload.calls.map((call) => JSON.parse(JSON.stringify(call)) as unknown),
    authWitnesses: payload.authWitnesses.map((w) => w.toString()),
    capsules: payload.capsules.map((c) => c.toString()),
    extraHashedArgs: payload.extraHashedArgs.map((h) => `0x${h.toBuffer().toString("hex")}`),
    ...(payload.feePayer ? { feePayer: payload.feePayer.toString() } : {}),
  };
  const bytes = Buffer.from(JSON.stringify(encoded), "utf8");
  if (bytes.length > MAX_PAYLOAD_BYTES)
    throw new Error(
      `Payload is ${bytes.length} bytes; a proposal carries at most ${MAX_PAYLOAD_BYTES}`,
    );
  return bytes;
}

export function decodePayload(bytes: Uint8Array): ExecutionPayload {
  const encoded = JSON.parse(Buffer.from(bytes).toString("utf8")) as EncodedPayload;
  if (encoded.v !== 0) throw new Error("Unknown payload encoding");
  return new ExecutionPayload(
    encoded.calls.map((call) => FunctionCall.schema.parse(call)),
    encoded.authWitnesses.map((w) => AuthWitness.fromString(w)),
    encoded.capsules.map((c) => Capsule.fromString(c)),
    encoded.extraHashedArgs.map((h) => HashedValues.fromBuffer(Buffer.from(h.slice(2), "hex"))),
    encoded.feePayer ? AztecAddress.schema.parse(encoded.feePayer) : undefined,
  );
}
