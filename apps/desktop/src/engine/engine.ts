import { join } from "node:path";

import { BackendType } from "@aztec-foundation/bb.js";
import { SponsoredFeePaymentMethod } from "@aztec-labs/aztec.js/fee";
import { createAztecNodeClient } from "@aztec-labs/aztec.js/node";
import { Fr } from "@aztec-labs/foundation/curves/bn254";
import { GrumpkinScalar } from "@aztec-labs/foundation/curves/grumpkin";
import { SponsoredFPCContract } from "@aztec-labs/noir-contracts.js/SponsoredFPC";
import type { AztecAddress } from "@aztec-labs/stdlib/aztec-address";
import { getContractInstanceFromInstantiationParams } from "@aztec-labs/stdlib/contract";
import type { AztecNode } from "@aztec-labs/stdlib/interfaces/client";
import type { Params, Results } from "@shared/contract.ts";
import type { EngineInit, HostMethod, VaultRecord } from "@shared/engine-protocol.ts";
import { peerIdOf } from "@shared/peer-id.ts";
import type {
  AppEvent,
  AuditEntry,
  LobbyView,
  MemberReachability,
  NodeStatus,
  VaultDetails,
} from "@shared/types.ts";
import {
  KeeperClient,
  type KeeperEvent,
  KeeperPartialsSource,
  type VaultDeployment,
  VaultWallet,
  deriveVault,
  fromNapiPoint,
  generateEpochTaggingSecret,
  generateVaultProtocolSecrets,
  initBarretenberg,
} from "@tecpan/vault-wallet";

import { Founding } from "./founding.ts";
import { KeeperApi, type KeeperLobby, hex0x, lobbyView } from "./keeper-api.ts";
import { Operations } from "./operations.ts";
import { decodeSecrets, encodeSecrets } from "./secrets.ts";

const NODE_POLL_MS = 4000;

export interface EngineHost {
  emit(event: AppEvent): void;
  saveVault(record: VaultRecord): Promise<void>;
  adoptAccount(address: string): void;
  /** A call into the main process (e.g. the signer). */
  call(method: HostMethod, params: unknown): Promise<unknown>;
}

/** Rebuilds a vault's deployment from its sealed record (everything needed is in it). */
export async function deploymentOf(record: VaultRecord): Promise<VaultDeployment> {
  const { protocol } = decodeSecrets(record.secrets);
  return deriveVault({
    signerKeys: record.members.map((m) => fromNapiPoint(m.signerKey)),
    threshold: record.threshold,
    viewingThreshold: record.viewingThreshold,
    groupKey: fromNapiPoint(record.groupKey),
    verificationShares: record.verificationShares.map(fromNapiPoint),
    secrets: protocol,
    salt: Fr.fromString(record.salt),
  });
}

/**
 * The engine: one member's Aztec PXE (with the threshold key store), its connection to the member's keeper, and the
 * vault workflows built on both. Runs in its own `utilityProcess`; talks to the rest of the app only through
 * {@link EngineHost} and the calls the main process forwards.
 */
export class Engine {
  readonly #init: EngineInit;
  readonly #host: EngineHost;
  readonly #node: AztecNode;
  #keeper: KeeperApi | undefined;
  #wallet: Promise<VaultWallet> | undefined;
  #account: AztecAddress | undefined;
  #fee: { paymentMethod: SponsoredFeePaymentMethod } | undefined;
  #status: NodeStatus;
  #network: string | undefined;
  #timer: NodeJS.Timeout | undefined;
  readonly #vaults = new Map<string, VaultRecord>();
  readonly #deployments = new Map<string, Promise<VaultDeployment>>();
  readonly #foundings = new Map<string, Founding>();
  readonly #ops: Operations;

