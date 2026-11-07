import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

import { Contract } from "@aztec-labs/aztec.js/contracts";
import { readFieldCompressedString } from "@aztec-labs/aztec.js/utils";
import { Fr } from "@aztec-labs/foundation/curves/bn254";
import { TokenContract } from "@aztec-labs/noir-contracts.js/Token";
import type { AbiType, ContractArtifact } from "@aztec-labs/stdlib/abi";
import { AztecAddress } from "@aztec-labs/stdlib/aztec-address";
import { type ExecutionPayload, mergeExecutionPayloads } from "@aztec-labs/stdlib/tx";
import {
  type Approval as WireApproval,
  type Proposal as WireProposal,
  decodeApproval,
  decodeProposal,
  decodeRejection,
  encodeApproval,
  encodeProposal,
  encodeRejection,
  proposalApprovalMessage,
  proposalId,
  proposalRejectionMessage,
  verifyProposalApproval,
  verifyProposalRejection,
} from "@clave/napi";
import {
  ProposalMismatchError,
  type VaultWallet,
  decodePayload,
  proposalOf,
  vaultApprovalsOf,
  vaultTxOf,
} from "@clave/vault-wallet";

import type { Secret } from "../rpc/contract.ts";
import { DaemonError } from "../rpc/errors.ts";
import type {
  ActivityEntry,
  ContractFunction,
  DecodedCall,
  ProposalIntent,
  ProposalPreview,
  ProposalStatus,
  ProposalView,
  Review,
  TokenBalance,
  TokenInfo,
  Vote,
} from "../types.ts";
import type { Engine } from "./engine.ts";
import type { VaultRecord } from "./protocol.ts";

/** Sponsored fee payment (the only method until the paymaster). */
const FEE_SPONSORED = 1;
const CONFIG_TTL_MS = 8000;
const TEST_TOKEN_SUPPLY = 10_000n * 10n ** 18n;

const hex = (bytes: Uint8Array) => `0x${Buffer.from(bytes).toString("hex")}`;
const fromHex = (value: string) => Buffer.from(value.replace(/^0x/u, ""), "hex");
const addressOf = (value: string) => AztecAddress.schema.parse(value);

interface Receipt {
  readonly proposal: string;
  readonly txHash: string;
  readonly at: number;
}

/**
 * Everyday vault operations on this member's device: assets, proposals, approvals, execution and activity. Proposals
 * and approvals live in the member's keeper (which replicates them to the others); the PXE rebuilds and checks every
 * proposal before this member signs or executes it.
 */
export class Operations {
  readonly #engine: Engine;
  readonly #tokensFile: string;
  #tokens: Record<string, TokenInfo[]> | undefined;
  readonly #config = new Map<string, { version: number; at: number }>();
  readonly #artifacts = new Map<string, ContractArtifact>();

  constructor(engine: Engine, dataDir: string) {
    this.#engine = engine;
    this.#tokensFile = join(dataDir, "tokens.json");
  }

  // Assets ---------------------------------------------------------------------------------------------------------

