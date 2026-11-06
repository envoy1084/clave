import { readdir } from "node:fs/promises";

import { Fr } from "@aztec-labs/foundation/curves/bn254";
import { GrumpkinScalar } from "@aztec-labs/foundation/curves/grumpkin";
import type { Method, Params, Results } from "@shared/contract.ts";
import type { EngineInit, HostMethod, VaultRecord } from "@shared/engine-protocol.ts";
import type { AppEvent, AppState, Settings, VaultSummary } from "@shared/types.ts";
import { clipboard, shell } from "electron";

import { EngineSupervisor } from "./engine-supervisor.ts";
import { KeeperSupervisor } from "./keeper-supervisor.ts";
import { probeNode } from "./node-probe.ts";
import { engineDir, keeperBinary, keeperDir, keeperSocket, secureDir, userData } from "./paths.ts";
import { ProfileStore } from "./profile-store.ts";
import { SecureStore } from "./secure-store.ts";
import { type Confirm, Signer } from "./signer.ts";

const ACCOUNT = "account";
const vaultKey = (address: string) => `vault-${address.toLowerCase()}`;

interface AccountSecrets {
  readonly secret: string;
  readonly salt: string;
  readonly signingKey: string;
}

const summarize = (record: VaultRecord): VaultSummary => ({
  address: record.address,
  name: record.name,
  threshold: record.threshold,
  viewingThreshold: record.viewingThreshold,
  members: record.members.length,
  myIndex: record.myIndex,
  foundedAt: record.foundedAt,
});

/**
 * The main process's application state: profile and settings, the signer, the keeper and engine supervisors, and the
 * sealed vault records. Renderer calls arrive here already validated against the IPC contract.
 */
export class Controller {
  readonly #secure = new SecureStore(secureDir());
  readonly #profiles = new ProfileStore(userData());
  readonly #keeper: KeeperSupervisor;
  readonly #engine: EngineSupervisor;
  readonly #emit: (event: AppEvent) => void;
  readonly #confirm: Confirm;
  #signer: Signer | undefined;
  /** Token of the keeper session the running engine was started with. */
  #engineKeeperToken: string | null | undefined;
  #vaults = new Map<string, VaultRecord>();