  constructor(init: EngineInit, host: EngineHost) {
    this.#init = init;
    this.#host = host;
    this.#node = createAztecNodeClient(init.nodeUrl);
    this.#status = { url: init.nodeUrl, reachable: false, checkedAt: 0 };
    for (const record of init.vaults) this.#vaults.set(record.address.toLowerCase(), record);
    this.#ops = new Operations(this, init.dataDir);
  }

  async start(): Promise<void> {
    if (this.#init.keeper) {
      const client = await KeeperClient.connect(this.#init.keeper.socket, this.#init.keeper.token);
      this.#keeper = new KeeperApi(client);
      await client.subscribe((event) => void this.#onKeeperEvent(event));
    }
    await this.#pollNode();
    this.#timer = setInterval(() => void this.#pollNode(), NODE_POLL_MS);
    // The PXE needs a reachable node; it starts in the background and every wallet call waits for it.
    if (this.#init.profile) void this.wallet().catch(() => undefined);
  }

  stop(): void {
    clearInterval(this.#timer);
  }

  get keeper(): KeeperApi {
    if (!this.#keeper) throw new Error("Your keeper is not running yet");
    return this.#keeper;
  }

  get profile() {
    if (!this.#init.profile) throw new Error("Create your profile first");
    return this.#init.profile;
  }

  get host(): EngineHost {
    return this.#host;
  }

  get account(): AztecAddress {
    if (!this.#account) throw new Error("Your Aztec account is not ready yet");
    return this.#account;
  }

  get fee() {
    if (!this.#fee) throw new Error("Your Aztec account is not ready yet");
    return this.#fee;
  }

  /** The PXE-backed wallet, created on first use (and again if creation failed). */
  wallet(): Promise<VaultWallet> {
    this.#wallet ??= this.#createWallet().catch((error: unknown) => {
      this.#wallet = undefined;
      throw error;
    });
    return this.#wallet;
  }

  async network(): Promise<string> {
    if (this.#network) return this.#network;
    const info = await this.#node.getNodeInfo();
    this.#network = `${info.l1ChainId}:${info.rollupVersion}`;
    return this.#network;
  }

  deployment(record: VaultRecord): Promise<VaultDeployment> {
    const key = record.address.toLowerCase();
    let deployment = this.#deployments.get(key);
    if (!deployment) {
      deployment = deploymentOf(record);
      this.#deployments.set(key, deployment);
    }
    return deployment;
  }

  async call<M extends keyof Results>(method: M, params: unknown): Promise<Results[M]> {
    return (await this.#dispatch(method, params)) as Results[M];
  }

  async #dispatch(method: keyof Results, params: unknown): Promise<unknown> {
    switch (method) {
      case "node.status":
        return this.#status;
      case "pxe.status": {
        const wallet = await this.wallet().catch(() => undefined);
        const header = await wallet?.pxeService.getSyncedBlockHeader().catch(() => undefined);
        return {
          syncedBlock: header ? Number(header.globalVariables.blockNumber) : undefined,
          proverEnabled: this.#init.proverEnabled,
          backend: "Barretenberg (native)",
          dataDir: join(this.#init.dataDir, "pxe"),
        };
      }
      case "lobby.inspect":
        return this.keeper.inspect(
          (params as Params<"lobby.inspect">).invite,
          await this.network(),
        );
      case "lobby.create": {
        const p = params as Params<"lobby.create">;
        const { lobby } = await this.keeper.open({
          name: p.name,
          threshold: p.threshold,
          viewingThreshold: p.viewingThreshold,
          network: await this.network(),
          member: { name: this.profile.name, signerKey: this.profile.signerKey },
        });
        return this.#lobby(lobby);
      }
      case "lobby.join": {
        const { invite } = params as Params<"lobby.join">;
        const preview = await this.keeper.inspect(invite, await this.network());
        if (!preview.sameNetwork)
          throw new Error(
            "This invite is for a different Aztec network than the one this app is connected to",
          );
        const { lobby } = await this.keeper.join(invite, {
          name: this.profile.name,
          signerKey: this.profile.signerKey,
        });
        return this.#lobby(lobby);
      }
      case "lobby.get":
        return this.#lobby((params as Params<"lobby.get">).lobby);
      case "lobby.list":
        return (await this.keeper.list()).map(lobbyView);
      case "lobby.start":
        return this.#startLobby(params as Params<"lobby.start">);
      case "lobby.leave": {
        const status = await this.keeper.status((params as Params<"lobby.leave">).lobby);
        await this.keeper.leave(status);
        return null;
      }
      case "founding.begin":
        return this.#found((params as Params<"founding.begin">).lobby);
      case "founding.get":
        return (
          this.#foundings.get((params as Params<"founding.get">).lobby.toLowerCase())?.state ?? null
        );
      case "vault.list":
        return [...this.#vaults.values()];
      case "vault.details":
        return this.#details((params as Params<"vault.details">).vault);
      case "vault.peers":
        return this.#peers((params as Params<"vault.peers">).vault);
      case "vault.activity":
        return this.#ops.activity(this.#record((params as Params<"vault.activity">).vault));
      case "tokens.list":
        return this.#ops.tokens(this.#record((params as Params<"tokens.list">).vault));
      case "tokens.add": {
        const p = params as Params<"tokens.add">;
        return this.#ops.addToken(this.#record(p.vault), p.token);
      }
      case "tokens.deployTest":
        return this.#ops.deployTestToken(
          this.#record((params as Params<"tokens.deployTest">).vault),
        );
      case "proposals.list":
        return this.#ops.proposals(this.#record((params as Params<"proposals.list">).vault));
      case "proposals.get": {
        const p = params as Params<"proposals.get">;
        return this.#ops.proposal(this.#record(p.vault), p.id);
      }
      case "proposals.preview": {
        const p = params as Params<"proposals.preview">;
        return this.#ops.preview(this.#record(p.vault), p.intent, p.expiresInHours);
      }
      case "proposals.create": {
        const p = params as Params<"proposals.create">;
        return this.#ops.create(this.#record(p.vault), p.intent, p.expiresInHours);
      }
      case "proposals.approve": {
        const p = params as Params<"proposals.approve">;
        return this.#ops.approve(this.#record(p.vault), p.id);
      }
      case "proposals.execute": {
        const p = params as Params<"proposals.execute">;
        return this.#ops.execute(this.#record(p.vault), p.id);
      }
      case "proposals.cancel": {
        const p = params as Params<"proposals.cancel">;
        return this.#ops.cancel(this.#record(p.vault), p.id);
      }
      case "contracts.functions":
        return this.#ops.functions((params as Params<"contracts.functions">).contract);
      case "vault.audit":
        return this.#audit(params as Params<"vault.audit">);
      default:
        throw new Error(`The engine does not handle ${method}`);
    }
  }

  async #createWallet(): Promise<VaultWallet> {
    try {
      await initBarretenberg(BackendType.NativeUnixSocket);
    } catch {
      await initBarretenberg(BackendType.Wasm);
    }
    const wallet = await VaultWallet.create(this.#node, {
      pxe: {
        dataDirectory: join(this.#init.dataDir, "pxe"),
        proverEnabled: this.#init.proverEnabled,
      },
    });
    const fpc = await getContractInstanceFromInstantiationParams(SponsoredFPCContract.artifact, {
      salt: new Fr(0n),
    });
    await wallet.registerContract(fpc, SponsoredFPCContract.artifact);
    this.#fee = { paymentMethod: new SponsoredFeePaymentMethod(fpc.address) };
    if (this.#init.account) {
      const { secret, salt, signingKey } = this.#init.account;
      this.#account = await wallet.addSchnorrAccount(
        Fr.fromString(secret),
        Fr.fromString(salt),
        GrumpkinScalar.fromString(signingKey),
      );
      this.#host.adoptAccount(this.#account.toString());
    }
    // Sequential on purpose: the PXE registers one account at a time.
    for (const record of this.#vaults.values()) {
      // oxlint-disable-next-line no-await-in-loop
      await this.registerVault(wallet, record);
    }
    return wallet;
  }

  /** Registers a founded vault in the PXE, viewing through this member's keeper. */
  async registerVault(wallet: VaultWallet, record: VaultRecord): Promise<void> {
    const deployment = await this.deployment(record);
    await wallet.registerVault(deployment, {
      epoch: 0,
      verificationShares: record.verificationShares.map(fromNapiPoint),
      viewingThreshold: record.viewingThreshold,
      source: new KeeperPartialsSource(this.keeper.client),
      epochTaggingSecret: decodeSecrets(record.secrets).epochTaggingSecret,
    });
  }

  async adoptVault(record: VaultRecord): Promise<void> {
    this.#vaults.set(record.address.toLowerCase(), record);
    await this.#host.saveVault(record);
  }

  async #lobby(lobby: string): Promise<LobbyView> {
    return lobbyView(await this.keeper.status(lobby));
  }

  async #startLobby(p: Params<"lobby.start">): Promise<LobbyView> {
    const secrets = encodeSecrets({
      protocol: await generateVaultProtocolSecrets(),
      epochTaggingSecret: await generateEpochTaggingSecret(),
    });
    const started = await this.keeper.start({
      lobby: p.lobby,
      threshold: p.threshold,
      viewingThreshold: p.viewingThreshold,
      salt: Fr.random().toString(),
      secrets,
    });
    void this.#found(p.lobby).catch(() => undefined);
    return lobbyView(started);
  }

  async #found(lobby: string) {
    const key = lobby.toLowerCase();
    let founding = this.#foundings.get(key);
    if (!founding || founding.state.error) {
      founding = new Founding(this, lobby);
      this.#foundings.set(key, founding);
      void founding.run();
    }
    return founding.state;
  }

  async #onKeeperEvent(event: KeeperEvent): Promise<void> {
    if (event.type === "proposal" || event.type === "approval" || event.type === "receipt") {
      // Records replicated from other members: tell the renderer which vault changed (all, if the event has none).
      const vaults =
        typeof event.vault === "string" ? [hex0x(event.vault)] : [...this.#vaults.keys()];
      for (const vault of vaults) this.#host.emit({ type: "vault-activity", vault });
      return;
    }
    if (event.type !== "lobby" || typeof event.lobby !== "string") return;
    try {
      const status: KeeperLobby = await this.keeper.status(event.lobby);
      this.#host.emit({ type: "lobby", lobby: lobbyView(status) });
    } catch {
      // A lobby that disappeared (closed by its host) has nothing more to show.
    }
  }

  async #probe(url: string): Promise<NodeStatus> {
    const node = createAztecNodeClient(url);
    try {
      const [info, block] = await Promise.all([node.getNodeInfo(), node.getBlockNumber()]);
      return {
        url,
        reachable: true,
        nodeVersion: info.nodeVersion,
        l1ChainId: info.l1ChainId,
        rollupVersion: Number(info.rollupVersion),
        blockNumber: Number(block),
        checkedAt: Date.now(),
      };
    } catch (error) {
      return {
        url,
        reachable: false,
        checkedAt: Date.now(),
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }

  async #pollNode(): Promise<void> {
    const status = await this.#probe(this.#init.nodeUrl);
    const changed =
      status.reachable !== this.#status.reachable ||
      status.blockNumber !== this.#status.blockNumber;
    this.#status = status;
    if (status.reachable && status.l1ChainId !== undefined)
      this.#network = `${status.l1ChainId}:${status.rollupVersion}`;
    if (changed) this.#host.emit({ type: "node", node: status });
  }

  get node(): AztecNode {
    return this.#node;
  }

  async #peers(vault: string): Promise<MemberReachability[]> {
    const record = this.#record(vault);
    const peers = await this.keeper.client.call<Omit<MemberReachability, "name">[]>(
      "peers.status",
      {
        vault: record.address,
      },
    );
    return peers.map((p) =>
      Object.assign(p, { name: record.members[p.index - 1]?.name ?? `Member ${p.index}` }),
    );
  }

  #record(vault: string): VaultRecord {
    const record = this.#vaults.get(vault.toLowerCase());
    if (!record) throw new Error("This vault is not on this device");
    return record;
  }

  async #details(vault: string): Promise<VaultDetails> {
    const record = this.#record(vault);
    const deployment = await this.deployment(record);
    const keeperVault = await this.keeper.vault(record.address).catch(() => undefined);
    let config: VaultDetails["config"] = null;
    try {
      const wallet = await this.wallet();
      const onChain = await wallet.getVaultConfig(deployment.address);
      config = {
        version: onChain.version,
        threshold: onChain.threshold,
        viewingThreshold: onChain.viewingThreshold,
        signerCount: onChain.signerCount,
        signerSetCommitment: onChain.signerSetCommitment.toString(),
        committeeCommitment: onChain.committeeCommitment.toString(),
        matchesSetup:
          onChain.signerSetCommitment.equals(deployment.signerSetCommitment) &&
          onChain.committeeCommitment.equals(deployment.committeeCommitment),
        readAt: Date.now(),
      };
    } catch {
      config = null;
    }
    const keys = deployment.completeAddress.publicKeys;
    const manifest = keeperVault?.manifest;
    return {
      summary: {
        address: record.address,
        name: record.name,
        threshold: record.threshold,
        viewingThreshold: record.viewingThreshold,
        members: record.members.length,
        myIndex: record.myIndex,
        foundedAt: record.foundedAt,
      },
      contract: {
        address: deployment.address.toString(),
        preaddress: deployment.preaddress.toString(),
        partialAddress: deployment.partialAddress.toString(),
        classId: deployment.instance.currentContractClassId.toString(),
        salt: deployment.instance.salt.toString(),
        deployer: deployment.instance.deployer.toString(),
        initializationHash: deployment.instance.initializationHash.toString(),
        publicKeys: {
          "Nullifier key hash": keys.npkMHash.toString(),
          "Incoming viewing key (group key)": keys.ivpkM.toString(),
          "Outgoing viewing key hash": keys.ovpkMHash.toString(),
          "Tagging key hash": keys.tpkMHash.toString(),
          "Message-signing key hash": keys.mspkMHash.toString(),
          "Fallback key hash": keys.fbpkMHash.toString(),
        },
      },
      config,
      committee: {
        epoch: manifest?.epoch ?? 0,
        viewingThreshold: record.viewingThreshold,
        groupKey: record.groupKey,
        serving: keeperVault?.serving ?? false,
        keeperStatus: keeperVault?.status ?? "unknown",
        members: record.members.map((member, i) => ({
          index: i + 1,
          name: member.name,
          signerKey: member.signerKey,
          verificationShare: record.verificationShares[i] ?? { x: "0x0", y: "0x0" },
          identity: member.identity,
          transport: member.transport,
          peerId: peerIdOf(member.identity),
          addresses: manifest?.members[i]?.addresses ?? member.addresses,
          you: i + 1 === record.myIndex,
        })),
      },
      founding: {
        session: record.session,
        transcript: record.transcript,
        deployTx: record.deployTx,
      },
    };
  }

  async #audit({ vault, limit }: Params<"vault.audit">): Promise<AuditEntry[]> {
    const record = this.#record(vault);
    const names = new Map(
      record.members.map((m) => [m.identity.replace(/^0x/u, "").toLowerCase(), m.name]),
    );
    const entries = await this.keeper.audit(record.address, limit);
    return entries.map((entry) =>
      Object.assign(entry, {
        requesterName:
          entry.requester === "local"
            ? "You"
            : names.get(entry.requester.replace(/^0x/u, "").toLowerCase()),
      }),
    );
  }
}
