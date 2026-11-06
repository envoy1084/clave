// A keeper's libp2p peer id from its Ed25519 identity (hex), so the app can show the id other members see without
// asking the keeper: base58btc(identity multihash(protobuf PublicKey { Type: Ed25519, Data: key })).
const ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function base58(bytes: Uint8Array): string {
  let value = 0n;
  for (const byte of bytes) value = (value << 8n) | BigInt(byte);
  let out = "";
  while (value > 0n) {
    out = ALPHABET[Number(value % 58n)] + out;
    value /= 58n;
  }
  for (const byte of bytes) {
    if (byte !== 0) break;
    out = `1${out}`;
  }
  return out;
}

export function peerIdOf(identity: string): string {
  const hex = identity.replace(/^0x/u, "");
  if (!/^[0-9a-fA-F]{64}$/u.test(hex)) return "";
  const key = Uint8Array.from(hex.match(/../gu) ?? [], (b) => Number.parseInt(b, 16));
  return base58(Uint8Array.from([0x00, 0x24, 0x08, 0x01, 0x12, 0x20, ...key]));
}
