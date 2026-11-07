import { SchnorrInitializerlessAccountContract } from "@aztec-labs/accounts/schnorr";
import {
  StubSchnorrAccountContractArtifact,
  createStubSchnorrAccount,
} from "@aztec-labs/accounts/schnorr/stub";
import type { Account } from "@aztec-labs/aztec.js/account";
import { NO_FROM, type NoFrom } from "@aztec-labs/aztec.js/account";
import {
  computeAuthWitMessageHash,
  type CallIntent,
  type IntentInnerHash,
} from "@aztec-labs/aztec.js/authorization";
import {
  Contract,
  ContractFunctionInteraction,
  type InteractionWaitOptions,
  type SendReturn,
} from "@aztec-labs/aztec.js/contracts";
import type {
  Aliased,
  ExecuteUtilityOptions,
  SendOptions,
  SimulateOptions,
  TxSimulationResultWithAppOffset,
} from "@aztec-labs/aztec.js/wallet";
import { AccountManager } from "@aztec-labs/aztec.js/wallet";
import {
  AccountFeePaymentMethodOptions,
  DefaultAccountEntrypoint,
} from "@aztec-labs/entrypoints/account";
import { poseidon2Hash } from "@aztec-labs/foundation/crypto/poseidon";
import { Schnorr } from "@aztec-labs/foundation/crypto/schnorr";
import { Fr } from "@aztec-labs/foundation/curves/bn254";
import type { GrumpkinScalar, Point } from "@aztec-labs/foundation/curves/grumpkin";
import { type Logger, createLogger } from "@aztec-labs/foundation/log";
import type { AccountPrivacyKeys } from "@aztec-labs/key-store";
import {
  DEFAULT_TAGGING_SECRET_STRATEGY,
  type PXE,
  type PXEConfig,
  type ResolveTaggingSecretStrategy,
  type TaggingSecretStrategy,
  type TaggingSecretStrategyRequest,
  createPXE,
  getPXEConfig,
} from "@aztec-labs/pxe/server";
import type { FunctionCall } from "@aztec-labs/stdlib/abi";
import { AuthWitness } from "@aztec-labs/stdlib/auth-witness";
import type { AztecAddress } from "@aztec-labs/stdlib/aztec-address";
import { getContractClassFromArtifact } from "@aztec-labs/stdlib/contract";
import type { GasSettings } from "@aztec-labs/stdlib/gas";
import type { AztecNode } from "@aztec-labs/stdlib/interfaces/client";
import { AppTaggingSecretKind } from "@aztec-labs/stdlib/logs";
import {
  type ExecutionPayload,
  SimulationOverrides,
  type TxExecutionRequest,
  type TxReceipt,
  type TxSimulationResult,
  type UtilityExecutionResult,
} from "@aztec-labs/stdlib/tx";
import { BaseWallet, type FeeOptions } from "@aztec-labs/wallet-sdk/base-wallet";

import { toNapiPoint } from "../keys/points.ts";
import { type PartialsSource, ThresholdEcdhProvider } from "../keys/threshold-ecdh.ts";
import { ThresholdKeyStore } from "../keys/threshold-key-store.ts";
import { VaultAccountContract } from "../vault/account.ts";
import { type VaultApproval, approvalMessage, encodeVaultWitness } from "../vault/approvals.ts";
import { vaultArtifact } from "../vault/artifact.ts";
import { type VaultConfig, readSignerKeys, readVaultConfig } from "../vault/config.ts";
import type { VaultDeployment } from "../vault/deployment.ts";
import { thresholdAccountKeys } from "../vault/keys.ts";

/** Options for {@link VaultWallet.create}. */
export interface VaultWalletOptions {
  /** PXE config overrides. Without `dataDirectory` the PXE runs on a temporary store. */
  readonly pxe?: Partial<PXEConfig>;
  /** Tagging strategy for everything except a vault's unconstrained self-sends. Defaults to Aztec's default. */
  readonly taggingStrategy?: ResolveTaggingSecretStrategy;
  readonly logger?: Logger;
}

