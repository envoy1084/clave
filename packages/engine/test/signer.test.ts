import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { verifySignature } from "@clave/napi";
import { describe, expect, it } from "vitest";

import { DaemonError } from "../src/rpc/errors.ts";
import { open, saltOf, seal, sealingKey } from "../src/signer/sealing.ts";
import { Signer } from "../src/signer/signer.ts";

const MESSAGE = "0x0000000000000000000000000000000000000000000000000000000000001234";
const kindOf = async (promise: Promise<unknown>) =>
  promise.then(
    () => "ok",
    (error: unknown) => (error instanceof DaemonError ? error.kind : String(error)),
  );

describe("sealing", () => {
  it("opens with the passphrase's key and refuses any other", async () => {
    const sealed = await seal(Buffer.from("secret"), "correct horse");
    expect(open(sealed, await sealingKey("correct horse", saltOf(sealed))).toString()).toBe(
      "secret",
    );
    expect(() => open(sealed, Buffer.alloc(32))).toThrow(DaemonError);
    const tampered = { ...sealed, ciphertext: `00${sealed.ciphertext.slice(2)}` };
    expect(() => open(tampered, Buffer.alloc(32))).toThrow("Wrong passphrase");
  });
});

const signer = async () => {
  const dir = await mkdtemp(join(tmpdir(), "clave-signer-"));
  const s = new Signer(join(dir, "signer.sealed"));
  return { signer: s, publicKey: await s.create("correct horse") };
};

describe("signer", () => {
  it("signs only with a secret or inside an unlock window", async () => {
    const { signer: s, publicKey } = await signer();
    expect(await kindOf(s.sign(MESSAGE))).toBe("locked");
    expect(await kindOf(s.sign(MESSAGE, { passphrase: "wrong" }))).toBe("auth");
    const limbs = await s.sign(MESSAGE, { passphrase: "correct horse" });
    expect(verifySignature(publicKey, MESSAGE, limbs)).toBe(true);

    await s.unlock({ passphrase: "correct horse" }, 60);
    expect(s.status().locked).toBe(false);
    expect(verifySignature(publicKey, MESSAGE, await s.sign(MESSAGE))).toBe(true);
    s.lock();
    expect(await kindOf(s.sign(MESSAGE))).toBe("locked");
  });

  it("accepts the sealing key in place of the passphrase (Touch ID)", async () => {
    const { signer: s, publicKey } = await signer();
    const key = await s.sealingKey("correct horse");
    expect(verifySignature(publicKey, MESSAGE, await s.sign(MESSAGE, { key }))).toBe(true);
    expect(await kindOf(s.sealingKey("wrong"))).toBe("auth");
    expect(await kindOf(s.create("again!!!"))).toBe("invalid");
  });
});
