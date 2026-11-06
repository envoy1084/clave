// Passphrase sealing of the approval signing key (architecture/cli.md, "Signer"): Argon2id derives a 32-byte sealing
// key, ChaCha20-Poly1305 seals the key under it with a fixed associated-data tag. Node's own crypto only.
import { argon2, createCipheriv, createDecipheriv, randomBytes } from "node:crypto";
import { promisify } from "node:util";

import { DaemonError } from "../rpc/errors.ts";

const derive = promisify(argon2);
const AAD = Buffer.from("tecpan.signer.v1", "utf8");
/** 64 MiB, 3 passes, one lane (memory is given in KiB). */
const KDF = { memory: 65_536, passes: 3, parallelism: 1, tagLength: 32 } as const;

export interface Sealed {
  readonly v: 1;
  readonly kdf: { readonly salt: string };
  readonly nonce: string;
  readonly ciphertext: string;
}

/** The sealing key for `passphrase` under the stored salt. */
export async function sealingKey(passphrase: string, salt: Buffer): Promise<Buffer> {
  return Buffer.from(
    await derive("argon2id", { message: Buffer.from(passphrase, "utf8"), nonce: salt, ...KDF }),
  );
}

export async function seal(plaintext: Buffer, passphrase: string): Promise<Sealed> {
  const salt = randomBytes(16);
  const key = await sealingKey(passphrase, salt);
  const nonce = randomBytes(12);
  const cipher = createCipheriv("chacha20-poly1305", key, nonce, { authTagLength: 16 });
  cipher.setAAD(AAD, { plaintextLength: plaintext.length });
  const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final(), cipher.getAuthTag()]);
  key.fill(0);
  return {
    v: 1,
    kdf: { salt: salt.toString("hex") },
    nonce: nonce.toString("hex"),
    ciphertext: ciphertext.toString("hex"),
  };
}

/** Opens `sealed` with the sealing key; throws an `auth` error when the key is wrong. */
export function open(sealed: Sealed, key: Buffer): Buffer {
  const bytes = Buffer.from(sealed.ciphertext, "hex");
  const decipher = createDecipheriv("chacha20-poly1305", key, Buffer.from(sealed.nonce, "hex"), {
    authTagLength: 16,
  });
  decipher.setAAD(AAD, { plaintextLength: bytes.length - 16 });
  decipher.setAuthTag(bytes.subarray(bytes.length - 16));
  try {
    return Buffer.concat([decipher.update(bytes.subarray(0, bytes.length - 16)), decipher.final()]);
  } catch (error) {
    throw new DaemonError("auth", "Wrong passphrase", { cause: error });
  }
}

export const saltOf = (sealed: Sealed) => Buffer.from(sealed.kdf.salt, "hex");