/** How this member's PXE obtains a vault's viewing operations for the current committee epoch. */
export interface VaultViewing {
  /** Committee epoch: the vault config version the shares belong to. */
  readonly epoch: number;
  /** Verification shares `Y_i` of the current committee, in member order. */
  readonly verificationShares: readonly Point[];
  /** Viewing threshold `t_v` of the current committee. */
  readonly viewingThreshold: number;
  readonly source: PartialsSource;
  /** Per-epoch tagging secret for the vault's unconstrained self-sends. */
  readonly epochTaggingSecret?: Point;
  /** Called with members whose partials failed verification. */
  readonly onInvalidMembers?: (members: number[]) => void;
}

/** A vault transaction ready for approval: the exact payload, nonce and hash members sign. */
export interface VaultTx {
  readonly vault: AztecAddress;
  /** App calls, with the fee payment calls merged in. */
  readonly payload: ExecutionPayload;
  readonly txNonce: Fr;
  /** Hash the entrypoint asks the witness for. */
  readonly outerHash: Fr;
  readonly configVersion: number;
  readonly expiresAt: bigint;
  /** What each signer signs. */
  readonly approvalMessage: Fr;
  /** Ordered signer set the witness carries (read from the vault). */
  readonly signerKeys: readonly Point[];
}

/** An authwit request on behalf of a vault (e.g. `transfer_in_private`), ready for approval. */
export interface VaultAuthWitRequest {
  readonly vault: AztecAddress;
  readonly messageHash: Fr;
  readonly configVersion: number;
  readonly expiresAt: bigint;
  readonly approvalMessage: Fr;
  readonly signerKeys: readonly Point[];
}

interface VaultRecord {
  readonly deployment: VaultDeployment;
  readonly contract: VaultAccountContract;
  readonly account: Account;
  epochTaggingSecret: Point | undefined;
}

/**
 * Aztec wallet hosting a patched PXE whose {@link ThresholdKeyStore} operates Clave vaults without their incoming
 * viewing secret. Holds ordinary accounts (a member's own fee-paying account) and vaults; vault transactions go through
 * {@link prepareVaultTx} → approvals → {@link executeVaultTx}.
 */
export class VaultWallet extends BaseWallet {
  readonly keyStore: ThresholdKeyStore;
  readonly #accounts = new Map<string, Account>();
  readonly #vaults = new Map<string, VaultRecord>();
  readonly #pinnedNonces = new Map<string, Fr>();
  readonly #taggingStrategy: ResolveTaggingSecretStrategy | undefined;
  #stubClassId: Fr | undefined;

  private constructor(
    pxe: PXE,
    node: AztecNode,
    keyStore: ThresholdKeyStore,
    taggingStrategy: ResolveTaggingSecretStrategy | undefined,
    log: Logger,
  ) {
    super(pxe, node, log);
    this.keyStore = keyStore;
    this.#taggingStrategy = taggingStrategy;
  }

  /** Creates the PXE with the threshold key store and the vault tagging hook. */
  static async create(node: AztecNode, options: VaultWalletOptions = {}): Promise<VaultWallet> {
    const log = options.logger ?? createLogger("clave:vault-wallet");
    const holder: { wallet?: VaultWallet } = {};
    let keyStore: ThresholdKeyStore | undefined;
    const pxe = await createPXE(
      node,
      { ...getPXEConfig(), autoSync: false, ...options.pxe },
      {
        createKeyStore: (store) => {
          keyStore = new ThresholdKeyStore(store);
          return keyStore;
        },
        hooks: {
          resolveTaggingSecretStrategy: (request) => {
            if (!holder.wallet) return Promise.resolve(DEFAULT_TAGGING_SECRET_STRATEGY);
            return holder.wallet.#resolveTaggingStrategy(request);
          },
        },
        loggers: {
          pxe: log.createChild("pxe"),
          store: log.createChild("pxe:data"),
          prover: log.createChild("prover"),
        },
      },
    );
    if (!keyStore)
      throw new Error(
        "The PXE did not use the injected key store; is the @aztec-labs/pxe patch applied?",
      );
    const wallet = new VaultWallet(pxe, node, keyStore, options.taggingStrategy, log);
    holder.wallet = wallet;
    return wallet;
  }

