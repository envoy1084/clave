import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { app } from "electron";

/**
 * Chooses the data directory before the app is ready. Several members can run on one machine (and the end-to-end
 * tests run three): `--profile=<name>` or `TECPAN_PROFILE` selects a named profile, `TECPAN_USER_DATA` an exact path.
 */
export function selectUserData(): void {
  const explicit = process.env.TECPAN_USER_DATA;
  const profile =
    process.argv.find((arg) => arg.startsWith("--profile="))?.slice("--profile=".length) ??
    process.env.TECPAN_PROFILE;
  if (explicit) app.setPath("userData", resolve(explicit));
  else if (profile && /^[\w-]{1,32}$/u.test(profile))
    app.setPath("userData", join(app.getPath("appData"), "Tecpan", "profiles", profile));
  else app.setPath("userData", join(app.getPath("appData"), "Tecpan"));
}

export const userData = () => app.getPath("userData");
export const secureDir = () => join(userData(), "secure");
export const keeperDir = () => join(userData(), "keeper");
export const engineDir = () => join(userData(), "engine");

/** The keeper socket lives in the temp dir: macOS limits socket paths to 104 bytes, deeper than most data dirs. */
export const keeperSocket = () =>
  join(
    tmpdir(),
    `tecpan-${createHash("sha256").update(userData()).digest("hex").slice(0, 12)}.sock`,
  );

/** The bundled keeper binary, or a local build during development (`TECPAN_KEEPER_BIN` overrides both). */
export function keeperBinary(): string {
  if (process.env.TECPAN_KEEPER_BIN) return process.env.TECPAN_KEEPER_BIN;
  if (app.isPackaged) return join(process.resourcesPath, "bin", "tecpan-keeper");
  const crates = resolve(app.getAppPath(), "../../crates/target");
  const candidates = ["release", "debug"].map((profile) => join(crates, profile, "tecpan-keeper"));
  return candidates.find((path) => existsSync(path)) ?? join(crates, "release", "tecpan-keeper");
}
