// The IPC contract: every method the renderer may call, the schema its params must satisfy, the process that serves it
// and its result type. The main process rejects any method not listed here and any params that fail their schema, so
// a compromised renderer can only make the calls this file allows, with well-formed arguments.
import { z } from "zod";

import type {
  ActivityEntry,
  AppState,
  AuditEntry,
  Contact,
  ContractFunction,
  MemberReachability,
  ProposalPreview,
  ProposalView,
  TokenBalance,
  TokenInfo,
  FoundingState,
  InvitePreview,
  LobbyView,
  NodeStatus,
  PxeStatus,
  VaultDetails,
  VaultSummary,
} from "./types.ts";

const hex = (bytes?: number) =>
  z
    .string()
    .regex(
      bytes ? new RegExp(`^0x[0-9a-fA-F]{${bytes * 2}}$`) : /^0x[0-9a-fA-F]+$/u,
      "Expected hex",
    );
const address = hex(32);
/** A token amount in base units, as a decimal string (u128 range). */
const amount = z.string().regex(/^[1-9]\d{0,38}$/u, "Enter a positive whole number of base units");
const memo = z.string().trim().max(280).optional();
const arg = z.string().trim().max(200);
const intent = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("transfer"), token: address, to: address, amount, memo }),
  z.object({
    kind: z.literal("calls"),
    calls: z
      .array(
        z.object({
          to: address,
          fn: z.string().regex(/^[A-Za-z_][A-Za-z0-9_]{0,63}$/u),
          args: z.array(arg).max(32),
        }),
      )
      .min(1)
      .max(8),
    memo,
  }),
]);
const proposalRef = z.object({ vault: address, id: hex(32) });
const name = z.string().trim().min(1, "Enter a name").max(48, "Keep it under 48 characters");
const threshold = z.number().int().min(1).max(8);
const nodeUrl = z
  .string()
  .trim()
  .url("Enter a full URL, like http://localhost:8080")
  .refine((url) => /^https?:\/\//u.test(url), "Use an http or https URL");
const relay = z
  .string()
  .trim()
  .regex(
    /^\/(ip4|ip6|dns4|dns6|dns)\/.+\/p2p\/[1-9A-HJ-NP-Za-km-z]+$/u,
    "A relay address ends with /p2p/<peer id>",
  );
const invite = z
  .string()
  .trim()
  .startsWith("tecpan-invite-v0:", "This is not a Tecpan invite")
  .max(4096);

const none = z.undefined();

export const methods = {
  // Main process: profile, settings, the signer and the OS.
  "app.state": { side: "main", params: none },
  "profile.create": { side: "main", params: z.object({ name, nodeUrl }) },
  "settings.update": {
    side: "main",
    params: z
      .object({
        nodeUrl,
        relays: z.array(relay).max(8),
        proverEnabled: z.boolean(),
        technicalDetails: z.boolean(),
      })
      .partial(),
  },
  "profile.rename": { side: "main", params: z.object({ name }) },
  "shell.openDataFolder": { side: "main", params: none },
  "clipboard.write": { side: "main", params: z.object({ text: z.string().max(8192) }) },
  "clipboard.readInvite": { side: "main", params: none },
  "node.probe": { side: "main", params: z.object({ nodeUrl }) },
  "keeper.restart": { side: "main", params: none },

  // Engine: the Aztec node, the PXE, the keeper's lobby and ceremonies, vault internals.
  "node.status": { side: "engine", params: none },
  "pxe.status": { side: "engine", params: none },
  "lobby.inspect": { side: "engine", params: z.object({ invite }) },
  "lobby.create": {
    side: "engine",
    params: z.object({ name, threshold, viewingThreshold: threshold.min(2) }),
  },
  "lobby.join": { side: "engine", params: z.object({ invite }) },
  "lobby.get": { side: "engine", params: z.object({ lobby: hex(16) }) },
  "lobby.list": { side: "engine", params: none },
  "lobby.start": {
    side: "engine",
    params: z.object({ lobby: hex(16), threshold, viewingThreshold: threshold.min(2) }),
  },
  "lobby.leave": { side: "engine", params: z.object({ lobby: hex(16) }) },
  "founding.begin": { side: "engine", params: z.object({ lobby: hex(16) }) },
  "founding.get": { side: "engine", params: z.object({ lobby: hex(16) }) },
  "vault.list": { side: "engine", params: none },
  "vault.details": { side: "engine", params: z.object({ vault: address }) },
  "vault.audit": {
    side: "engine",
    params: z.object({ vault: address, limit: z.number().int().min(1).max(500) }),
  },
  "vault.peers": { side: "engine", params: z.object({ vault: address }) },
  "vault.activity": { side: "engine", params: z.object({ vault: address }) },

  // Assets.
  "tokens.list": { side: "engine", params: z.object({ vault: address }) },
  "tokens.add": { side: "engine", params: z.object({ vault: address, token: address }) },
  "tokens.deployTest": { side: "engine", params: z.object({ vault: address }) },

  // Proposals: create, inspect, approve, execute, cancel.
  "proposals.list": { side: "engine", params: z.object({ vault: address }) },
  "proposals.get": { side: "engine", params: proposalRef },
  "proposals.preview": {
    side: "engine",
    params: z.object({ vault: address, intent, expiresInHours: z.number().int().min(1).max(720) }),
  },
  "proposals.create": {
    side: "engine",
    params: z.object({ vault: address, intent, expiresInHours: z.number().int().min(1).max(720) }),
  },
  "proposals.approve": { side: "engine", params: proposalRef },
  "proposals.execute": { side: "engine", params: proposalRef },
  "proposals.cancel": { side: "engine", params: proposalRef },
  "contracts.functions": { side: "engine", params: z.object({ contract: address }) },

  // Address book (local only).
  "contacts.list": { side: "main", params: none },
  "contacts.put": { side: "main", params: z.object({ address, label: name }) },
  "contacts.remove": { side: "main", params: z.object({ address }) },
} as const;

export type Method = keyof typeof methods;
export type Params<M extends Method> = z.input<(typeof methods)[M]["params"]>;

export interface Results {
  "app.state": AppState;
  "profile.create": AppState;
  "settings.update": AppState;
  "profile.rename": AppState;
  "shell.openDataFolder": null;
  "clipboard.write": null;
  "clipboard.readInvite": string | null;
  "keeper.restart": null;
  "node.probe": NodeStatus;
  "node.status": NodeStatus;
  "pxe.status": PxeStatus;
  "lobby.inspect": InvitePreview;
  "lobby.create": LobbyView;
  "lobby.join": LobbyView;
  "lobby.get": LobbyView;
  "lobby.list": LobbyView[];
  "lobby.start": LobbyView;
  "lobby.leave": null;
  "founding.begin": FoundingState;
  "founding.get": FoundingState | null;
  "vault.list": VaultSummary[];
  "vault.details": VaultDetails;
  "vault.audit": AuditEntry[];
  "vault.peers": MemberReachability[];
  "vault.activity": ActivityEntry[];
  "tokens.list": TokenBalance[];
  "tokens.add": TokenInfo;
  "tokens.deployTest": TokenInfo;
  "proposals.list": ProposalView[];
  "proposals.get": ProposalView;
  "proposals.preview": ProposalPreview;
  "proposals.create": ProposalView;
  "proposals.approve": ProposalView;
  "proposals.execute": ProposalView;
  "proposals.cancel": ProposalView;
  "contracts.functions": ContractFunction[];
  "contacts.list": Contact[];
  "contacts.put": Contact[];
  "contacts.remove": Contact[];
}

export type EngineMethod = {
  [M in Method]: (typeof methods)[M]["side"] extends "engine" ? M : never;
}[Method];

/** Validates a renderer call; returns the parsed params or throws a readable error. */
export function parseCall(method: unknown, params: unknown): { method: Method; params: unknown } {
  if (typeof method !== "string" || !Object.hasOwn(methods, method))
    throw new Error("Unknown method");
  const spec = methods[method as Method];
  const parsed = spec.params.safeParse(params);
  if (!parsed.success) throw new Error(parsed.error.issues[0]?.message ?? "Invalid request");
  return { method: method as Method, params: parsed.data };
}

/** Error shape crossing the IPC boundary. */
export interface IpcError {
  readonly message: string;
  readonly code?: string;
}
