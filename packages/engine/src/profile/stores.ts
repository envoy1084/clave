import { readdir } from "node:fs/promises";

import type { VaultRecord } from "../engine/protocol.ts";
import type { Contact, Profile, Settings } from "../types.ts";
import { readJson, writeJson } from "./files.ts";
import type { ProfilePaths } from "./paths.ts";

/** Non-secret profile data: the public profile, settings, the address book and local vault preferences. */
export interface StoredProfile {
  readonly profile: Profile | null;
  readonly settings: Settings;
  readonly contacts: readonly Contact[];
  /** Vault address (lowercase) → local alias. */
  readonly aliases: Readonly<Record<string, string>>;
  readonly defaultVault?: string;
}

export const DEFAULT_SETTINGS: Settings = {
  nodeUrl: "http://localhost:8080",
  relays: [],
  proverEnabled: false,
  unlockTimeoutSecs: 3600,
};

/** `profile.json`. */
export class ProfileStore {
  readonly #path: string;
  #value: StoredProfile = { profile: null, settings: DEFAULT_SETTINGS, contacts: [], aliases: {} };

  constructor(paths: ProfilePaths) {
    this.#path = paths.profile;
  }

  get value(): StoredProfile {
    return this.#value;
  }

  async load(): Promise<StoredProfile> {
    const stored = await readJson<Partial<StoredProfile>>(this.#path);
    this.#value = {
      profile: stored?.profile ?? null,
      settings: { ...DEFAULT_SETTINGS, ...stored?.settings },
      contacts: stored?.contacts ?? [],
      aliases: stored?.aliases ?? {},
      defaultVault: stored?.defaultVault,
    };
    return this.#value;
  }

  async update(change: (value: StoredProfile) => StoredProfile): Promise<StoredProfile> {
    this.#value = change(this.#value);
    await writeJson(this.#path, this.#value);
    return this.#value;
  }
}

/** The member's own Aztec account (an initializerless Schnorr account that pays for deployments). */
export interface AccountSecrets {
  readonly secret: string;
  readonly salt: string;
  readonly signingKey: string;
}

/**
 * `secrets.json`: the member's account secrets and the keeper master key. Protected by file permissions (0600 in a
 * 0700 directory) so the daemon can start unattended; the approval signing key is sealed separately under the
 * member's passphrase.
 */
export interface StoredSecrets {
  readonly account: AccountSecrets;
  readonly keeperMasterKey: string;
}

export class SecretsStore {
  readonly #path: string;

  constructor(paths: ProfilePaths) {
    this.#path = paths.secrets;
  }

  get(): Promise<StoredSecrets | undefined> {
    return readJson<StoredSecrets>(this.#path);
  }

  put(value: StoredSecrets): Promise<void> {
    return writeJson(this.#path, value);
  }
}

/** `vaults/<address>.json`: founded vault records. */
export class VaultStore {
  readonly #paths: ProfilePaths;

  constructor(paths: ProfilePaths) {
    this.#paths = paths;
  }

  async all(): Promise<VaultRecord[]> {
    let files: string[];
    try {
      files = await readdir(this.#paths.vaults);
    } catch {
      return [];
    }
    const records = await Promise.all(
      files
        .filter((f) => /^0x[0-9a-f]{64}\.json$/u.test(f))
        .map((f) => readJson<VaultRecord>(this.#paths.vault(f.slice(0, -".json".length)))),
    );
    return records.filter((r): r is VaultRecord => r !== undefined);
  }

  put(record: VaultRecord): Promise<void> {
    return writeJson(this.#paths.vault(record.address), record);
  }
}
