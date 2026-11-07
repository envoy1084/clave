// The daemon's RPC contract (architecture/cli.md): every method a client may call, the schema its params must satisfy
// and its result type. The server rejects any other method and any params that fail their schema.
import { z } from "zod";

import type {
  ActivityEntry,
  AuditEntry,
  Contact,
  ContractFunction,
  DaemonInfo,
  FoundingState,
  InvitePreview,
  LobbyView,
  MemberReachability,
  NodeStatus,
  ProposalPreview,
  ProposalView,
  Review,
  Settings,
  SignerStatus,
  Status,
  TokenBalance,
  TokenInfo,
  VaultDetails,
  VaultSummary,
} from "../types.ts";
import { DaemonError } from "./errors.ts";

const hex = (bytes?: number) =>
  z
    .string()
    .regex(
      bytes ? new RegExp(`^0x[0-9a-fA-F]{${bytes * 2}}$`) : /^0x[0-9a-fA-F]+$/u,
      "Expected hex",
    );
const address = hex(32);
/** A token amount in base units, as a decimal string (u128 range). */
const amount = z
  .string()
  .regex(/^[1-9]\d{0,38}$/u, "Expected a positive whole number of base units");
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
const alias = z
  .string()
  .regex(/^[a-z0-9][a-z0-9_-]{0,31}$/u, "Aliases are lowercase letters, digits, - and _");
const threshold = z.number().int().min(1).max(8);
const nodeUrl = z
  .string()
  .trim()
  .url("Expected a full URL, like http://localhost:8080")
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
  .startsWith("clave-invite-v0:", "This is not a Clave invite")
  .max(4096);
const passphrase = z.string().min(8, "Use a passphrase of at least 8 characters").max(1024);
const secret = z.union([
  z.object({ passphrase: z.string().min(1).max(1024) }),
  z.object({ key: hex(32) }),
]);
const lobby = z.object({ lobby: hex(16) });
const vault = z.object({ vault: address });
const hours = z.number().int().min(1).max(720);
const none = z.undefined().or(z.null());

export const methods = {
  "daemon.info": none,
  "daemon.stop": none,
  status: none,
  "profile.create": z.object({ name, nodeUrl, passphrase }),
  "config.get": none,
  "config.set": z
    .object({
      nodeUrl,
      proverEnabled: z.boolean(),
      unlockTimeoutSecs: z.number().int().min(60).max(86_400),
    })
    .partial(),
  "relays.set": z.object({ relays: z.array(relay).max(8) }),
  "signer.status": none,
  "signer.unlock": z.object({ secret, ttlSecs: z.number().int().min(1).max(86_400) }),
  "signer.lock": none,
  "signer.sealingKey": z.object({ passphrase: z.string().min(1).max(1024) }),
  "node.status": none,
  "lobby.inspect": z.object({ invite }),
  "lobby.create": z.object({ name, threshold, viewingThreshold: threshold.min(2) }),
  "lobby.join": z.object({ invite }),
  "lobby.get": lobby,
  "lobby.list": none,
  "lobby.start": lobby.extend({ threshold, viewingThreshold: threshold.min(2) }),
  "lobby.leave": lobby,
  "founding.begin": lobby,
  "founding.get": lobby,
  "vault.list": none,
  "vault.details": vault,
  "vault.peers": vault,
  "vault.audit": vault.extend({ limit: z.number().int().min(1).max(500) }),
  "vault.activity": vault,
  "vault.alias": vault.extend({ alias }),
  "vault.use": vault,
  "tokens.list": vault,
  "tokens.add": vault.extend({ token: address }),
  "tokens.deployTest": vault,
  "proposals.list": vault,
  "proposals.get": proposalRef,
  "proposals.preview": vault.extend({ intent, expiresInHours: hours }),
  "proposals.create": vault.extend({ intent, expiresInHours: hours }),
  "proposals.review": proposalRef,
  "proposals.approve": proposalRef.extend({ secret: secret.optional() }),
  "proposals.reject": proposalRef.extend({
    reason: z.string().trim().max(280).optional(),
    secret: secret.optional(),
  }),
  "proposals.execute": proposalRef,
  "proposals.cancel": proposalRef,
  "contracts.functions": z.object({ contract: address }),
  "contacts.list": none,
  "contacts.put": z.object({ address, label: alias }),
  "contacts.remove": z.object({ address }),
} as const;

export type Method = keyof typeof methods;
export type Params<M extends Method> = z.output<(typeof methods)[M]>;
export type Secret = z.output<typeof secret>;
export type Intent = z.output<typeof intent>;

export interface Results {
  "daemon.info": DaemonInfo;
  "daemon.stop": null;
  status: Status;
  "profile.create": Status;
  "config.get": Settings;
  "config.set": Settings;
  "relays.set": Settings;
  "signer.status": SignerStatus;
  "signer.unlock": SignerStatus;
  "signer.lock": SignerStatus;
  "signer.sealingKey": { key: string };
  "node.status": NodeStatus;
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
  "vault.peers": MemberReachability[];
  "vault.audit": AuditEntry[];
  "vault.activity": ActivityEntry[];
  "vault.alias": VaultSummary[];
  "vault.use": VaultSummary[];
  "tokens.list": TokenBalance[];
  "tokens.add": TokenInfo;
  "tokens.deployTest": TokenInfo;
  "proposals.list": ProposalView[];
  "proposals.get": ProposalView;
  "proposals.preview": ProposalPreview;
  "proposals.create": ProposalView;
  "proposals.review": Review;
  "proposals.approve": ProposalView;
  "proposals.reject": ProposalView;
  "proposals.execute": ProposalView;
  "proposals.cancel": ProposalView;
  "contracts.functions": ContractFunction[];
  "contacts.list": Contact[];
  "contacts.put": Contact[];
  "contacts.remove": Contact[];
}

/** Validates a call; returns the parsed params or throws an `invalid` {@link DaemonError}. */
export function parseCall(method: unknown, params: unknown): { method: Method; params: unknown } {
  if (typeof method !== "string" || !Object.hasOwn(methods, method))
    throw new DaemonError("invalid", `Unknown method ${String(method)}`);
  const parsed = methods[method as Method].safeParse(params ?? undefined);
  if (!parsed.success) {
    const issue = parsed.error.issues[0];
    const where = issue?.path.length ? `${issue.path.join(".")}: ` : "";
    throw new DaemonError("invalid", `${where}${issue?.message ?? "Invalid request"}`);
  }
  return { method: method as Method, params: parsed.data };
}
