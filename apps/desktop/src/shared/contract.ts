// The IPC contract: every method the renderer may call, the schema its params must satisfy, the process that serves it
// and its result type. The main process rejects any method not listed here and any params that fail their schema, so
// a compromised renderer can only make the calls this file allows, with well-formed arguments.
import { z } from "zod";

import type {
  AppState,
  AuditEntry,
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
