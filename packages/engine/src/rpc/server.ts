import { timingSafeEqual } from "node:crypto";
import { chmod, rm } from "node:fs/promises";
import { type Server, type Socket, createServer } from "node:net";
import { createInterface } from "node:readline";

import type { DaemonEvent } from "../types.ts";
import { parseCall, type Method } from "./contract.ts";
import { DaemonError, rpcError } from "./errors.ts";

export type Handler = (method: Method, params: unknown) => Promise<unknown>;

const MAX_LINE = 1 << 20;

interface Request {
  readonly id?: number | string | null;
  readonly method?: unknown;
  readonly params?: unknown;
}

/**
 * The daemon's JSON-RPC server on its unix socket, in the keeper's framing: newline-delimited JSON-RPC 2.0, the first
 * call of every connection is `session.auth { token }`, and `events.subscribe` turns the connection into an event
 * stream as well. The socket is owner-only (0600) inside the owner-only profile directory.
 */
export class RpcServer {
  readonly #server: Server;
  readonly #token: Buffer;
  readonly #subscribers = new Set<Socket>();

  private constructor(token: string, handle: Handler) {
    this.#token = Buffer.from(token, "utf8");
    this.#server = createServer((socket) => this.#connection(socket, handle));
  }

  static async listen(path: string, token: string, handle: Handler): Promise<RpcServer> {
    await rm(path, { force: true });
    const self = new RpcServer(token, handle);
    await new Promise<void>((resolve, reject) => {
      self.#server.once("error", reject);
      self.#server.listen(path, () => resolve());
    });
    await chmod(path, 0o600);
    return self;
  }

  /** Sends `event` to every subscribed connection. */
  broadcast(event: DaemonEvent): void {
    const line = `${JSON.stringify({ jsonrpc: "2.0", method: "event", params: event })}\n`;
    for (const socket of this.#subscribers) socket.write(line);
  }

  close(): Promise<void> {
    for (const socket of this.#subscribers) socket.end();
    return new Promise((resolve) => this.#server.close(() => resolve()));
  }

  #connection(socket: Socket, handle: Handler): void {
    let authenticated = false;
    const reply = (id: Request["id"], body: { result: unknown } | { error: unknown }) => {
      if (id === undefined || socket.destroyed) return;
      socket.write(`${JSON.stringify({ jsonrpc: "2.0", id, ...body })}\n`);
    };
    socket.on("error", () => this.#subscribers.delete(socket));
    socket.on("close", () => this.#subscribers.delete(socket));
    const lines = createInterface({ input: socket, crlfDelay: Number.POSITIVE_INFINITY });
    lines.on("line", (line) => {
      if (line.length > MAX_LINE) {
        socket.destroy();
        return;
      }
      let request: Request;
      try {
        request = JSON.parse(line) as Request;
      } catch {
        reply(null, { error: { code: -32700, message: "Parse error" } });
        return;
      }
      if (request.method === "session.auth") {
        authenticated = this.#matches((request.params as { token?: unknown } | null)?.token);
        reply(
          request.id,
          authenticated
            ? { result: true }
            : { error: rpcError(new DaemonError("auth", "invalid token")) },
        );
        return;
      }
      if (!authenticated) {
        reply(request.id, {
          error: rpcError(new DaemonError("auth", "authenticate with session.auth first")),
        });
        return;
      }
      if (request.method === "events.subscribe") {
        this.#subscribers.add(socket);
        reply(request.id, { result: null });
        return;
      }
      void (async () => {
        try {
          const call = parseCall(request.method, request.params);
          reply(request.id, { result: (await handle(call.method, call.params)) ?? null });
        } catch (error) {
          reply(request.id, { error: rpcError(error) });
        }
      })();
    });
  }

  #matches(given: unknown): boolean {
    if (typeof given !== "string") return false;
    const candidate = Buffer.from(given, "utf8");
    return candidate.length === this.#token.length && timingSafeEqual(candidate, this.#token);
  }
}
