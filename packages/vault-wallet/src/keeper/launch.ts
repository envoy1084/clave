import { type ChildProcess, spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import { createInterface } from "node:readline";

import { KeeperClient } from "./client.ts";

/** How to launch a keeper. */
export interface KeeperLaunchOptions {
  /** Path to the `tecpan-keeper` binary. */
  readonly binary: string;
  /** Keeper data directory. */
  readonly dataDir: string;
  /** 32-byte master key (hex) sealing the keeper's secrets; the desktop keeps it in the OS keychain. */
  readonly masterKey: string;
  /** RPC socket path (default `<dataDir>/keeper.sock`). */
  readonly rpcSocket?: string;
  /** Peer listen address (default `127.0.0.1:0`). */
  readonly listen?: string;
  /** Extra environment (e.g. `RUST_LOG`). */
  readonly env?: NodeJS.ProcessEnv;
}

/** A running keeper. */
export interface LaunchedKeeper {
  readonly process: ChildProcess;
  readonly client: KeeperClient;
  readonly identity: string;
  readonly transport: string;
  readonly addresses: readonly string[];
  readonly rpcSocket: string;
  /** Stops the keeper (SIGTERM) and closes the client. */
  stop(): Promise<void>;
}

/**
 * Starts `tecpan-keeper run`, hands it the master key and a fresh RPC token on stdin (never on the command line or in
 * the environment), waits for its ready line and connects an authenticated client.
 */
export async function launchKeeper(options: KeeperLaunchOptions): Promise<LaunchedKeeper> {
  const token = randomBytes(32).toString("hex");
  const args = ["run", "--data-dir", options.dataDir, "--secrets-stdin"];
  if (options.rpcSocket) args.push("--rpc-socket", options.rpcSocket);
  if (options.listen) args.push("--listen", options.listen);
  const child = spawn(options.binary, args, {
    stdio: ["pipe", "pipe", "inherit"],
    env: { ...process.env, ...options.env },
  });
  child.stdin.end(`${JSON.stringify({ masterKey: options.masterKey, rpcToken: token })}\n`);
  const ready = await new Promise<{
    identity: string;
    transport: string;
    addresses: string[];
    rpcSocket: string;
  }>((resolve, reject) => {
    const lines = createInterface({ input: child.stdout });
    lines.once("line", (line) => {
      try {
        resolve(JSON.parse(line).ready);
      } catch (error) {
        reject(error);
      }
    });
    child.once("exit", (code) =>
      reject(new Error(`tecpan-keeper exited with ${code} before it was ready`)),
    );
  });
  const client = await KeeperClient.connect(ready.rpcSocket, token);
  return {
    process: child,
    client,
    ...ready,
    async stop() {
      client.close();
      if (child.exitCode !== null) return;
      const exited = new Promise((resolve) => child.once("exit", resolve));
      child.kill("SIGTERM");
      await exited;
    },
  };
}