  constructor(emit: (event: AppEvent) => void, confirm: Confirm) {
    this.#emit = emit;
    this.#confirm = confirm;
    this.#keeper = new KeeperSupervisor(this.#secure, () => {
      this.#broadcast();
      void this.#followKeeper();
    });
    this.#engine = new EngineSupervisor({
      onState: () => this.#broadcast(),
      onEvent: (event) => this.#emit(event),
      onReady: (info) => void this.#adoptAccount(info.account),
      host: (method, params) => this.#host(method, params),
    });
  }

  async start(): Promise<void> {
    if (!SecureStore.available())
      throw new Error(
        "This system has no OS keychain available, so Tecpan cannot protect your keys.",
      );
    await this.#profiles.load();
    await this.#loadVaults();
    if (this.#profiles.value.profile) await this.#startServices();
  }

  async stop(): Promise<void> {
    await this.#engine.stop();
    await this.#keeper.stop();
  }

  async handle<M extends Method>(method: M, params: Params<M>): Promise<Results[M]> {
    return (await this.#dispatch(method, params)) as Results[M];
  }

  async #dispatch(method: Method, params: unknown): Promise<unknown> {
    switch (method) {
      case "app.state":
        return this.state();
      case "profile.create":
        return this.#createProfile(params as Params<"profile.create">);
      case "profile.rename":
        await this.#profiles.update((v) => ({
          ...v,
          profile: v.profile && { ...v.profile, name: (params as Params<"profile.rename">).name },
        }));
        return this.#broadcast();
      case "settings.update":
        return this.#updateSettings(params as Partial<Settings>);
      case "shell.openDataFolder":
        await shell.openPath(userData());
        return null;
      case "clipboard.write":
        await clipboard.writeText((params as Params<"clipboard.write">).text);
        return null;
      case "clipboard.readInvite": {
        // Only hands the renderer clipboard text that looks like an invite, never arbitrary clipboard contents.
        const text = (await clipboard.readText()).trim();
        return /^tecpan-invite-v0:[A-Za-z0-9_-]{16,4000}$/u.test(text) ? text : null;
      }
      case "node.probe":
        return probeNode((params as Params<"node.probe">).nodeUrl);
      case "keeper.restart":
        await this.#keeper.restart();
        return null;
      default:
        return this.#engine.call(method, params);
    }
  }

  state(): AppState {
    const { profile, settings } = this.#profiles.value;
    return {
      profile,
      settings,
      keeper: this.#keeper.state,
      engine: this.#engine.state,
      vaults: [...this.#vaults.values()]
        .map(summarize)
        .toSorted((a, b) => a.foundedAt - b.foundedAt),
      dataDir: userData(),
      platform: process.platform,
    };
  }

  async #createProfile({ name, nodeUrl }: Params<"profile.create">): Promise<AppState> {
    if (this.#profiles.value.profile) throw new Error("A profile already exists on this device");
    const signer = await this.#openSigner();
    if (!(await this.#secure.get(ACCOUNT))) {
      await this.#secure.put(ACCOUNT, {
        secret: Fr.random().toString(),
        salt: Fr.random().toString(),
        signingKey: GrumpkinScalar.random().toString(),
      } satisfies AccountSecrets);
    }
    await this.#profiles.update((v) => ({
      settings: { ...v.settings, nodeUrl },
      profile: { name, signerKey: signer.publicKey, account: "", createdAt: Date.now() },
    }));
    this.#broadcast();
    await this.#startServices();
    return this.state();
  }

  async #updateSettings(change: Partial<Settings>): Promise<AppState> {
    const before = this.#profiles.value.settings;
    await this.#profiles.update((v) => ({ ...v, settings: { ...v.settings, ...change } }));
    const after = this.#profiles.value.settings;
    if (this.#profiles.value.profile) {
      if (JSON.stringify(before.relays) !== JSON.stringify(after.relays))
        await this.#keeper.restart({ relays: after.relays });
      if (before.nodeUrl !== after.nodeUrl || before.proverEnabled !== after.proverEnabled)
        await this.#engine.restart();
    }
    return this.#broadcast();
  }

  async #startServices(): Promise<void> {
    await this.#openSigner();
    await this.#keeper.start({
      binary: keeperBinary(),
      dataDir: keeperDir(),
      socket: keeperSocket(),
      relays: this.#profiles.value.settings.relays,
    });
    await this.#engine.start(() => this.#engineInit());
  }

  async #openSigner(): Promise<Signer> {
    this.#signer ??= await Signer.open(this.#secure, this.#confirm);
    return this.#signer;
  }

  /** A keeper restart issues a new session token: restart the engine so it reconnects with it. */
  async #followKeeper(): Promise<void> {
    if (this.#engine.state.status !== "running") return;
    const token = this.#keeper.connection?.token ?? null;
    if (token && token !== this.#engineKeeperToken) await this.#engine.restart();
  }

  async #engineInit(): Promise<EngineInit> {
    const { profile, settings } = this.#profiles.value;
    this.#engineKeeperToken = this.#keeper.connection?.token ?? null;
    return {
      dataDir: engineDir(),
      nodeUrl: settings.nodeUrl,
      proverEnabled: settings.proverEnabled,
      profile: profile && { name: profile.name, signerKey: profile.signerKey },
      account: (await this.#secure.get<AccountSecrets>(ACCOUNT)) ?? null,
      keeper: this.#keeper.connection,
      vaults: [...this.#vaults.values()],
    };
  }

  async #adoptAccount(account: string | undefined): Promise<void> {
    if (!account || this.#profiles.value.profile?.account === account) return;
    await this.#profiles.update((v) => ({ ...v, profile: v.profile && { ...v.profile, account } }));
    this.#broadcast();
  }

  async #host(method: HostMethod, params: unknown): Promise<unknown> {
    switch (method) {
      case "vault.save": {
        const record = params as VaultRecord;
        await this.#secure.put(vaultKey(record.address), record);
        this.#vaults.set(record.address.toLowerCase(), record);
        this.#broadcast();
        return null;
      }
    }
  }

  async #loadVaults(): Promise<void> {
    let files: string[] = [];
    try {
      files = await readdir(secureDir());
    } catch {
      return;
    }
    const records = await Promise.all(
      files
        .filter((f) => /^vault-0x[0-9a-f]+\.sealed$/u.test(f))
        .map((f) => this.#secure.get<VaultRecord>(f.slice(0, -".sealed".length))),
    );
    for (const record of records)
      if (record) this.#vaults.set(record.address.toLowerCase(), record);
  }

  #broadcast(): AppState {
    const state = this.state();
    this.#emit({ type: "state", state });
    return state;
  }
}