  async tokens(vault: VaultRecord): Promise<TokenBalance[]> {
    const wallet = await this.#engine.wallet();
    const owner = addressOf(vault.address);
    const known = (await this.#loadTokens())[vault.address.toLowerCase()] ?? [];
    return Promise.all(
      known.map(async (token) => {
        try {
          const { result } = await TokenContract.at(addressOf(token.address), wallet)
            .methods.balance_of_private(owner)
            .simulate({ from: owner });
          return { token, balance: String(result as bigint) };
        } catch (error) {
          return { token, error: error instanceof Error ? error.message : String(error) };
        }
      }),
    );
  }

  async addToken(vault: VaultRecord, token: string): Promise<TokenInfo> {
    const info = await this.#registerToken(token);
    await this.#rememberToken(vault, info);
    return info;
  }

  /** Local networks only: deploys a test token from this member's account and mints it privately to the vault. */
  async deployTestToken(vault: VaultRecord): Promise<TokenInfo> {
    if (this.#engine.nodeStatus.l1ChainId !== 31337)
      throw new DaemonError("invalid", "Test tokens are only for a local network (L1 chain 31337)");
    const wallet = await this.#engine.wallet();
    const from = this.#engine.account;
    const fee = this.#engine.fee;
    const { contract } = await TokenContract.deploy(
      wallet,
      from,
      "Clave Test Token",
      "TTT",
      18,
    ).send({ from, fee });
    await contract.methods
      .mint_to_private(addressOf(vault.address), TEST_TOKEN_SUPPLY)
      .send({ from, fee });
    const info: TokenInfo = {
      address: contract.address.toString(),
      name: "Clave Test Token",
      symbol: "TTT",
      decimals: 18,
    };
    await this.#rememberToken(vault, info);
    this.#engine.host.emit({ type: "vault-activity", vault: vault.address });
    return info;
  }

  // Proposals ------------------------------------------------------------------------------------------------------

  async preview(
    vault: VaultRecord,
    intent: ProposalIntent,
    hours: number,
  ): Promise<ProposalPreview> {
    const wallet = await this.#engine.wallet();
    const payload = await this.#payload(intent);
    const calls = await this.#decodeCalls(vault, payload);
    try {
      const tx = await wallet.prepareVaultTx(addressOf(vault.address), payload, {
        expiresAt: (await this.#chainTime()) + BigInt(hours * 3600),
      });
      const simulation = await wallet.previewVaultTx(tx);
      const gas = simulation.gasUsed?.totalGas;
      return {
        ok: true,
        calls,
        gas: gas ? { da: Number(gas.daGas), l2: Number(gas.l2Gas) } : undefined,
      };
    } catch (error) {
      return { ok: false, calls, error: error instanceof Error ? error.message : String(error) };
    }
  }

  async create(vault: VaultRecord, intent: ProposalIntent, hours: number): Promise<ProposalView> {
    if (intent.kind === "transfer") await this.addToken(vault, intent.token);
    const payload = await this.#payload(intent);
    return this.#propose(vault, intent, payload, { hours });
  }

  async cancel(vault: VaultRecord, id: string): Promise<ProposalView> {
    const target = await this.#wire(vault, id);
    // Same nonce, no calls: executing it consumes the nonce, so the original can never run.
    const payload = mergeExecutionPayloads([await this.#feePayload()]);
    return this.#propose(vault, { kind: "cancel", cancels: id }, payload, {
      hours: 24 * 7,
      txNonce: Fr.fromString(target.txNonce),
    });
  }

  /**
   * Everything the member is shown before signing, rebuilt on this device: the transaction from its calls (its outer
   * hash must match the proposal's), the config version and deadline, a simulation, and what the signature covers.
   */
  async review(vault: VaultRecord, id: string): Promise<Review> {
    const wallet = await this.#engine.wallet();
    const view = await this.proposal(vault, id);
    const proposal = await this.#wire(vault, id);
    const chainTime = await this.#chainTime();
    let failure: string | undefined;
    let rebuilt = false;
    let simulation: Review["simulation"] = { ok: false, error: "not simulated" };
    try {
      const tx = await this.#rebuild(wallet, vault, proposal);
      rebuilt = true;
      try {
        const preview = await wallet.previewVaultTx(tx);
        const gas = preview.gasUsed?.totalGas;
        simulation = {
          ok: true,
          gas: gas ? { da: Number(gas.daGas), l2: Number(gas.l2Gas) } : undefined,
        };
      } catch (error) {
        simulation = { ok: false, error: error instanceof Error ? error.message : String(error) };
      }
    } catch (error) {
      failure = error instanceof Error ? error.message : String(error);
    }
    const configCurrent = view.configVersion === view.currentConfigVersion;
    const notExpired = BigInt(view.expiresAt) > chainTime;
    if (!failure && !configCurrent)
      failure = `Proposed at config v${view.configVersion}, the vault is at v${view.currentConfigVersion}`;
    if (!failure && !notExpired) failure = "The proposal's deadline has passed";
    const intent = view.intent;
    const recipient =
      intent.kind === "transfer"
        ? {
            address: intent.to,
            ...this.#label(intent.to),
          }
        : undefined;
    return {
      proposal: view,
      vaultName: vault.name,
      myName: vault.members[vault.myIndex - 1]?.name ?? `member ${vault.myIndex}`,
      checks: { rebuilt, outerHash: proposal.outerHash, configCurrent, notExpired, failure },
      action: await this.#describe(vault, intent),
      recipient,
      simulation,
      signs: {
        message: view.approvalMessage,
        formula: `Poseidon2(clave.approval.v0, [vault, outer hash, v${view.configVersion}, ${view.expiresAt}])`,
      },
      rejects: {
        message: proposalRejectionMessage(id, vault.address),
        formula: "Poseidon2(clave.rejection.v0, [vault, id_hi, id_lo])",
      },
    };
  }

  async approve(vault: VaultRecord, id: string, secret: Secret | undefined): Promise<ProposalView> {
    const wallet = await this.#engine.wallet();
    const proposal = await this.#wire(vault, id);
    const view = await this.proposal(vault, id);
    if (view.status === "executed" || view.status === "cancelled" || view.status === "expired")
      throw new DaemonError("invalid", `Proposal ${short(id)} is ${view.status}`);
    // Rebuilding refuses a proposal whose calls do not hash to its outer hash, or whose config version is stale.
    const tx = await this.#rebuild(wallet, vault, proposal);
    const signature = await this.#engine.host.sign(tx.approvalMessage.toString(), secret);
    const approval: WireApproval = { proposal: id, slot: vault.myIndex - 1, signature };
    await this.#engine.keeper.client.call("approvals.put", {
      vault: vault.address,
      body: hex(encodeApproval(approval)),
    });
    return this.proposal(vault, id);
  }

  /** A signed vote against `id`. Approvals outrank rejections, so a member who approved cannot reject. */
  async reject(
    vault: VaultRecord,
    id: string,
    reason: string,
    secret: Secret | undefined,
  ): Promise<ProposalView> {
    const view = await this.proposal(vault, id);
    if (view.approvedByMe)
      throw new DaemonError(
        "invalid",
        "You approved this proposal; an approval cannot be withdrawn. Propose a cancel instead: clave cancel",
      );
    if (view.status !== "collecting" && view.status !== "ready" && view.status !== "blocked")
      throw new DaemonError("invalid", `Proposal ${short(id)} is ${view.status}`);
    if (view.rejectedByMe) return view;
    const signature = await this.#engine.host.sign(
      proposalRejectionMessage(id, vault.address),
      secret,
    );
    await this.#engine.keeper.client.call("rejections.put", {
      vault: vault.address,
      body: hex(encodeRejection({ proposal: id, slot: vault.myIndex - 1, reason, signature })),
    });
    return this.proposal(vault, id);
  }

  async execute(vault: VaultRecord, id: string): Promise<ProposalView> {
    const progress = (
      phase: "rebuilding" | "proving" | "mined" | "failed",
      extra: { error?: string; txHash?: string } = {},
    ) =>
      this.#engine.host.emit({
        type: "execution",
        execution: { vault: vault.address, proposal: id, phase, ...extra },
      });
    try {
      progress("rebuilding");
      const wallet = await this.#engine.wallet();
      const proposal = await this.#wire(vault, id);
      const tx = await this.#rebuild(wallet, vault, proposal);
      const approvals = (await this.#approvals(vault, id, proposal)).filter((a) => a.valid);
      if (approvals.length < vault.threshold)
        throw new DaemonError(
          "not-ready",
          `${approvals.length} of ${vault.threshold} approvals; cannot execute yet`,
        );
      const wire = await this.#wireApprovals(id);
      const valid = new Set(approvals.map((a) => a.slot));
      progress("proving");
      const receipt = await wallet.executeVaultTx(
        tx,
        vaultApprovalsOf(wire.filter((a) => valid.has(a.slot))),
      );
      const txHash = receipt.txHash.toString();
      await this.#engine.keeper.client.call("receipts.put", {
        vault: vault.address,
        proposal: id,
        txHash,
      });
      progress("mined", { txHash });
      this.#engine.host.emit({ type: "vault-activity", vault: vault.address });
      return this.proposal(vault, id);
    } catch (error) {
      progress("failed", { error: error instanceof Error ? error.message : String(error) });
      throw error;
    }
  }

  async proposals(vault: VaultRecord): Promise<ProposalView[]> {
    const bodies = await this.#engine.keeper.client.call<string[]>("proposals.list", {
      vault: vault.address,
    });
    const receipts = await this.#receipts(vault);
    const [chainTime, version] = await Promise.all([this.#chainTime(), this.#configVersion(vault)]);
    const wires = bodies.map((body) => decodeProposal(fromHex(body)));
    const views = await Promise.all(
      wires.map((w) => this.#view(vault, w, receipts, chainTime, version)),
    );
    // A cancel proposal that executed cancels its target.
    for (const view of views)
      if (view.intent.kind === "cancel" && view.status === "executed") {
        const target = views.findIndex(
          (v) => v.id === (view.intent as { cancels: string }).cancels,
        );
        const cancelled = views[target];
        if (cancelled && cancelled.status !== "executed")
          views[target] = { ...cancelled, status: "cancelled", cancelledBy: view.id };
      }
    return views.toSorted((a, b) => rank(a.status) - rank(b.status) || b.expiresAt - a.expiresAt);
  }

  async proposal(vault: VaultRecord, id: string): Promise<ProposalView> {
    const view = (await this.proposals(vault)).find((p) => p.id.toLowerCase() === id.toLowerCase());
    if (!view) throw new DaemonError("invalid", "No such proposal in this vault");
    return view;
  }

  async functions(contract: string): Promise<ContractFunction[]> {
    const artifact = await this.#artifact(contract);
    return artifact.functions
      .filter((f) => !f.isOnlySelf && !f.isInitializer && !f.name.startsWith("_"))
      .map((f) => ({
        name: f.name,
        kind:
          f.functionType === "private"
            ? "private"
            : f.functionType === "public"
              ? "public"
              : "utility",
        isStatic: f.isStatic,
        params: f.parameters.map((p) => ({ name: p.name, type: typeName(p.type) })),
      }));
  }

  async activity(vault: VaultRecord): Promise<ActivityEntry[]> {
    const proposals = await this.proposals(vault);
    const entries: ActivityEntry[] = proposals
      .filter((p) => p.status !== "collecting" && p.status !== "ready")
      .map((p) => ({ kind: "proposal" as const, at: p.execution?.at ?? p.expiresAt, proposal: p }));
    const wallet = await this.#engine.wallet();
    const owner = addressOf(vault.address);
    for (const token of (await this.#loadTokens())[vault.address.toLowerCase()] ?? []) {
      try {
        // Sequential: each query syncs the PXE and fans out ECDH requests to the keepers.
        // oxlint-disable-next-line no-await-in-loop
        const events = await wallet.getPrivateEvents<{
          from: AztecAddress;
          to: AztecAddress;
          amount: bigint;
        }>(TokenContract.events.Transfer, {
          contractAddress: addressOf(token.address),
          scopes: [owner],
        });
        for (const { event, metadata } of events) {
          if (!event.to.equals(owner)) continue;
          const block = Number((metadata as { blockNumber?: number | bigint }).blockNumber ?? 0);
          entries.push({
            kind: "received",
            at: block,
            token,
            from: event.from.toString(),
            amount: String(event.amount),
            txHash: String((metadata as { txHash?: unknown }).txHash ?? ""),
            block,
          });
        }
      } catch {
        // A token whose notes this PXE cannot read yet (keepers offline) shows no transfers rather than failing.
      }
    }
    return entries.toSorted((a, b) => b.at - a.at);
  }

  // Internals ------------------------------------------------------------------------------------------------------

  async #propose(
    vault: VaultRecord,
    intent: ProposalIntent,
    payload: ExecutionPayload,
    options: { hours: number; txNonce?: Fr },
  ): Promise<ProposalView> {
    const wallet = await this.#engine.wallet();
    const summary = JSON.stringify(intent);
    if (Buffer.byteLength(summary) > 4096)
      throw new DaemonError("invalid", "The proposal description is over 4 KiB");
    const tx = await wallet.prepareVaultTx(addressOf(vault.address), payload, {
      expiresAt: (await this.#chainTime()) + BigInt(options.hours * 3600),
      ...(options.txNonce ? { txNonce: options.txNonce } : {}),
    });
    const wire = proposalOf(tx, { summary, proposer: vault.myIndex, feeMethod: FEE_SPONSORED });
    const { id } = await this.#engine.keeper.client.call<{ id: string }>("proposals.put", {
      vault: vault.address,
      body: hex(encodeProposal(wire)),
    });
    this.#engine.host.emit({ type: "vault-activity", vault: vault.address });
    return this.proposal(vault, id);
  }

  async #payload(intent: ProposalIntent): Promise<ExecutionPayload> {
    const wallet = await this.#engine.wallet();
    const requests: ExecutionPayload[] = [];
    if (intent.kind === "transfer") {
      await this.#registerToken(intent.token);
      requests.push(
        await TokenContract.at(addressOf(intent.token), wallet)
          .methods.transfer(addressOf(intent.to), BigInt(intent.amount))
          .request(),
      );
    } else if (intent.kind === "calls") {
      requests.push(
        ...(await Promise.all(intent.calls.map((call) => this.#callRequest(wallet, call)))),
      );
    }
    return mergeExecutionPayloads([await this.#feePayload(), ...requests]);
  }

  async #callRequest(
    wallet: VaultWallet,
    call: { to: string; fn: string; args: readonly string[] },
  ): Promise<ExecutionPayload> {
    const artifact = await this.#artifact(call.to);
    const fn = artifact.functions.find((f) => f.name === call.fn);
    if (!fn) throw new DaemonError("invalid", `${call.fn} is not a function of ${call.to}`);
    if (fn.parameters.length !== call.args.length)
      throw new DaemonError(
        "invalid",
        `${call.fn} takes ${fn.parameters.length} arguments, got ${call.args.length}`,
      );
    const args = fn.parameters.map((p, i) => coerce(call.args[i] ?? "", p.type));
    const contract = await Contract.at(addressOf(call.to), artifact, wallet);
    const method = contract.methods[call.fn];
    if (!method) throw new DaemonError("invalid", `${call.fn} is not callable on ${call.to}`);
    return method(...args).request();
  }

  #feePayload(): Promise<ExecutionPayload> {
    return this.#engine.fee.paymentMethod.getExecutionPayload();
  }

  async #rebuild(wallet: VaultWallet, vault: VaultRecord, proposal: WireProposal) {
    try {
      return await vaultTxOf(
        (payload, options) => wallet.prepareVaultTx(addressOf(vault.address), payload, options),
        proposal,
      );
    } catch (error) {
      if (error instanceof ProposalMismatchError && error.reason === "stale-config")
        throw new DaemonError(
          "invalid",
          `${error.message}. Approvals from older config versions are void; propose it again.`,
          { cause: error },
        );
      throw error;
    }
  }

  async #wire(vault: VaultRecord, id: string): Promise<WireProposal> {
    const body = await this.#engine.keeper.client.call<string | null>("proposals.get", { id });
    if (!body) throw new DaemonError("invalid", "No such proposal");
    const wire = decodeProposal(fromHex(body));
    if (proposalId(wire).toLowerCase() !== id.toLowerCase())
      throw new Error("Proposal id mismatch");
    void vault;
    return wire;
  }

  async #wireApprovals(id: string): Promise<WireApproval[]> {
    const bodies = await this.#engine.keeper.client.call<string[]>("approvals.list", {
      proposal: id,
    });
    return bodies.map((b) => decodeApproval(fromHex(b)));
  }

  async #rejections(vault: VaultRecord, id: string): Promise<Vote[]> {
    const bodies = await this.#engine.keeper.client.call<string[]>("rejections.list", {
      proposal: id,
    });
    return bodies.map((body) => {
      const r = decodeRejection(fromHex(body));
      const member = vault.members[r.slot];
      let valid = false;
      try {
        valid = member
          ? verifyProposalRejection(r, vault.address, member.signerKey) &&
            r.proposal.toLowerCase() === id.toLowerCase()
          : false;
      } catch {
        valid = false;
      }
      return {
        slot: r.slot,
        index: r.slot + 1,
        name: member?.name ?? `Slot ${r.slot}`,
        valid,
        reason: r.reason || undefined,
      };
    });
  }

  async #approvals(vault: VaultRecord, id: string, proposal: WireProposal): Promise<Vote[]> {
    const wire = await this.#wireApprovals(id);
    return wire.map((a) => {
      const member = vault.members[a.slot];
      let valid = false;
      try {
        valid = member
          ? verifyProposalApproval(proposal, a, vault.address, member.signerKey)
          : false;
      } catch {
        valid = false;
      }
      return { slot: a.slot, index: a.slot + 1, name: member?.name ?? `Slot ${a.slot}`, valid };
    });
  }

  async #receipts(vault: VaultRecord): Promise<Map<string, Receipt>> {
    try {
      const list = await this.#engine.keeper.client.call<Receipt[]>("receipts.list", {
        vault: vault.address,
      });
      return new Map(list.map((r) => [r.proposal.toLowerCase(), r]));
    } catch {
      return new Map();
    }
  }

  async #view(
    vault: VaultRecord,
    wire: WireProposal,
    receipts: Map<string, Receipt>,
    chainTime: bigint,
    version: number,
  ): Promise<ProposalView> {
    const id = proposalId(wire);
    const intent = parseIntent(wire.summary);
    const approvals = await this.#approvals(vault, id, wire);
    const approvedSlots = new Set(approvals.filter((a) => a.valid).map((a) => a.slot));
    // An approval outranks a rejection from the same slot.
    const rejections = (await this.#rejections(vault, id)).filter(
      (r) => r.valid && !approvedSlots.has(r.slot),
    );
    const payload = decodePayload(wire.payload);
    const receipt = receipts.get(id.toLowerCase());
    const validCount = approvedSlots.size;
    const members = vault.members.length;
    const status: ProposalStatus = receipt
      ? "executed"
      : BigInt(wire.expiresAt) <= chainTime
        ? "expired"
        : wire.configVersion !== version
          ? "stale"
          : validCount >= vault.threshold
            ? "ready"
            : rejections.length > members - vault.threshold
              ? "blocked"
              : "collecting";
    const proposer = vault.members[wire.proposer - 1];
    return {
      id,
      vault: vault.address,
      intent,
      proposer: { index: wire.proposer, name: proposer?.name ?? `Member ${wire.proposer}` },
      status,
      threshold: vault.threshold,
      members,
      approvals,
      rejections,
      mySlot: vault.myIndex - 1,
      approvedByMe: approvedSlots.has(vault.myIndex - 1),
      rejectedByMe: rejections.some((r) => r.slot === vault.myIndex - 1),
      configVersion: wire.configVersion,
      currentConfigVersion: version,
      expiresAt: wire.expiresAt,
      txNonce: wire.txNonce,
      outerHash: wire.outerHash,
      approvalMessage: proposalApprovalMessage(wire, vault.address),
      feeMethod: wire.feeMethod === FEE_SPONSORED ? "Sponsored FPC" : `method ${wire.feeMethod}`,
      calls: await this.#decodeCalls(vault, payload),
      // A transfer of a token this member has never seen: register and track it, so amounts and balances read.
      token:
        intent.kind === "transfer"
          ? await this.addToken(vault, intent.token).catch(() => undefined)
          : undefined,
      execution: receipt ? { txHash: receipt.txHash, at: receipt.at } : undefined,
      cancelledBy: intent.kind === "cancel" ? intent.cancels : undefined,
    };
  }

  async #decodeCalls(vault: VaultRecord, payload: ExecutionPayload): Promise<DecodedCall[]> {
    const fee = await this.#feePayload();
    const feeTargets = new Set(fee.calls.map((c) => c.to.toString().toLowerCase()));
    const tokens = (await this.#loadTokens())[vault.address.toLowerCase()] ?? [];
    return payload.calls.map((call) => {
      const to = call.to.toString();
      const token = tokens.find((t) => t.address.toLowerCase() === to.toLowerCase());
      return {
        to,
        toLabel: token
          ? token.symbol
          : feeTargets.has(to.toLowerCase())
            ? "Sponsored FPC"
            : undefined,
        fn: call.name,
        args: call.args.map((a) => a.toString()),
        isStatic: call.isStatic,
      };
    });
  }

  /** Where a recipient's name comes from: this member's own account, the address book, or nowhere. */
  #label(address: string): { label?: string; source: "address book" | "member" | null } {
    if (this.#engine.ownAccount?.toLowerCase() === address.toLowerCase())
      return { label: `${this.#engine.profile.name} (you)`, source: "member" };
    const contact = this.#engine.host
      .contacts()
      .find((c) => c.address.toLowerCase() === address.toLowerCase());
    return contact ? { label: contact.label, source: "address book" } : { source: null };
  }

  async #describe(vault: VaultRecord, intent: ProposalIntent): Promise<string> {
    if (intent.kind === "transfer") {
      const token = await this.#tokenInfo(intent.token).catch(() => undefined);
      const units = token
        ? `${formatUnits(BigInt(intent.amount), token.decimals)} ${token.symbol} (${intent.amount} base units)`
        : `${intent.amount} base units of ${intent.token}`;
      void vault;
      return `transfer ${units}`;
    }
    if (intent.kind === "cancel")
      return `cancel proposal ${short(intent.cancels)} (burns its tx nonce)`;
    return intent.calls.map((c) => `call ${c.fn}(${c.args.join(", ")}) on ${c.to}`).join("; ");
  }

  async #registerToken(address: string): Promise<TokenInfo> {
    const wallet = await this.#engine.wallet();
    const contract = addressOf(address);
    if (!(await wallet.getContractMetadata(contract)).instance) {
      const instance = await this.#engine.node.getContract(contract);
      if (!instance)
        throw new DaemonError("invalid", `No contract instance at ${address} on this network`);
      await wallet.registerContract(instance, TokenContract.artifact);
    }
    return this.#tokenInfo(address);
  }

  async #tokenInfo(address: string): Promise<TokenInfo> {
    const cached = Object.values(await this.#loadTokens())
      .flat()
      .find((t) => t.address.toLowerCase() === address.toLowerCase());
    if (cached) return cached;
    const wallet = await this.#engine.wallet();
    const token = TokenContract.at(addressOf(address), wallet);
    const from = this.#engine.account;
    const [name, symbol, decimals] = await Promise.all([
      token.methods.public_get_name().simulate({ from }),
      token.methods.public_get_symbol().simulate({ from }),
      token.methods.public_get_decimals().simulate({ from }),
    ]);
    return {
      address: contractString(address),
      name: readFieldCompressedString(name.result as { value: bigint }),
      symbol: readFieldCompressedString(symbol.result as { value: bigint }),
      decimals: Number(decimals.result as bigint),
    };
  }

  async #artifact(address: string): Promise<ContractArtifact> {
    const key = address.toLowerCase();
    const cached = this.#artifacts.get(key);
    if (cached) return cached;
    const wallet = await this.#engine.wallet();
    const instance = await wallet.pxeService.getContractInstance(addressOf(address));
    // The PXE stores the deployment preimage; a contract never upgraded still runs its original class.
    const classId = instance?.originalContractClassId;
    const artifact = classId ? await wallet.pxeService.getContractArtifact(classId) : undefined;
    if (!artifact)
      throw new DaemonError(
        "invalid",
        `No artifact registered for ${address}; add it as a token or register it first`,
      );
    this.#artifacts.set(key, artifact);
    return artifact;
  }

  async #chainTime(): Promise<bigint> {
    const block = await this.#engine.node.getBlockData("latest");
    if (!block) throw new DaemonError("not-ready", "The node has no blocks yet");
    return block.header.globalVariables.timestamp;
  }

  async #configVersion(vault: VaultRecord): Promise<number> {
    const cached = this.#config.get(vault.address);
    if (cached && Date.now() - cached.at < CONFIG_TTL_MS) return cached.version;
    const wallet = await this.#engine.wallet();
    const config = await wallet.getVaultConfig(addressOf(vault.address));
    this.#config.set(vault.address, { version: config.version, at: Date.now() });
    return config.version;
  }

  async #loadTokens(): Promise<Record<string, TokenInfo[]>> {
    if (this.#tokens) return this.#tokens;
    try {
      this.#tokens = JSON.parse(await readFile(this.#tokensFile, "utf8")) as Record<
        string,
        TokenInfo[]
      >;
    } catch {
      this.#tokens = {};
    }
    return this.#tokens;
  }

  async #rememberToken(vault: VaultRecord, info: TokenInfo): Promise<void> {
    const tokens = await this.#loadTokens();
    const key = vault.address.toLowerCase();
    const list = tokens[key] ?? [];
    if (!list.some((t) => t.address.toLowerCase() === info.address.toLowerCase())) {
      tokens[key] = [...list, info];
      await writeFile(this.#tokensFile, JSON.stringify(tokens, null, 2));
    }
  }
}

