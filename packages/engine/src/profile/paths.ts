import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** Where everything of one profile lives (architecture/cli.md, "Profiles and files"). */
export class ProfilePaths {
  readonly dir: string;

  constructor(dir: string) {
    this.dir = resolve(dir);
  }

  get name(): string {
    return basename(this.dir);
  }

  get profile(): string {
    return join(this.dir, "profile.json");
  }

  get secrets(): string {
    return join(this.dir, "secrets.json");
  }

  get signer(): string {
    return join(this.dir, "signer.sealed");
  }

  get vaults(): string {
    return join(this.dir, "vaults");
  }

  vault(address: string): string {
    return join(this.vaults, `${address.toLowerCase()}.json`);
  }

  get engine(): string {
    return join(this.dir, "engine");
  }

  get keeper(): string {
    return join(this.dir, "keeper");
  }

  /**
   * `<dir>/daemon.sock`, or a temp-dir socket when that path is too long for a unix socket (104 bytes on macOS). The
   * daemon writes the path it uses to `daemon.addr`, which clients read first.
   */
  get socket(): string {
    const local = join(this.dir, "daemon.sock");
    if (Buffer.byteLength(local) <= 100) return local;
    const id = createHash("sha256").update(this.dir).digest("hex").slice(0, 12);
    return join(tmpdir(), `tecpand-${id}.sock`);
  }

  get addr(): string {
    return join(this.dir, "daemon.addr");
  }

  get token(): string {
    return join(this.dir, "daemon.token");
  }

  get pid(): string {
    return join(this.dir, "daemon.pid");
  }

  get keeperLog(): string {
    return join(this.dir, "keeper.log");
  }

  /** The keeper socket lives in the temp dir: macOS limits socket paths to 104 bytes. */
  get keeperSocket(): string {
    const id = createHash("sha256").update(this.dir).digest("hex").slice(0, 12);
    return join(tmpdir(), `tecpan-${id}.sock`);
  }
}

/** `TECPAN_KEEPER_BIN`, else `$TECPAN_HOME/bin/tecpan-keeper`, else the repository's release or debug build. */
export function keeperBinary(): string {
  if (process.env.TECPAN_KEEPER_BIN) return process.env.TECPAN_KEEPER_BIN;
  const home = process.env.TECPAN_HOME ?? join(homedir(), ".tecpan");
  const installed = join(home, "bin", "tecpan-keeper");
  if (existsSync(installed)) return installed;
  // dist/tecpand.js → packages/engine → repository root.
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
  const builds = ["release", "debug"].map((p) =>
    join(root, "crates", "target", p, "tecpan-keeper"),
  );
  return builds.find((path) => existsSync(path)) ?? installed;
}
