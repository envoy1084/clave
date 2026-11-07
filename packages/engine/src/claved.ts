#!/usr/bin/env node
// claved: one member profile's daemon. `clave daemon start` runs it detached with its output in daemon.log; it serves
// the contract in architecture/cli.md on <profile-dir>/daemon.sock until `daemon.stop` or SIGTERM.
import { randomBytes } from "node:crypto";
import { chmod, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { parseArgs } from "node:util";

import { Daemon } from "./daemon.ts";
import { ProfilePaths } from "./profile/paths.ts";
import { RpcServer } from "./rpc/server.ts";
import { VERSION } from "./version.ts";

const log = (message: string) => process.stdout.write(`${new Date().toISOString()} ${message}\n`);

async function alreadyRunning(paths: ProfilePaths): Promise<number | undefined> {
  const pid = Number.parseInt(await readFile(paths.pid, "utf8").catch(() => ""), 10);
  if (!Number.isInteger(pid) || pid <= 0 || pid === process.pid) return undefined;
  try {
    process.kill(pid, 0);
    return pid;
  } catch {
    return undefined;
  }
}

async function main(): Promise<void> {
  const { values } = parseArgs({
    options: {
      "profile-dir": { type: "string" },
      // The daemon never forks itself; the CLI decides whether to detach it. Accepted for symmetry with the CLI.
      foreground: { type: "boolean" },
      version: { type: "boolean" },
    },
  });
  if (values.version) {
    process.stdout.write(`claved ${VERSION}\n`);
    return;
  }
  const dir = values["profile-dir"];
  if (!dir) throw new Error("usage: claved --profile-dir <dir>");
  const paths = new ProfilePaths(dir);
  await mkdir(paths.dir, { recursive: true, mode: 0o700 });
  await chmod(paths.dir, 0o700);
  const running = await alreadyRunning(paths);
  if (running) throw new Error(`a daemon is already running for this profile (pid ${running})`);

  const token = randomBytes(32).toString("hex");
  await writeFile(paths.token, token, { mode: 0o600 });
  await writeFile(paths.pid, `${process.pid}\n`, { mode: 0o600 });
  await writeFile(paths.addr, `${paths.socket}\n`, { mode: 0o600 });

  // Events only flow once the daemon has started, after the server below exists.
  const daemon = new Daemon(paths, (event) => server.broadcast(event));
  const server = await RpcServer.listen(paths.socket, token, (method, params) =>
    daemon.handle(method, params),
  );
  log(`claved ${VERSION} serving profile ${paths.name} on ${paths.socket}`);

  let stopping = false;
  const shutdown = async (reason: string) => {
    if (stopping) return;
    stopping = true;
    log(`stopping (${reason})`);
    await daemon.stop().catch((error: unknown) => log(`stop: ${String(error)}`));
    await server.close();
    await Promise.all(
      [paths.socket, paths.addr, paths.token, paths.pid].map((p) => rm(p, { force: true })),
    );
    process.exit(0);
  };
  daemon.onStop = () => void shutdown("daemon.stop");
  process.on("SIGTERM", () => void shutdown("SIGTERM"));
  process.on("SIGINT", () => void shutdown("SIGINT"));
  process.on("unhandledRejection", (error) => log(`unhandled rejection: ${String(error)}`));

  await daemon.start();
  log("ready");
}

main().catch((error: unknown) => {
  process.stderr.write(`claved: ${error instanceof Error ? error.message : String(error)}\n`);
  process.exit(1);
});