const RANK: Record<ProposalStatus, number> = {
  ready: 0,
  collecting: 1,
  blocked: 2,
  stale: 3,
  executed: 4,
  cancelled: 5,
  expired: 6,
};

const short = (id: string) => id.slice(2, 8);
const rank = (status: ProposalStatus) => RANK[status];

const contractString = (address: string) => addressOf(address).toString();

function parseIntent(summary: string): ProposalIntent {
  try {
    const intent = JSON.parse(summary) as ProposalIntent;
    if (intent.kind === "transfer" || intent.kind === "calls" || intent.kind === "cancel")
      return intent;
  } catch {
    // Proposals made elsewhere may carry free text.
  }
  return { kind: "calls", calls: [], memo: summary };
}

export function formatUnits(value: bigint, decimals: number): string {
  if (decimals === 0) return value.toString();
  const base = 10n ** BigInt(decimals);
  const whole = value / base;
  const fraction = (value % base).toString().padStart(decimals, "0").replace(/0+$/u, "");
  return fraction ? `${whole}.${fraction}` : whole.toString();
}

function typeName(type: AbiType): string {
  switch (type.kind) {
    case "field":
      return "Field";
    case "boolean":
      return "bool";
    case "integer":
      return `${type.sign === "signed" ? "i" : "u"}${type.width}`;
    case "string":
      return `str<${type.length}>`;
    case "array":
      return `[${typeName(type.type)}; ${type.length}]`;
    case "struct":
      return type.path.split("::").at(-1) ?? "struct";
    default:
      return type.kind;
  }
}

/** A form value as the ABI type the function takes. */
function coerce(value: string, type: AbiType): unknown {
  const trimmed = value.trim();
  switch (type.kind) {
    case "field":
    case "integer":
      return BigInt(trimmed);
    case "boolean":
      return trimmed === "true" || trimmed === "1";
    case "struct":
      if (type.path.endsWith("AztecAddress")) return addressOf(trimmed);
      return JSON.parse(trimmed) as unknown;
    default:
      return JSON.parse(trimmed) as unknown;
  }
}
