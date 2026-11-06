import { BackendType, Barretenberg } from "@aztec-foundation/bb.js";
import { Schnorr } from "@aztec-labs/foundation/crypto/schnorr";
import { Fr } from "@aztec-labs/foundation/curves/bn254";
import { GrumpkinScalar } from "@aztec-labs/foundation/curves/grumpkin";
import type { Point } from "@shared/types.ts";

import type { SecureStore } from "./secure-store.ts";

const KEY = "signer";

/** What the member is asked to confirm before a signature. */
export interface ApprovalPrompt {
  readonly vaultName: string;
  readonly summary: string;
  readonly message: string;
  readonly slot: number;
}

/** Asks the member to confirm; the main process passes a native dialog (and Touch ID where available). */
export type Confirm = (prompt: ApprovalPrompt) => Promise<boolean>;

let barretenberg: Promise<void> | undefined;
/** Schnorr needs Barretenberg; the main process uses its WASM build (no native `bb` process here). */
const ready = () =>
  (barretenberg ??= Barretenberg.initSingleton({ backend: BackendType.Wasm }).then(
    () => undefined,
  ));

/**
 * The member's approval signing key (Grumpkin). It is generated here, sealed with the OS keychain, and used only in
 * this process: the engine and the renderer receive public keys and signatures, never the key.
 */
export class Signer {
  readonly #key: GrumpkinScalar;
  readonly publicKey: Point;
  readonly #confirm: Confirm;

  private constructor(key: GrumpkinScalar, publicKey: Point, confirm: Confirm) {
    this.#key = key;
    this.publicKey = publicKey;
    this.#confirm = confirm;
  }

  /** Loads the member's key, creating it on first use. */
  static async open(store: SecureStore, confirm: Confirm): Promise<Signer> {
    await ready();
    let hex = await store.get<string>(KEY);
    if (!hex) {
      hex = GrumpkinScalar.random().toString();
      await store.put(KEY, hex);
    }
    const key = GrumpkinScalar.fromString(hex);
    const point = await new Schnorr().computePublicKey(key);
    return new Signer(key, { x: point.x.toString(), y: point.y.toString() }, confirm);
  }

  /**
   * Signs an approval message after the member confirms it. Returns the signature limbs `[s.lo, s.hi, e.lo, e.hi]`,
   * or `null` when the member declines.
   */
  async signApproval(prompt: ApprovalPrompt): Promise<string[] | null> {
    if (!(await this.#confirm(prompt))) return null;
    const signature = await new Schnorr().constructSignature(
      Fr.fromString(prompt.message),
      this.#key,
    );
    return signature.toLimbFields().map((limb) => limb.toString());
  }
}
