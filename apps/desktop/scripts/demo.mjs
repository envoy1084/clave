// Runs several members on this machine for a demo: one app instance per named profile, each with its own data dir,
// keychain-sealed keys, keeper (own peer id and ports) and PXE. Windows cascade so they don't overlap exactly.
//
//   pnpm --filter @tecpan/desktop demo                 # Ana, Bea, Cai
//   pnpm --filter @tecpan/desktop demo -- Ana Bea      # any names
//   pnpm --filter @tecpan/desktop demo -- --fresh      # wipe those profiles first
import { spawn } from "node:child_process";
import { rm } from "node:fs/promises";
import { createRequire } from "node:module";
import { homedir } from "node:os";
import { join } from "node:path";

const args = process.argv.slice(2).filter((a) => a !== "--");
const fresh = args.includes("--fresh");
const names = args.filter((a) => !a.startsWith("--"));
const profiles = names.length > 0 ? names : ["Ana", "Bea", "Cai"];

const appData =
  process.platform === "darwin"
    ? join(homedir(), "Library", "Application Support")
    : process.platform === "win32"
      ? (process.env.APPDATA ?? join(homedir(), "AppData", "Roaming"))
      : (process.env.XDG_CONFIG_HOME ?? join(homedir(), ".config"));

if (fresh)
  await Promise.all(
    profiles.map((p) =>
      rm(join(appData, "Tecpan", "profiles", p), { recursive: true, force: true }),
    ),
  );

const electron = createRequire(import.meta.url)("electron");
const children = profiles.map((profile, i) => {
  const child = spawn(electron, [".", `--profile=${profile}`, `--cascade=${i}`], {
    cwd: join(import.meta.dirname, ".."),
    stdio: ["ignore", "inherit", "inherit"],
    env: { ...process.env, RUST_LOG: process.env.RUST_LOG ?? "warn" },
  });
  process.stdout.write(`${profile}: profile ${join(appData, "Tecpan", "profiles", profile)}\n`);
  return child;
});

const stop = () => {
  for (const child of children) child.kill("SIGTERM");
};
process.on("SIGINT", stop);
process.on("SIGTERM", stop);
await Promise.all(children.map((c) => new Promise((resolve) => c.once("exit", resolve))));
