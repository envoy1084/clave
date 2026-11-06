/* oxlint-disable no-await-in-loop -- the founding steps poll the keeper and the chain in sequence by design. */
import { Fr } from "@aztec-labs/foundation/curves/bn254";
import {
  type KeeperDkgResult,
  deriveVault,
  fromNapiPoint,
  keeperManifest,
} from "@tecpan/vault-wallet";

import type { FoundingState, FoundingStep, FoundingStepId } from "../types.ts";
import type { Engine } from "./engine.ts";
import { type KeeperLobby, participantOf } from "./keeper-api.ts";
import type { VaultRecord } from "./protocol.ts";
import { decodeSecrets } from "./secrets.ts";

const ORDER: FoundingStepId[] = ["agree", "dkg", "derive", "register", "deploy", "verify"];
const DKG_TIMEOUT_MS = 6 * 60_000;
const DEPLOY_WAIT_MS = 15 * 60_000;

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

class StepError extends Error {
  readonly blamed: readonly string[] | undefined;

  constructor(message: string, blamed?: readonly string[]) {
    super(message);
    this.blamed = blamed;
  }
}

/**
 * Turns a started lobby into a deployed, verified vault on this member's device: the DKG through the keeper, the
 * vault's address from the agreed setup, registration with the keeper and the PXE, deployment by the host (the others
 * wait for it on chain) and verification of the on-chain config. Every member runs the same steps independently and
 * arrives at the same address.
 */
export class Founding {
  readonly #engine: Engine;
  readonly #lobby: string;
  #steps: FoundingStep[] = ORDER.map((id) => ({ id, status: "pending" }));
  #vault: string | undefined;
  #error: FoundingState["error"];
  #done = false;

  constructor(engine: Engine, lobby: string) {
    this.#engine = engine;
    this.#lobby = lobby;
  }

  get state(): FoundingState {
    return {
      lobby: this.#lobby,
      steps: this.#steps,
      vault: this.#vault,
      error: this.#error,
      done: this.#done,
    };
  }