  /** Stops the PXE's job queue. */
  stop(): Promise<void> {
    return this.pxe.stop();
  }

  /** The PXE (for tests and a host's sync loop). */
  get pxeService(): PXE {
    return this.pxe;
  }

  override getAccounts(): Promise<Aliased<AztecAddress>[]> {
    return Promise.resolve([
      ...[...this.#vaults.values()].map((v) => ({ alias: "vault", item: v.deployment.address })),
      ...[...this.#accounts.values()].map((a) => ({ alias: "account", item: a.getAddress() })),
    ]);
  }

  protected override getAccountFromAddress(address: AztecAddress): Promise<Account> {
    const account =
      this.#vaults.get(address.toString())?.account ?? this.#accounts.get(address.toString());
    if (!account)
      return Promise.reject(new Error(`Account ${address.toString()} is not held by this wallet`));
    return Promise.resolve(account);
  }

  /** Adds a member's own Schnorr account (initializerless, as genesis-funded test accounts are). */
  async addSchnorrAccount(secret: Fr, salt: Fr, signingKey: GrumpkinScalar): Promise<AztecAddress> {
    const contract = new SchnorrInitializerlessAccountContract(signingKey);
    const publicKey = await new Schnorr().computePublicKey(signingKey);
    const immutablesHash = await poseidon2Hash([publicKey.x, publicKey.y]);
    const manager = await AccountManager.create(this, secret, contract, { salt, immutablesHash });
    const instance = manager.getInstance();
    if (!(await this.pxe.getContractInstance(instance.address))) {
      const artifact = await contract.getContractArtifact();
      await this.registerContract(instance, artifact, secret);
      const constructor = artifact.functions.find((f) => f.name === "constructor");
      if (!constructor) throw new Error("Schnorr initializerless artifact has no constructor");
      await new ContractFunctionInteraction(this, instance.address, constructor, [
        publicKey.x,
        publicKey.y,
      ]).simulate({
        from: instance.address,
      });
    }
    const account = await manager.getAccount();
    this.#accounts.set(account.getAddress().toString(), account);
    return account.getAddress();
  }

  /**
   * Registers a vault in this PXE: its instance, its keys without the incoming viewing secret, and the viewing
   * committee that answers its ECDH requests.
   */
  async registerVault(deployment: VaultDeployment, viewing: VaultViewing): Promise<AztecAddress> {
    const { address, instance, partialAddress, setup } = deployment;
    await this.registerContract(instance, vaultArtifact());
    const keys = thresholdAccountKeys(setup.secrets, setup.groupKey);
    // The key store recognises threshold keys; PXE's type only names ordinary ones.
    const registered = await this.pxe.registerAccount(
      keys as unknown as AccountPrivacyKeys,
      partialAddress,
    );
    if (!registered.address.equals(address))
      throw new Error("Registered vault address does not match its instance");
    const contract = new VaultAccountContract(deployment);
    this.#vaults.set(address.toString(), {
      deployment,
      contract,
      account: contract.getAccount(deployment.completeAddress),
      epochTaggingSecret: undefined,
    });
    await this.setViewing(address, viewing);
    return address;
  }

