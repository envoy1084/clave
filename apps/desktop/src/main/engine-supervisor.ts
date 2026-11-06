import { join } from "node:path";

import type {
  EngineInit,
  EngineToMain,
  HostMethod,
  MainToEngine,
} from "@shared/engine-protocol.ts";
import type { AppEvent, EngineState } from "@shared/types.ts";
import { type UtilityProcess, utilityProcess } from "electron";

const CALL_TIMEOUT_MS = 10 * 60_000;

/** A utility process's port takes no target origin; the lint rule is for window.postMessage. */
// oxlint-disable-next-line unicorn/require-post-message-target-origin
const send = (child: UtilityProcess, message: MainToEngine) => child.postMessage(message);

interface Pending {
  readonly resolve: (value: unknown) => void;
  readonly reject: (error: Error) => void;
  readonly timer: NodeJS.Timeout;
}

export interface EngineHooks {
  readonly onState: (state: EngineState) => void;
  readonly onEvent: (event: AppEvent) => void;
  readonly onReady: (info: { account?: string }) => void;
  readonly host: (method: HostMethod, params: unknown) => Promise<unknown>;
}

/**
 * Runs the engine (PXE, proving, vault logic) in an Electron `utilityProcess` and relays calls to it. A crash fails
 * the calls in flight and restarts the engine with the same init.
 */
export class EngineSupervisor {
  readonly #hooks: EngineHooks;
  #child: UtilityProcess | undefined;
  #init: (() => Promise<EngineInit>) | undefined;
  #ready: Promise<void> | undefined;
  #pending = new Map<number, Pending>();
  #next = 0;
  #state: EngineState = { status: "stopped" };
  #crashes = 0;
  #stopping = false;

  constructor(hooks: EngineHooks) {
    this.#hooks = hooks;
  }

  get state(): EngineState {
    return this.#state;
  }

  /** Starts the engine; `init` is read again on every restart so it always carries current settings and vaults. */
  start(init: () => Promise<EngineInit>): Promise<void> {
    this.#init = init;
    this.#stopping = false;
    this.#ready = this.#spawn();
    return this.#ready;
  }

  async restart(): Promise<void> {
    await this.stop();
    if (this.#init) await this.start(this.#init);
  }

  async call<T>(method: string, params: unknown): Promise<T> {
    if (!this.#ready) throw new Error("The engine is not running");
    await this.#ready;
    const child = this.#child;
    if (!child) throw new Error("The engine is not running");
    const id = ++this.#next;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#pending.delete(id);
        reject(new Error(`${method} timed out`));
      }, CALL_TIMEOUT_MS);
      this.#pending.set(id, { resolve: resolve as (value: unknown) => void, reject, timer });
      send(child, { kind: "call", id, method, params } satisfies MainToEngine);
    });
  }

  async stop(): Promise<void> {
    this.#stopping = true;
    const child = this.#child;
    this.#child = undefined;
    this.#ready = undefined;
    if (child) {
      const exited = new Promise((resolve) => child.once("exit", resolve));
      child.kill();
      await exited;
    }
    this.#failAll(new Error("The engine stopped"));
    this.#set({ status: "stopped" });
  }

  #spawn(): Promise<void> {
    const init = this.#init;
    if (!init) return Promise.resolve();
    this.#set({ status: this.#crashes ? "restarting" : "starting" });
    const child = utilityProcess.fork(join(import.meta.dirname, "engine.js"), [], {
      serviceName: "Tecpan engine",
      stdio: "inherit",
    });
    this.#child = child;
    return new Promise<void>((resolve, reject) => {
      child.on("message", (message: EngineToMain) => {
        switch (message.kind) {
          case "ready":
            this.#crashes = 0;
            this.#set({ status: "running" });
            this.#hooks.onReady({ account: message.account });
            resolve();
            break;
          case "reply":
            this.#settle(message);
            break;
          case "event":
            this.#hooks.onEvent(message.event);
            break;
          case "host-call":
            void this.#hooks.host(message.method, message.params).then(
              (result) =>
                send(child, { kind: "host-reply", id: message.id, result } satisfies MainToEngine),
              (error: unknown) =>
                send(child, {
                  kind: "host-reply",
                  id: message.id,
                  error: String(error),
                } satisfies MainToEngine),
            );
            break;
        }
      });
      child.once("exit", (code) => {
        if (this.#child !== child) return;
        this.#child = undefined;
        this.#failAll(new Error("The engine stopped unexpectedly"));
        if (this.#stopping) return;
        this.#crashes += 1;
        const error = `The engine exited (code ${code})`;
        if (this.#crashes > 3) {
          this.#set({ status: "failed", error });
          reject(new Error(error));
          return;
        }
        this.#set({ status: "restarting", error });
        setTimeout(() => {
          this.#ready = this.#spawn();
          this.#ready.then(resolve, reject);
        }, 1000 * this.#crashes);
      });
      void init().then(
        (value) => send(child, { kind: "init", init: value } satisfies MainToEngine),
        reject,
      );
    });
  }

  #settle(message: Extract<EngineToMain, { kind: "reply" }>): void {
    const pending = this.#pending.get(message.id);
    if (!pending) return;
    this.#pending.delete(message.id);
    clearTimeout(pending.timer);
    if (message.error !== undefined) pending.reject(new Error(message.error));
    else pending.resolve(message.result);
  }

  #failAll(error: Error): void {
    for (const pending of this.#pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this.#pending.clear();
  }

  #set(state: EngineState): void {
    this.#state = state;
    this.#hooks.onState(state);
  }
}
