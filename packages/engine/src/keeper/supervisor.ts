import { mkdir, open } from "node:fs/promises";

import { type LaunchedKeeper, launchKeeper } from "@clave/vault-wallet";

import type { KeeperState } from "../types.ts";
/** Listen on every interface so co-signers on the LAN (or this machine) can dial in; ports are picked by the OS. */
const LISTEN = ["/ip4/0.0.0.0/udp/0/quic-v1", "/ip4/0.0.0.0/tcp/0"];
const MAX_BACKOFF_MS = 30_000;

export interface KeeperOptions {
  readonly binary: string;
  readonly dataDir: string;
  readonly socket: string;
  readonly relays: readonly string[];
  /** The keeper's log file. */
  readonly log: string;
}

/**
 * Runs the member's keeper as a child process: hands it the master key on stdin, restarts it with backoff when it
 * exits, and reports its state. The keeper holds viewing shares; this process never does.
 */
export class KeeperSupervisor {
  readonly #masterKey: () => Promise<string>;
  readonly #onChange: (state: KeeperState) => void;
  #options: KeeperOptions | undefined;
  #keeper: LaunchedKeeper | undefined;
  #state: KeeperState = { status: "stopped" };
  #failures = 0;
  #stopping = false;
  #timer: NodeJS.Timeout | undefined;

  constructor(masterKey: () => Promise<string>, onChange: (state: KeeperState) => void) {
    this.#masterKey = masterKey;
    this.#onChange = onChange;
  }

  get state(): KeeperState {
    return this.#state;
  }

  /** Socket and token the engine uses to reach this keeper. */
  get connection(): { socket: string; token: string } | null {
    return this.#keeper ? { socket: this.#keeper.rpcSocket, token: this.#keeper.token } : null;
  }

  async start(options: KeeperOptions): Promise<void> {
    this.#options = options;
    this.#stopping = false;
    await this.#launch();
  }

  async restart(options?: Partial<KeeperOptions>): Promise<void> {
    if (!this.#options) return;
    this.#options = { ...this.#options, ...options };
    await this.stop();
    this.#stopping = false;
    await this.#launch();
  }

  async stop(): Promise<void> {
    this.#stopping = true;
    clearTimeout(this.#timer);
    const keeper = this.#keeper;
    this.#keeper = undefined;
    await keeper?.stop();
    this.#set({ status: "stopped" });
  }

  async #launch(): Promise<void> {
    const options = this.#options;
    if (!options) return;
    this.#set({
      ...this.#state,
      status: this.#failures ? "restarting" : "starting",
      error: undefined,
    });
    try {
      const masterKey = await this.#masterKey();
      await mkdir(options.dataDir, { recursive: true, mode: 0o700 });
      const log = await open(options.log, "a", 0o600);
      const keeper = await launchKeeper({
        stderr: log.fd,
        binary: options.binary,
        dataDir: options.dataDir,
        masterKey,
        rpcSocket: options.socket,
        listen: LISTEN,
        relays: options.relays,
        env: { RUST_LOG: process.env.RUST_LOG ?? "info", NO_COLOR: "1" },
      });
      // The child holds its own copy of the descriptor.
      await log.close();
      this.#keeper = keeper;
      this.#failures = 0;
      keeper.process.once("exit", (code) => this.#exited(keeper, code));
      const info = await keeper.client.call<{ version: string }>("keeper.info");
      this.#set({
        status: "running",
        identity: keeper.identity,
        transport: keeper.transport,
        peerId: keeper.peerId,
        addresses: keeper.addresses,
        version: info.version,
      });
    } catch (error) {
      this.#keeper = undefined;
      this.#scheduleRestart(error instanceof Error ? error.message : String(error));
    }
  }

  #exited(keeper: LaunchedKeeper, code: number | null): void {
    if (this.#keeper !== keeper || this.#stopping) return;
    this.#keeper = undefined;
    this.#scheduleRestart(`The keeper stopped unexpectedly (exit code ${code ?? "unknown"})`);
  }

  #scheduleRestart(error: string): void {
    if (this.#stopping) return;
    this.#failures += 1;
    const delay = Math.min(MAX_BACKOFF_MS, 500 * 2 ** this.#failures);
    this.#set({ status: this.#failures > 5 ? "failed" : "restarting", error });
    this.#timer = setTimeout(() => void this.#launch(), delay);
  }

  #set(state: KeeperState): void {
    this.#state = state;
    this.#onChange(state);
  }
}