  /** Points a vault's viewing operations at the committee of a new epoch (after a reshare). */
  async setViewing(vault: AztecAddress, viewing: VaultViewing): Promise<void> {
    const record = this.#vault(vault);
    const { deployment } = record;
    const committee = {
      vault,
      epoch: viewing.epoch,
      preaddress: deployment.preaddress.toString(),
      groupKey: toNapiPoint(deployment.setup.groupKey),
      verificationShares: viewing.verificationShares.map(toNapiPoint),
      threshold: viewing.viewingThreshold,
    };
    this.keyStore.setEcdhProvider(
      vault,
      new ThresholdEcdhProvider(committee, viewing.source, viewing.onInvalidMembers),
    );
    if (viewing.epochTaggingSecret) {
      record.epochTaggingSecret = viewing.epochTaggingSecret;
      await this.pxe.registerTaggingSecretSource({
        kind: "arbitrary-secret",
        recipient: vault,
        secret: viewing.epochTaggingSecret,
      });
    }
  }

  /**
   * Initializes a registered vault: `from` (a member's own account) sends the constructor with the vault in scope. The
   * vault needs no class or instance publication (it has no public functions). Returns the mined receipt.
   */
  async deployVault(
    vault: AztecAddress,
    options: {
      from: AztecAddress;
      fee?: Parameters<ContractFunctionInteraction["send"]>[0]["fee"];
    },
  ): Promise<TxReceipt> {
    const { deployment } = this.#vault(vault);
    const deploy = Contract.deploy(
      this,
      vaultArtifact(),
      deployment.constructorArgs,
      "constructor",
      {
        salt: deployment.setup.salt,
        publicKeys: deployment.completeAddress.publicKeys,
        universalDeploy: true,
      },
    );
    const { receipt } = await deploy.send({
      from: options.from,
      fee: options.fee,
      skipClassPublication: true,
      skipInstancePublication: true,
      skipRegistration: true,
      additionalScopes: [vault],
    });
    return receipt;
  }

  /** The vault's current config as stored on chain (read through this PXE). */
  async getVaultConfig(vault: AztecAddress): Promise<VaultConfig> {
    await this.pxe.sync();
    return readVaultConfig(this, vault);
  }

  /**
   * Reads the on-chain config and checks it against the agreed setup: same signer set and committee commitments and the
   * same thresholds. Members run this before funding or using a vault.
   */
  async verifyVaultConfig(vault: AztecAddress): Promise<VaultConfig> {
    const { deployment } = this.#vault(vault);
    const config = await this.getVaultConfig(vault);
    const { setup } = deployment;
    const mismatches = [
      config.signerSetCommitment.equals(deployment.signerSetCommitment) ? undefined : "signer set",
      config.committeeCommitment.equals(deployment.committeeCommitment) ? undefined : "committee",
      config.threshold === setup.threshold ? undefined : "threshold",
      config.viewingThreshold === setup.viewingThreshold ? undefined : "viewing threshold",
      config.signerCount === setup.signerKeys.length ? undefined : "signer count",
    ].filter(Boolean);
    if (mismatches.length > 0)
      throw new Error(`Vault config does not match the setup: ${mismatches.join(", ")}`);
    return config;
  }

  /**
   * Prepares a vault transaction: pins the nonce, merges the fee payment calls and computes the outer hash and the
   * approval message under the vault's current config version.
   */
  async prepareVaultTx(
    vault: AztecAddress,
    payload: ExecutionPayload,
    options: { expiresAt: bigint; txNonce?: Fr },
  ): Promise<VaultTx> {
    this.#vault(vault);
    const config = await this.getVaultConfig(vault);
    const signerKeys = await readSignerKeys(this, vault);
    const txNonce = options.txNonce ?? Fr.random();
    const outerHash = await this.#captureOuterHash(vault, payload, txNonce);
    return {
      vault,
      payload,
      txNonce,
      outerHash,
      configVersion: config.version,
      expiresAt: options.expiresAt,
      approvalMessage: approvalMessage(vault, outerHash, config.version, options.expiresAt),
      signerKeys,
    };
  }

