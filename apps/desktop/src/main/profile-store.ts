import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { join } from "node:path";

import type { Contact, Profile, Settings } from "@shared/types.ts";

/** Non-secret profile data and settings, as plain JSON in the data directory. */
export interface StoredProfile {
  readonly profile: Profile | null;
  readonly settings: Settings;
  /** Address book: labelled counterparties, kept on this device only. */
  readonly contacts: readonly Contact[];
}

export const DEFAULT_SETTINGS: Settings = {
  nodeUrl: "http://localhost:8080",
  relays: [],
  proverEnabled: false,
  technicalDetails: false,
};

export class ProfileStore {
  readonly #path: string;
  #value: StoredProfile = { profile: null, settings: DEFAULT_SETTINGS, contacts: [] };

  constructor(dir: string) {
    this.#path = join(dir, "profile.json");
  }

  get value(): StoredProfile {
    return this.#value;
  }

  async load(): Promise<StoredProfile> {
    try {
      const stored = JSON.parse(await readFile(this.#path, "utf8")) as Partial<StoredProfile>;
      this.#value = {
        profile: stored.profile ?? null,
        settings: { ...DEFAULT_SETTINGS, ...stored.settings },
        contacts: stored.contacts ?? [],
      };
    } catch {
      this.#value = { profile: null, settings: DEFAULT_SETTINGS, contacts: [] };
    }
    return this.#value;
  }

  async update(change: (value: StoredProfile) => StoredProfile): Promise<StoredProfile> {
    this.#value = change(this.#value);
    await mkdir(join(this.#path, ".."), { recursive: true });
    await writeFile(`${this.#path}.tmp`, JSON.stringify(this.#value, null, 2));
    await rename(`${this.#path}.tmp`, this.#path);
    return this.#value;
  }
}