  async run(): Promise<void> {
    let current: FoundingStepId = "agree";
    try {
      current = "agree";
      this.#begin("agree");
      const lobby = await this.#plan();
      const plan = lobby.plan;
      if (!plan) throw new StepError("The host has not shared the vault plan yet");
      const me = lobby.members.find((m) => m.you);
      if (!me) throw new StepError("You are not a member of this vault's plan");
      const host = lobby.members.find((m) => m.index === 1);
      this.#finish(
        "agree",
        `n=${lobby.members.length}, k=${plan.threshold}, t_v=${plan.viewingThreshold}`,
      );

      current = "dkg";
      this.#begin("dkg", `Feldman rounds with ${lobby.members.length - 1} keepers`);
      const dkg = await this.#dkg(plan.session, plan.viewingThreshold, lobby);
      this.#finish("dkg", `transcript ${dkg.transcript.slice(0, 18)}…`);

      current = "derive";
      this.#begin("derive");
      const secrets = decodeSecrets(plan.secrets);
      const deployment = await deriveVault({
        signerKeys: lobby.members.map((m) => fromNapiPoint(m.signerKey)),
        threshold: plan.threshold,
        viewingThreshold: plan.viewingThreshold,
        groupKey: fromNapiPoint(dkg.groupKey),
        verificationShares: dkg.verificationShares.map(fromNapiPoint),
        secrets: secrets.protocol,
        salt: Fr.fromString(plan.salt),
      });
      this.#vault = deployment.address.toString();
      this.#finish("derive", this.#vault);

      current = "register";
      this.#begin("register");
      const participants = lobby.members.map(participantOf);
      await this.#engine.keeper.register(plan.session, keeperManifest(deployment, participants));
      let record: VaultRecord = {
        address: deployment.address.toString(),
        name: lobby.name,
        foundedAt: Date.now(),
        myIndex: me.index,
        lobby: this.#lobby,
        session: plan.session,
        threshold: plan.threshold,
        viewingThreshold: plan.viewingThreshold,
        salt: plan.salt,
        secrets: plan.secrets,
        groupKey: dkg.groupKey,
        verificationShares: dkg.verificationShares,
        members: lobby.members.map((m) => ({
          name: m.name,
          signerKey: m.signerKey,
          identity: m.identity,
          transport: m.transport,
          addresses: m.addresses,
        })),
        transcript: dkg.transcript,
      };
      const wallet = await this.#engine.wallet();
      await this.#engine.registerVault(wallet, record);
      this.#finish("register", "vault.register on the keeper; account and committee in the PXE");

      current = "deploy";
      if (me.index === 1) {
        this.#begin("deploy", "Proving and sending the private constructor");
        const receipt = await wallet.deployVault(deployment.address, {
          from: this.#engine.account,
          fee: this.#engine.fee,
        });
        record = { ...record, deployTx: receipt.txHash.toString() };
        this.#finish("deploy", receipt.txHash.toString());
      } else {
        this.#begin("deploy", `Waiting for ${host?.name ?? "the host"} to deploy`);
        await this.#waitForDeployment(() => wallet.verifyVaultConfig(deployment.address));
        this.#finish("deploy", "Deployed by member 1");
      }

      current = "verify";
      this.#begin("verify");
      const config = await wallet.verifyVaultConfig(deployment.address);
      await this.#engine.adoptVault(record);
      this.#finish(
        "verify",
        `config v${config.version}: commitments and thresholds match the plan`,
      );
      this.#done = true;
      this.#emit();
    } catch (error) {
      this.#fail(current, error);
    }
  }

  /** The started lobby with its plan (a guest may get here a moment before the plan arrives). */
  async #plan(): Promise<KeeperLobby> {
    for (let i = 0; i < 120; i++) {
      const lobby = await this.#engine.keeper.status(this.#lobby);
      if (lobby.plan) return lobby;
      if (lobby.status === "closed") throw new StepError("The host closed this lobby");
      await sleep(500);
    }
    throw new StepError("The host has not shared the vault plan yet");
  }

  async #dkg(session: string, threshold: number, lobby: KeeperLobby): Promise<KeeperDkgResult> {
    const keeper = this.#engine.keeper;
    const existing = await keeper.ceremony(session).catch(() => undefined);
    if (!existing) await keeper.startDkg(session, threshold, lobby.members.map(participantOf));
    const deadline = Date.now() + DKG_TIMEOUT_MS;
    while (Date.now() < deadline) {
      const ceremony = await keeper.ceremony(session);
      if (ceremony.status === "completed" && ceremony.result) return ceremony.result;
      if (ceremony.status === "aborted") {
        const blamed = (ceremony.failure?.blamed ?? []).map(
          (identity) =>
            lobby.members.find(
              (m) => m.identity.replace(/^0x/u, "") === identity.replace(/^0x/u, ""),
            )?.name ?? identity,
        );
        throw new StepError(
          `The key ceremony stopped: ${ceremony.failure?.reason ?? "unknown reason"}`,
          blamed,
        );
      }
      await sleep(400);
    }
    throw new StepError("The key ceremony took too long; some co-founders may be offline");
  }

  async #waitForDeployment(check: () => Promise<unknown>): Promise<void> {
    const deadline = Date.now() + DEPLOY_WAIT_MS;
    while (Date.now() < deadline) {
      try {
        await check();
        return;
      } catch {
        await sleep(3000);
      }
    }
    throw new StepError(
      "The vault did not appear on chain; ask the host whether their deployment went through",
    );
  }

  #begin(id: FoundingStepId, detail?: string): void {
    this.#update(id, { status: "active", startedAt: Date.now(), detail });
  }

  #finish(id: FoundingStepId, detail?: string): void {
    this.#update(id, { status: "done", finishedAt: Date.now(), detail });
  }

  #fail(id: FoundingStepId, error: unknown): void {
    const message = error instanceof Error ? error.message : String(error);
    this.#update(id, { status: "failed", finishedAt: Date.now() });
    this.#error = {
      step: id,
      message,
      blamed: error instanceof StepError ? error.blamed : undefined,
    };
    this.#emit();
  }

  #update(id: FoundingStepId, change: Partial<FoundingStep>): void {
    this.#steps = this.#steps.map((step) => (step.id === id ? { ...step, ...change } : step));
    this.#emit();
  }

  #emit(): void {
    this.#engine.host.emit({ type: "founding", founding: this.state });
  }
}
