// The vault's shared protocol secrets travel between members as one opaque blob (sealed by the keepers per member);
// only engines ever read it. Compact JSON, hex encoded, well under the keeper's 1 KiB limit.
import { GrumpkinScalar, type Point } from "@aztec-labs/foundation/curves/grumpkin";
import { type VaultProtocolSecrets, fromNapiPoint, toNapiPoint } from "@clave/vault-wallet";

export interface SharedSecrets {
  readonly protocol: VaultProtocolSecrets;
  /** Tags the vault's unconstrained self-sends for the current committee epoch. */
  readonly epochTaggingSecret: Point;
}

const point = (p: Point) => {
  const { x, y } = toNapiPoint(p);
  return [x, y];
};
const fromPoint = ([x, y]: string[]) => {
  if (!x || !y) throw new Error("Malformed point in the vault secrets");
  return fromNapiPoint({ x, y });
};

export function encodeSecrets({ protocol, epochTaggingSecret }: SharedSecrets): string {
  const json = JSON.stringify({
    n: protocol.nullifierHidingSecretKey.toString(),
    o: protocol.outgoingViewingSecretKey.toString(),
    t: protocol.taggingSecretKey.toString(),
    m: point(protocol.messageSigningPublicKey),
    f: point(protocol.fallbackPublicKey),
    e: point(epochTaggingSecret),
  });
  return `0x${Buffer.from(json, "utf8").toString("hex")}`;
}

export function decodeSecrets(hex: string): SharedSecrets {
  const v = JSON.parse(Buffer.from(hex.replace(/^0x/u, ""), "hex").toString("utf8")) as {
    n: string;
    o: string;
    t: string;
    m: string[];
    f: string[];
    e: string[];
  };
  return {
    protocol: {
      nullifierHidingSecretKey: GrumpkinScalar.fromString(v.n),
      outgoingViewingSecretKey: GrumpkinScalar.fromString(v.o),
      taggingSecretKey: GrumpkinScalar.fromString(v.t),
      messageSigningPublicKey: fromPoint(v.m),
      fallbackPublicKey: fromPoint(v.f),
    },
    epochTaggingSecret: fromPoint(v.e),
  };
}
