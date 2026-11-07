import { randomBytes } from "node:crypto";

import { Fr } from "@aztec-labs/foundation/curves/bn254";
import { GrumpkinScalar } from "@aztec-labs/foundation/curves/grumpkin";

import { Engine } from "./engine/engine.ts";
import type { VaultRecord } from "./engine/protocol.ts";
import { KeeperSupervisor } from "./keeper/supervisor.ts";
import { type ProfilePaths, keeperBinary } from "./profile/paths.ts";
import { ProfileStore, SecretsStore, type StoredSecrets, VaultStore } from "./profile/stores.ts";
import type { Method, Params } from "./rpc/contract.ts";
import { DaemonError } from "./rpc/errors.ts";
import { Signer } from "./signer/signer.ts";
import type {
  DaemonEvent,
  DaemonInfo,
  EngineState,
  Settings,
  Status,
  VaultSummary,
} from "./types.ts";
import { VERSION } from "./version.ts";

/**
 * One member profile's daemon: the profile files, the signer, the keeper supervisor and the engine (PXE, founding,
 * proposals). Calls arrive validated against the contract; the daemon serves its own methods and forwards the rest to
 * the engine.
 */
export class Daemon {
  readonly #paths: ProfilePaths;
  readonly #profile: ProfileStore;
  readonly #secrets: SecretsStore;
  readonly #vaultStore: VaultStore;
  readonly #signer: Signer;
  readonly #keeper: KeeperSupervisor;
  readonly #emit: (event: DaemonEvent) => void;
  readonly #startedAt = Date.now();
  #engine: Engine | undefined;
  #engineState: EngineState = { status: "stopped" };
  #keeperToken: string | undefined;
  #vaults = new Map<string, VaultRecord>();
  /** Set once `daemon.stop` asks the process to exit. */
  onStop: () => void = () => undefined;

  constructor(paths: ProfilePaths, emit: (event: DaemonEvent) => void) {
    this.#paths = paths;
    this.#emit = emit;
    this.#profile = new ProfileStore(paths);
    this.#secrets = new SecretsStore(paths);
    this.#vaultStore = new VaultStore(paths);
    this.#signer = new Signer(paths.signer);
    this.#keeper = new KeeperSupervisor(
      async () => (await this.#requireSecrets()).keeperMasterKey,
      (keeper) => {
        this.#emit({ type: "keeper", keeper });
        void this.#followKeeper();
      },
    );
  }

