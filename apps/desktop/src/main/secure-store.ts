import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

import { safeStorage } from "electron";

/**
 * JSON values encrypted with the OS keychain (`safeStorage`) at rest, one file per key. Holds the signing key, the
 * keeper master key, the member's account secrets and the vault records. Plaintext never touches the disk.
 */
export class SecureStore {
  readonly #dir: string;

  constructor(dir: string) {
    this.#dir = dir;
  }

  static available(): boolean {
    return safeStorage.isEncryptionAvailable();
  }

  async get<T>(key: string): Promise<T | undefined> {
    let sealed: Buffer;
    try {
      sealed = await readFile(this.#path(key));
    } catch {
      return undefined;
    }
    const { result, shouldReEncrypt } = await safeStorage.decryptStringAsync(sealed);
    const value = JSON.parse(result) as T;
    // The keychain rotated its key: seal the value again under the new one.
    if (shouldReEncrypt) await this.put(key, value);
    return value;
  }

  async put(key: string, value: unknown): Promise<void> {
    await mkdir(this.#dir, { recursive: true, mode: 0o700 });
    const sealed = await safeStorage.encryptStringAsync(JSON.stringify(value));
    const path = this.#path(key);
    await writeFile(`${path}.tmp`, sealed, { mode: 0o600 });
    await rename(`${path}.tmp`, path);
  }

  async delete(key: string): Promise<void> {
    await rm(this.#path(key), { force: true });
  }

  #path(key: string): string {
    if (!/^[\w.-]{1,96}$/u.test(key)) throw new Error("Invalid store key");
    return join(this.#dir, `${key}.sealed`);
  }
}
