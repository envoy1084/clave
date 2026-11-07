// Who answers the vault's threshold ECDH in the end-to-end suite: in-process shares, or real `clave-keeper` processes
// that ran a DKG among themselves. The suite runs once per backend.
import { execFileSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { existsSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import type { Fr } from "@aztec-labs/foundation/curves/bn254";
import { GrumpkinScalar, type Point } from "@aztec-labs/foundation/curves/grumpkin";

import {
  type KeeperCeremony,
  type KeeperDkgResult,
  type KeeperReshareResult,
  type KeeperVault,
  KeeperPartialsSource,
  type LaunchedKeeper,
  type PartialsSource,
  type VaultDeployment,
  fromNapiPoint,
  keeperManifest,
  launchKeeper,
  toNapiPoint,
} from "../../src/index.ts";
import {
  InProcessPartialsSource,
  type TestCommittee,
  dealTestCommittee,
} from "../../src/testing/index.ts";

/** A 2-of-3 viewing committee for members 1..3, resharable to members 1, 2 and a newcomer. */
export interface ViewingBackend {
  readonly name: string;
  /** Group key and verification shares of the current committee. */
  readonly groupKey: Point;
  verificationShares: Point[];
  /** Committee epoch the shares are bound to. */
  epoch: number;
  /** Binds the committee to the derived vault. */
  attach(deployment: VaultDeployment): Promise<void>;
  /** Where member `index`'s engine gets partials. */
  source(index: 1 | 2): PartialsSource;
  /** Member 3 leaves and a newcomer joins; returns the new committee's verification shares. */
  reshare(signerKeys: readonly Point[]): Promise<Point[]>;
  /** The rotation landed on chain at config `version`. */
  activate(version: number, committeeCommitment: Fr): Promise<void>;
  /** Checks that the departed member can no longer serve. */
  assertRemoved(): Promise<void>;
  stop(): Promise<void>;
}

export async function inProcessBackend(): Promise<ViewingBackend> {
  const secret = GrumpkinScalar.random(); // only this test dealer knows it, standing in for the DKG
  let committee: TestCommittee = dealTestCommittee(2, 3, secret);
  let next: TestCommittee | undefined;
  const sources: InProcessPartialsSource[] = [];
  const backend: ViewingBackend = {
    name: "in-process shares",
    groupKey: committee.groupKey,
    verificationShares: committee.verificationShares,
    epoch: 0,
    attach(deployment) {
      sources.push(
        new InProcessPartialsSource(committee, deployment.preaddress, [1, 2]),
        new InProcessPartialsSource(committee, deployment.preaddress, [2, 3]),
      );
      return Promise.resolve();
    },
    source: (index) => sources[index - 1] as InProcessPartialsSource,
    reshare() {
      next = dealTestCommittee(2, 3, secret);
      return Promise.resolve(next.verificationShares);
    },
    activate(version) {
      if (!next) throw new Error("no reshare to activate");
      committee = next;
      sources[0]?.rotate(committee, [1, 3]);
      sources[1]?.rotate(committee, [2, 3]);
      backend.verificationShares = committee.verificationShares;
      backend.epoch = version;
      return Promise.resolve();
    },
    assertRemoved: () => Promise.resolve(),
    stop: () => Promise.resolve(),
  };
  return backend;
}

const KEEPER_BINARY =
  process.env.CLAVE_KEEPER_BIN ??
  fileURLToPath(new URL("../../../../crates/target/debug/clave-keeper", import.meta.url));

function keeperBinary(): string {
  if (!existsSync(KEEPER_BINARY)) {
    const manifest = fileURLToPath(new URL("../../../../crates/Cargo.toml", import.meta.url));
    execFileSync("cargo", ["build", "--manifest-path", manifest, "-p", "clave-keeper"], {
      stdio: "inherit",
    });
  }
  return KEEPER_BINARY;
}

const session = () => `0x${randomBytes(32).toString("hex")}`;

async function until<T>(check: () => Promise<T | undefined>, what: string): Promise<T> {
  for (let i = 0; i < 600; i++) {
    // oxlint-disable-next-line no-await-in-loop
    const value = await check();
    if (value !== undefined) return value;
    // oxlint-disable-next-line no-await-in-loop
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`timed out waiting for ${what}`);
}

async function finished<R>(keeper: LaunchedKeeper, id: string): Promise<R> {
  const ceremony = await until(async () => {
    const status = await keeper.client.call<KeeperCeremony<R>>("ceremony.status", { session: id });
    if (status.status === "aborted") throw new Error(`ceremony aborted: ${status.failure?.reason}`);
    return status.status === "completed" ? status : undefined;
  }, `ceremony ${id}`);
  return ceremony.result as R;
}

const participant = (k: LaunchedKeeper) => ({
  identity: k.identity,
  transport: k.transport,
  addresses: k.addresses,
});

export async function keeperBackend(): Promise<ViewingBackend> {
  const binary = keeperBinary();
  const dirs = await Promise.all([1, 2, 3, 4].map((i) => mkdtemp(join(tmpdir(), `clave-k${i}-`))));
  const keepers = await Promise.all(
    dirs.map((dataDir) =>
      launchKeeper({ binary, dataDir, masterKey: randomBytes(32).toString("hex") }),
    ),
  );
  const [k1, k2, k3, k4] = keepers as [
    LaunchedKeeper,
    LaunchedKeeper,
    LaunchedKeeper,
    LaunchedKeeper,
  ];

  const dkg = session();
  await Promise.all(
    [k1, k2, k3].map((k) =>
      k.client.call("ceremony.dkg", {
        session: dkg,
        threshold: 2,
        members: [k1, k2, k3].map(participant),
      }),
    ),
  );
  const results = await Promise.all([k1, k2, k3].map((k) => finished<KeeperDkgResult>(k, dkg)));
  if (new Set(results.map((r) => r.transcript)).size !== 1)
    throw new Error("keepers disagree on the DKG transcript");
  const result = results[0] as KeeperDkgResult;

  let vault = "";
  let manifest: ReturnType<typeof keeperManifest> | undefined;
  let reshareSession = "";
  const backend: ViewingBackend = {
    name: "clave-keeper processes",
    groupKey: fromNapiPoint(result.groupKey),
    verificationShares: result.verificationShares.map(fromNapiPoint),
    epoch: 0,
    async attach(deployment) {
      vault = deployment.address.toString();
      manifest = keeperManifest(deployment, [k1, k2, k3].map(participant));
      await Promise.all(
        [k1, k2, k3].map((k) => k.client.call("vault.register", { session: dkg, manifest })),
      );
    },
    source: (index) => new KeeperPartialsSource((index === 1 ? k1 : k2).client),
    async reshare(signerKeys) {
      reshareSession = session();
      const request = {
        session: reshareSession,
        vault,
        dealers: [1, 2],
        newThreshold: 2,
        newSpendingThreshold: 2,
        newMembers: [k1, k2, k4].map((k, i) => ({
          identity: k.identity,
          transport: k.transport,
          addresses: k.addresses,
          signerKey: toNapiPoint(signerKeys[i] as Point),
        })),
      };
      await Promise.all([
        k1.client.call("ceremony.reshare", request),
        k2.client.call("ceremony.reshare", request),
        k4.client.call("ceremony.reshare", { ...request, current: manifest }),
      ]);
      const reshared = await Promise.all(
        [k1, k2, k4].map((k) => finished<KeeperReshareResult>(k, reshareSession)),
      );
      const next = reshared[0]?.manifest;
      if (!next) throw new Error("reshare produced no manifest");
      return next.members.map((m) => fromNapiPoint(m.verificationShare));
    },
    async activate(version, committeeCommitment) {
      const observe = (k: LaunchedKeeper) =>
        k.client.call<{ observation: string }>("vault.observeConfig", {
          vault,
          version,
          committeeCommitment: committeeCommitment.toString(),
        });
      const observed = await Promise.all([k1, k2, k3, k4].map(observe));
      const kinds = observed.map((o) => o.observation);
      if (kinds.join() !== "activated,activated,removed,activated")
        throw new Error(`unexpected activation ${kinds}`);
      const status = await k1.client.call<KeeperVault>("vault.status", { vault });
      this.verificationShares = status.manifest.members.map((m) =>
        fromNapiPoint(m.verificationShare),
      );
      this.epoch = version;
    },
    async assertRemoved() {
      const status = await k3.client.call<KeeperVault>("vault.status", { vault });
      if (status.status !== "removed" || status.serving)
        throw new Error("departed keeper still serves");
      const refused = await k3.client
        .call("ecdh.batch", {
          vault,
          epoch: this.epoch,
          purpose: "decryption",
          points: [toNapiPoint(this.groupKey)],
        })
        .then(
          () => false,
          () => true,
        );
      if (!refused) throw new Error("departed keeper answered an ECDH batch");
    },
    async stop() {
      await Promise.all(keepers.map((k) => k.stop()));
      await Promise.all(dirs.map((dir) => rm(dir, { recursive: true, force: true })));
    },
  };
  return backend;
}