  async start(): Promise<void> {
    await this.#profile.load();
    for (const record of await this.#vaultStore.all())
      this.#vaults.set(record.address.toLowerCase(), record);
    if (this.#profile.value.profile) await this.#startServices();
  }

  async stop(): Promise<void> {
    this.#signer.lock();
    this.#engine?.stop();
    await this.#keeper.stop();
  }

  async handle(method: Method, params: unknown): Promise<unknown> {
    switch (method) {
      case "daemon.info":
        return this.#info();
      case "daemon.stop":
        setImmediate(() => this.onStop());
        return null;
      case "status":
        return this.#status();
      case "profile.create":
        return this.#createProfile(params as Params<"profile.create">);
      case "config.get":
        return this.#profile.value.settings;
      case "config.set":
        return this.#updateSettings(params as Params<"config.set">);
      case "relays.set": {
        const settings = await this.#updateSettings({
          relays: (params as Params<"relays.set">).relays,
        });
        if (this.#profile.value.profile) await this.#keeper.restart({ relays: settings.relays });
        return settings;
      }
      case "signer.status":
        return this.#signer.status();
      case "signer.unlock": {
        const { secret, ttlSecs } = params as Params<"signer.unlock">;
        const limit = this.#profile.value.settings.unlockTimeoutSecs;
        if (ttlSecs > limit)
          throw new DaemonError(
            "invalid",
            `Unlock windows are at most ${limit} s (clave config set unlock-timeout)`,
          );
        return this.#signer.unlock(secret, ttlSecs);
      }
      case "signer.lock":
        return this.#signer.lock();
      case "signer.sealingKey":
        return {
          key: await this.#signer.sealingKey((params as Params<"signer.sealingKey">).passphrase),
        };
      case "node.status":
        return (
          this.#engine?.nodeStatus ?? {
            url: this.#profile.value.settings.nodeUrl,
            reachable: false,
            checkedAt: 0,
          }
        );
      case "vault.list":
        return this.#summaries();
      case "vault.alias": {
        const { vault, alias } = params as Params<"vault.alias">;
        this.#record(vault);
        const taken = Object.entries(this.#profile.value.aliases).find(
          ([address, name]) => name === alias && address !== vault.toLowerCase(),
        );
        if (taken) throw new DaemonError("invalid", `vaults:${alias} already names ${taken[0]}`);
        await this.#profile.update((v) => ({
          ...v,
          aliases: { ...v.aliases, [vault.toLowerCase()]: alias },
        }));
        return this.#summaries();
      }
      case "vault.use": {
        const { vault } = params as Params<"vault.use">;
        this.#record(vault);
        await this.#profile.update((v) => ({ ...v, defaultVault: vault.toLowerCase() }));
        return this.#summaries();
      }
      case "contacts.list":
        return this.#profile.value.contacts;
      case "contacts.put": {
        const { address, label } = params as Params<"contacts.put">;
        await this.#profile.update((v) => ({
          ...v,
          contacts: [
            ...v.contacts.filter(
              (c) => c.address.toLowerCase() !== address.toLowerCase() && c.label !== label,
            ),
            { address, label },
          ],
        }));
        return this.#profile.value.contacts;
      }
      case "contacts.remove": {
        const { address } = params as Params<"contacts.remove">;
        await this.#profile.update((v) => ({
          ...v,
          contacts: v.contacts.filter((c) => c.address.toLowerCase() !== address.toLowerCase()),
        }));
        return this.#profile.value.contacts;
      }
      default:
        return this.#requireEngine().call(method, params);
    }
  }

  #info(): DaemonInfo {
    return {
      version: VERSION,
      pid: process.pid,
      profile: this.#paths.name,
      profileDir: this.#paths.dir,
      startedAt: this.#startedAt,
      node: this.#profile.value.settings.nodeUrl,
    };
  }

  async #status(): Promise<Status> {
    const { profile, settings, defaultVault } = this.#profile.value;
    return {
      profile,
      settings,
      keeper: this.#keeper.state,
      engine: this.#engineState,
      node: this.#engine?.nodeStatus ?? { url: settings.nodeUrl, reachable: false, checkedAt: 0 },
      pxe: { syncedBlock: await this.#engine?.syncedBlock() },
      signer: this.#signer.status(),
      vaults: this.#summaries(),
      defaultVault,
    };
  }

  async #createProfile({ name, nodeUrl, passphrase }: Params<"profile.create">): Promise<Status> {
    if (this.#profile.value.profile)
      throw new DaemonError("invalid", "This profile already exists");
    const signerKey = await this.#signer.create(passphrase);
    if (!(await this.#secrets.get())) {
      await this.#secrets.put({
        account: {
          secret: Fr.random().toString(),
          salt: Fr.random().toString(),
          signingKey: GrumpkinScalar.random().toString(),
        },
        keeperMasterKey: randomBytes(32).toString("hex"),
      });
    }
    await this.#profile.update((v) => ({
      ...v,
      settings: { ...v.settings, nodeUrl },
      profile: { name, signerKey, account: "", createdAt: Date.now() },
    }));
    await this.#startServices();
    return this.#status();
  }

  /** Settings take effect for the keeper at once (relays) and for the node and prover on the next daemon start. */
  async #updateSettings(change: Partial<Settings>): Promise<Settings> {
    await this.#profile.update((v) => ({ ...v, settings: { ...v.settings, ...change } }));
    return this.#profile.value.settings;
  }

  async #startServices(): Promise<void> {
    const { profile, settings } = this.#profile.value;
    if (!profile) return;
    await this.#keeper.start({
      binary: keeperBinary(),
      dataDir: this.#paths.keeper,
      socket: this.#paths.keeperSocket,
      relays: settings.relays,
      log: this.#paths.keeperLog,
    });
    this.#setEngine({ status: "starting" });
    const secrets = await this.#requireSecrets();
    const engine = new Engine(
      {
        dataDir: this.#paths.engine,
        nodeUrl: settings.nodeUrl,
        proverEnabled: settings.proverEnabled,
        profile: { name: profile.name, signerKey: profile.signerKey },
        account: secrets.account,
        vaults: [...this.#vaults.values()],
      },
      {
        emit: (event) => this.#emit(event),
        saveVault: async (record) => {
          await this.#vaultStore.put(record);
          this.#vaults.set(record.address.toLowerCase(), record);
        },
        adoptAccount: (account) => void this.#adoptAccount(account),
        sign: (message, secret) => this.#signer.sign(message, secret),
        contacts: () => this.#profile.value.contacts,
      },
    );
    this.#engine = engine;
    try {
      this.#keeperToken = this.#keeper.connection?.token;
      await engine.connectKeeper(this.#keeper.connection);
      await engine.start();
      this.#setEngine({ status: "running" });
    } catch (error) {
      this.#setEngine({
        status: "failed",
        error: error instanceof Error ? error.message : String(error),
      });
    }
  }

  /** A keeper restart issues a new session token: reconnect the engine with it. */
  async #followKeeper(): Promise<void> {
    const connection = this.#keeper.connection;
    if (!this.#engine || !connection || connection.token === this.#keeperToken) return;
    this.#keeperToken = connection.token;
    await this.#engine
      .connectKeeper(connection)
      .catch((error: unknown) =>
        process.stderr.write(`claved: keeper reconnect failed: ${String(error)}\n`),
      );
  }

  async #adoptAccount(account: string): Promise<void> {
    if (this.#profile.value.profile?.account === account) return;
    await this.#profile.update((v) => ({ ...v, profile: v.profile && { ...v.profile, account } }));
  }

  #summaries(): VaultSummary[] {
    const { aliases, defaultVault } = this.#profile.value;
    const records = [...this.#vaults.values()].toSorted((a, b) => a.foundedAt - b.foundedAt);
    const fallback = records.length === 1 ? records[0]?.address.toLowerCase() : undefined;
    return records.map((record) => {
      const key = record.address.toLowerCase();
      return {
        address: record.address,
        name: record.name,
        alias: aliases[key],
        threshold: record.threshold,
        viewingThreshold: record.viewingThreshold,
        members: record.members.length,
        myIndex: record.myIndex,
        foundedAt: record.foundedAt,
        isDefault: key === (defaultVault ?? fallback),
      };
    });
  }

  #record(vault: string): VaultRecord {
    const record = this.#vaults.get(vault.toLowerCase());
    if (!record) throw new DaemonError("invalid", "This vault is not on this profile");
    return record;
  }

  #requireEngine(): Engine {
    if (!this.#profile.value.profile)
      throw new DaemonError("not-ready", "Create your profile first: clave init");
    if (!this.#engine) throw new DaemonError("not-ready", "The engine is still starting");
    return this.#engine;
  }

  async #requireSecrets(): Promise<StoredSecrets> {
    const secrets = await this.#secrets.get();
    if (!secrets)
      throw new DaemonError("not-ready", "This profile has no account yet: run clave init");
    return secrets;
  }

  #setEngine(state: EngineState): void {
    this.#engineState = state;
  }
}
