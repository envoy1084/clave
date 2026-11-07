import { BackendType, Barretenberg } from "@aztec-foundation/bb.js";
import { Schnorr } from "@aztec-labs/foundation/crypto/schnorr";
import { Fr } from "@aztec-labs/foundation/curves/bn254";
import { GrumpkinScalar } from "@aztec-labs/foundation/curves/grumpkin";

import { readJson, writeJson } from "../profile/files.ts";
import type { Secret } from "../rpc/contract.ts";
import { DaemonError } from "../rpc/errors.ts";
import type { Point, SignerStatus } from "../types.ts";
import { type Sealed, open, saltOf, seal, sealingKey } from "./sealing.ts";

let barretenberg: Promise<void> | undefined;
/** Schnorr needs Barretenberg; the signer uses its WASM build. */
const ready = () =>
  (barretenberg ??= Barretenberg.initSingleton({ backend: BackendType.Wasm }).then(
    () => undefined,
  ));

/**
 * The member's approval signing key (Grumpkin), sealed in `signer.sealed` under a passphrase-derived key. It is opened
 * for one signature, or kept open for an unlock window the member asked for; nothing else in the daemon sees it.
 */
export class Signer {
  readonly #path: string;
  #unlocked: { key: GrumpkinScalar; until: number; timer: NodeJS.Timeout } | undefined;

  constructor(path: string) {
    this.#path = path;
  }

  /** Generates the key, seals it under `passphrase` and returns its public half. */
  async create(passphrase: string): Promise<Point> {
    if (await readJson<Sealed>(this.#path))
      throw new DaemonError("invalid", "This profile already has a signing key");
    await ready();
    const key = GrumpkinScalar.random();
    await writeJson(this.#path, await seal(Buffer.from(key.toString(), "utf8"), passphrase));
    return publicKeyOf(key);
  }

  status(): SignerStatus {
    if (this.#unlocked && this.#unlocked.until > Date.now())
      return { locked: false, unlockedUntil: this.#unlocked.until };
    return { locked: true };
  }

  /** Keeps the key open for `ttlSecs` (or until {@link lock}). */
  async unlock(secret: Secret, ttlSecs: number): Promise<SignerStatus> {
    const key = await this.#open(secret);
    this.lock();
    const until = Date.now() + ttlSecs * 1000;
    this.#unlocked = { key, until, timer: setTimeout(() => this.lock(), ttlSecs * 1000).unref() };
    return this.status();
  }

  lock(): SignerStatus {
    if (this.#unlocked) clearTimeout(this.#unlocked.timer);
    this.#unlocked = undefined;
    return { locked: true };
  }

  /** The sealing key for `passphrase`, checked against `signer.sealed` (for the CLI's Touch ID keychain item). */
  async sealingKey(passphrase: string): Promise<string> {
    const sealed = await this.#sealed();
    const key = await sealingKey(passphrase, saltOf(sealed));
    open(sealed, key).fill(0);
    return `0x${key.toString("hex")}`;
  }

  /**
   * Signs `message` (a field element) and returns the contract's limbs `[s.lo, s.hi, e.lo, e.hi]`. Uses the unlock
   * window when one is open, else `secret`; with neither it fails with a `locked` error.
   */
  async sign(message: string, secret?: Secret): Promise<string[]> {
    await ready();
    const key =
      this.#unlocked && this.#unlocked.until > Date.now() ? this.#unlocked.key : undefined;
    const signingKey = key ?? (secret ? await this.#open(secret) : undefined);
    if (!signingKey)
      throw new DaemonError(
        "locked",
        "The signer is locked: give your passphrase or run clave unlock",
      );
    const signature = await new Schnorr().constructSignature(Fr.fromString(message), signingKey);
    return signature.toLimbFields().map((limb) => limb.toString());
  }

  async #sealed(): Promise<Sealed> {
    const sealed = await readJson<Sealed>(this.#path);
    if (!sealed) throw new DaemonError("not-ready", "No signing key yet: run clave init");
    return sealed;
  }

  async #open(secret: Secret): Promise<GrumpkinScalar> {
    const sealed = await this.#sealed();
    const key =
      "passphrase" in secret
        ? await sealingKey(secret.passphrase, saltOf(sealed))
        : Buffer.from(secret.key.slice(2), "hex");
    const plaintext = open(sealed, key);
    key.fill(0);
    const scalar = GrumpkinScalar.fromString(plaintext.toString("utf8"));
    plaintext.fill(0);
    return scalar;
  }
}

async function publicKeyOf(key: GrumpkinScalar): Promise<Point> {
  const point = await new Schnorr().computePublicKey(key);
  return { x: point.x.toString(), y: point.y.toString() };
}
