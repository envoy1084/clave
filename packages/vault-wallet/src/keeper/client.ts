import { type Socket, createConnection } from "node:net";
import { createInterface } from "node:readline";

/** An error returned by the keeper. */
export class KeeperRpcError extends Error {
  readonly code: number;

  constructor(code: number, message: string) {
    super(message);
    this.name = "KeeperRpcError";
    this.code = code;
  }
}

/** An event streamed by the keeper (`events.subscribe`). */
export type KeeperEvent = { readonly type: string } & Record<string, unknown>;

interface Pending {
  readonly resolve: (value: unknown) => void;
  readonly reject: (error: Error) => void;
}

/**
 * JSON-RPC client for the member's own keeper over its local socket (newline-delimited JSON). The session is
 * authenticated with the keeper's launch token before any call.
 */
export class KeeperClient {
  readonly #socket: Socket;
  readonly #pending = new Map<number, Pending>();
  readonly #listeners = new Set<(event: KeeperEvent) => void>();
  #next = 0;
  #closed: Error | undefined;

  private constructor(socket: Socket) {
    this.#socket = socket;
    createInterface({ input: socket, crlfDelay: Number.POSITIVE_INFINITY }).on("line", (line) =>
      this.#receive(line),
    );
    socket.on("close", () => this.#fail(new Error("keeper connection closed")));
    socket.on("error", (error) => this.#fail(error));
  }

  /** Connects to `socketPath` and authenticates with `token`. */
  static async connect(socketPath: string, token: string): Promise<KeeperClient> {
    const socket = await new Promise<Socket>((resolve, reject) => {
      const s = createConnection(socketPath, () => resolve(s));
      s.once("error", reject);
    });
    const client = new KeeperClient(socket);
    await client.call("session.auth", { token });
    return client;
  }

  /** Calls `method`; rejects with {@link KeeperRpcError} on a keeper error. */
  call<T>(method: string, params: unknown = null): Promise<T> {
    if (this.#closed) return Promise.reject(this.#closed);
    const id = ++this.#next;
    return new Promise<T>((resolve, reject) => {
      this.#pending.set(id, { resolve: resolve as (value: unknown) => void, reject });
      this.#socket.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    });
  }

  /** Streams keeper events to `listener`; returns an unsubscribe function. */
  async subscribe(listener: (event: KeeperEvent) => void): Promise<() => void> {
    if (this.#listeners.size === 0) await this.call("events.subscribe");
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  close(): void {
    this.#socket.end();
  }

  #receive(line: string): void {
    let message: {
      id?: number;
      result?: unknown;
      error?: { code: number; message: string };
      method?: string;
      params?: unknown;
    };
    try {
      message = JSON.parse(line);
    } catch {
      return;
    }
    if (message.method === "event") {
      for (const listener of this.#listeners) listener(message.params as KeeperEvent);
      return;
    }
    const pending = message.id === undefined ? undefined : this.#pending.get(message.id);
    if (!pending || message.id === undefined) return;
    this.#pending.delete(message.id);
    if (message.error)
      pending.reject(new KeeperRpcError(message.error.code, message.error.message));
    else pending.resolve(message.result);
  }

  #fail(error: Error): void {
    this.#closed ??= error;
    for (const { reject } of this.#pending.values()) reject(error);
    this.#pending.clear();
  }
}
