import { mkdtemp, stat } from "node:fs/promises";
import { createConnection } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";

import { afterEach, describe, expect, it } from "vitest";

import { parseCall } from "../src/rpc/contract.ts";
import { DaemonError } from "../src/rpc/errors.ts";
import { RpcServer } from "../src/rpc/server.ts";

const servers: RpcServer[] = [];
afterEach(async () => Promise.all(servers.splice(0).map((s) => s.close())));

async function start() {
  const path = join(await mkdtemp(join(tmpdir(), "tecpan-rpc-")), "daemon.sock");
  const server = await RpcServer.listen(path, "t0ken", async (method, params) => {
    if (method === "signer.status") return { locked: true };
    if (method === "proposals.execute") throw new DaemonError("not-ready", "1 of 2 approvals");
    return params;
  });
  servers.push(server);
  return { path, server };
}

/** A raw client: sends lines, collects replies and events. */
async function client(path: string) {
  const socket = createConnection(path);
  await new Promise((resolve) => socket.once("connect", resolve));
  const lines: unknown[] = [];
  const waiters: (() => void)[] = [];
  createInterface({ input: socket }).on("line", (line) => {
    lines.push(JSON.parse(line));
    waiters.splice(0).forEach((w) => w());
  });
  let id = 0;
  const next = async (count: number) => {
    // oxlint-disable-next-line no-await-in-loop -- waits for one reply line at a time.
    while (lines.length < count) await new Promise<void>((resolve) => waiters.push(resolve));
    return lines[count - 1] as Record<string, unknown>;
  };
  return {
    call: async (method: string, params?: unknown) => {
      socket.write(`${JSON.stringify({ jsonrpc: "2.0", id: ++id, method, params })}\n`);
      return next(lines.length + 1);
    },
    next,
    close: () => socket.end(),
  };
}

describe("rpc server", () => {
  it("is owner-only and requires the token before any call", async () => {
    const { path } = await start();
    expect((await stat(path)).mode & 0o777).toBe(0o600);
    const c = await client(path);
    expect(await c.call("signer.status")).toMatchObject({ error: { data: { kind: "auth" } } });
    expect(await c.call("session.auth", { token: "wrong" })).toMatchObject({
      error: { data: { kind: "auth" } },
    });
    expect(await c.call("session.auth", { token: "t0ken" })).toMatchObject({ result: true });
    expect(await c.call("signer.status")).toMatchObject({ result: { locked: true } });
    c.close();
  });

  it("validates params and carries error kinds", async () => {
    const { path } = await start();
    const c = await client(path);
    await c.call("session.auth", { token: "t0ken" });
    expect(await c.call("nope")).toMatchObject({
      error: { code: -32602, data: { kind: "invalid" } },
    });
    expect(await c.call("vault.details", { vault: "0x12" })).toMatchObject({
      error: { data: { kind: "invalid" } },
    });
    const ref = { vault: `0x${"1".repeat(64)}`, id: `0x${"2".repeat(64)}` };
    expect(await c.call("proposals.execute", ref)).toMatchObject({
      error: { code: 1001, message: "1 of 2 approvals", data: { kind: "not-ready" } },
    });
    c.close();
  });

  it("streams events to subscribers", async () => {
    const { path, server } = await start();
    const c = await client(path);
    await c.call("session.auth", { token: "t0ken" });
    await c.call("events.subscribe");
    server.broadcast({ type: "vault-activity", vault: "0x01" });
    expect(await c.next(3)).toMatchObject({
      method: "event",
      params: { type: "vault-activity", vault: "0x01" },
    });
    c.close();
  });
});

describe("contract", () => {
  it("rejects malformed intents and secrets", () => {
    const vault = `0x${"a".repeat(64)}`;
    const id = `0x${"b".repeat(64)}`;
    expect(() =>
      parseCall("proposals.create", {
        vault,
        expiresInHours: 24,
        intent: { kind: "transfer", token: vault, to: vault, amount: "0" },
      }),
    ).toThrow("intent.amount");
    expect(() => parseCall("proposals.approve", { vault, id, secret: { key: "0x12" } })).toThrow();
    expect(() => parseCall("proposals.reject", { vault, id, reason: "x".repeat(281) })).toThrow();
    expect(parseCall("proposals.approve", { vault, id }).method).toBe("proposals.approve");
    expect(() =>
      parseCall("profile.create", { name: "ana", nodeUrl: "http://x:8080", passphrase: "short" }),
    ).toThrow("at least 8");
  });
});