  /**
   * Simulates a vault transaction before approvals exist, with the vault's class swapped for Aztec's stub account
   * (whose authorization always passes). Shows the effects of app calls; calls into the vault itself are not
   * previewable this way.
   */
  async previewVaultTx(tx: VaultTx): Promise<TxSimulationResult> {
    const { deployment } = this.#vault(tx.vault);
    await this.pxe.sync();
    const stubClassId = await this.#registerStubClass();
    const instance = await this.pxe.getContractInstance(tx.vault);
    if (!instance) throw new Error("Vault instance is not registered");
    const overrides = new SimulationOverrides({
      contracts: {
        [tx.vault.toString()]: { instance: { ...instance, currentContractClassId: stubClassId } },
      },
    });
    const feeOptions = await this.completeFeeOptions({
      from: tx.vault,
      feePayer: tx.payload.feePayer,
      forEstimation: true,
    });
    const request = await createStubSchnorrAccount(
      deployment.completeAddress,
    ).createTxExecutionRequest(
      tx.payload,
      feeOptions.gasSettings,
      await this.getChainInfo(),
      this.#entrypointOptions(tx.txNonce, feeOptions),
    );
    return this.pxe.simulateTx(request, {
      simulatePublic: true,
      overrides,
      scopes: [tx.vault],
      senderForTags: tx.vault,
    });
  }

  /** Assembles the witness from `approvals`, proves and sends the transaction, and waits for it to be mined. */
  async executeVaultTx(
    tx: VaultTx,
    approvals: readonly VaultApproval[],
    options: { gasSettings?: Partial<GasSettings> } = {},
  ): Promise<TxReceipt> {
    const record = this.#vault(tx.vault);
    const fields = encodeVaultWitness(tx.signerKeys, tx.approvalMessage, approvals, tx.expiresAt);
    record.contract.witnesses.provide(tx.outerHash, fields);
    this.#pinnedNonces.set(tx.vault.toString(), tx.txNonce);
    try {
      const { receipt } = await this.sendTx(tx.payload, {
        from: tx.vault,
        fee: options.gasSettings ? { gasSettings: options.gasSettings } : undefined,
      });
      return receipt;
    } finally {
      this.#pinnedNonces.delete(tx.vault.toString());
      record.contract.witnesses.revoke(tx.outerHash);
    }
  }

  /** Prepares an authwit the vault grants (e.g. to a token for `transfer_in_private`). */
  async prepareVaultAuthWit(
    vault: AztecAddress,
    intent: IntentInnerHash | CallIntent,
    options: { expiresAt: bigint },
  ): Promise<VaultAuthWitRequest> {
    this.#vault(vault);
    const config = await this.getVaultConfig(vault);
    const messageHash = await computeAuthWitMessageHash(intent, await this.getChainInfo());
    return {
      vault,
      messageHash,
      configVersion: config.version,
      expiresAt: options.expiresAt,
      approvalMessage: approvalMessage(vault, messageHash, config.version, options.expiresAt),
      signerKeys: await readSignerKeys(this, vault),
    };
  }

  /** The authwit for `request` carrying `approvals`, to attach to the consuming transaction. */
  createVaultAuthWit(
    request: VaultAuthWitRequest,
    approvals: readonly VaultApproval[],
  ): AuthWitness {
    const fields = encodeVaultWitness(
      request.signerKeys,
      request.approvalMessage,
      approvals,
      request.expiresAt,
    );
    return new AuthWitness(request.messageHash, fields);
  }

  // The PXE runs with auto-sync off; every operation syncs first so it anchors on the latest block.

  override async simulateTx(
    executionPayload: ExecutionPayload,
    opts: SimulateOptions,
  ): Promise<TxSimulationResultWithAppOffset> {
    await this.pxe.sync();
    return super.simulateTx(executionPayload, opts);
  }

  override async executeUtility(
    call: FunctionCall,
    opts: ExecuteUtilityOptions,
  ): Promise<UtilityExecutionResult> {
    await this.pxe.sync();
    return super.executeUtility(call, opts);
  }

  override async sendTx<W extends InteractionWaitOptions = undefined>(
    executionPayload: ExecutionPayload,
    opts: SendOptions<W>,
  ): Promise<SendReturn<W>> {
    await this.pxe.sync();
    return super.sendTx(executionPayload, opts);
  }

  /** Vault requests use the pinned proposal nonce and are always cancellable (cancellation burns the nonce). */
  protected override async createTxExecutionRequestFromPayloadAndFee(
    executionPayload: ExecutionPayload,
    from: AztecAddress | NoFrom,
    feeOptions: FeeOptions,
  ): Promise<TxExecutionRequest> {
    if (from === NO_FROM || !this.#vaults.has(from.toString())) {
      return super.createTxExecutionRequestFromPayloadAndFee(executionPayload, from, feeOptions);
    }
    const txNonce = this.#pinnedNonces.get(from.toString());
    if (!txNonce)
      throw new Error("Vault transactions go through prepareVaultTx and executeVaultTx");
    const account = await this.getAccountFromAddress(from);
    return account.createTxExecutionRequest(
      executionPayload,
      feeOptions.gasSettings,
      await this.getChainInfo(),
      this.#entrypointOptions(txNonce, feeOptions),
    );
  }

  #entrypointOptions(txNonce: Fr, feeOptions: FeeOptions) {
    return {
      txNonce,
      cancellable: true,
      feePaymentMethodOptions:
        feeOptions.accountFeePaymentMethodOptions ?? AccountFeePaymentMethodOptions.EXTERNAL,
    };
  }

  /** The entrypoint's outer hash for `payload`, computed by Aztec's own entrypoint code. */
  async #captureOuterHash(
    vault: AztecAddress,
    payload: ExecutionPayload,
    txNonce: Fr,
  ): Promise<Fr> {
    let captured: Fr | undefined;
    const capture = {
      createAuthWit: (messageHash: Fr) => {
        captured = messageHash;
        return Promise.resolve(new AuthWitness(messageHash, []));
      },
    };
    const feeOptions = await this.completeFeeOptions({
      from: vault,
      feePayer: payload.feePayer,
      forEstimation: true,
    });
    await new DefaultAccountEntrypoint(vault, capture).createTxExecutionRequest(
      payload,
      feeOptions.gasSettings,
      await this.getChainInfo(),
      this.#entrypointOptions(txNonce, feeOptions),
    );
    if (!captured) throw new Error("Entrypoint did not request a witness");
    return captured;
  }

  async #registerStubClass(): Promise<Fr> {
    if (!this.#stubClassId) {
      await this.pxe.registerContractClass(StubSchnorrAccountContractArtifact);
      this.#stubClassId = (
        await getContractClassFromArtifact(StubSchnorrAccountContractArtifact)
      ).id;
    }
    return this.#stubClassId;
  }

  #resolveTaggingStrategy(request: TaggingSecretStrategyRequest): Promise<TaggingSecretStrategy> {
    const vault = this.#vaults.get(request.sender.toString());
    const selfSend = request.recipient.equals(request.sender);
    if (vault && selfSend && request.deliveryMode === AppTaggingSecretKind.UNCONSTRAINED) {
      // Unconstrained self-sends use the current epoch's secret, so members removed at a reshare stop finding them.
      return Promise.resolve(
        vault.epochTaggingSecret
          ? { type: "arbitrary-secret", secret: vault.epochTaggingSecret }
          : { type: "address-derived" },
      );
    }
    return this.#taggingStrategy?.(request) ?? Promise.resolve(DEFAULT_TAGGING_SECRET_STRATEGY);
  }

  #vault(vault: AztecAddress): VaultRecord {
    const record = this.#vaults.get(vault.toString());
    if (!record) throw new Error(`Vault ${vault.toString()} is not registered in this wallet`);
    return record;
  }
}
