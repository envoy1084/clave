import type {
  KeeperCeremony,
  KeeperClient,
  KeeperDkgResult,
  KeeperManifest,
  KeeperParticipant,
  KeeperVault,
} from "@clave/vault-wallet";

// Typed wrappers over the keeper's lobby, ceremony and vault RPC (see architecture/keeper.md). Every keeper JSON
// shape the app depends on is named here, so a keeper change touches one file.
import { peerIdOf } from "../peer-id.ts";
import type { InvitePreview, LobbyView, Point } from "../types.ts";

export interface KeeperLobbyMember {
  readonly index: number;
  readonly name: string;
  readonly signerKey: Point;
  readonly identity: string;
  readonly transport: string;
  readonly addresses: readonly string[];
  readonly you: boolean;
}

export interface KeeperLobby {
  readonly lobby: string;
  readonly role: "host" | "guest";
  readonly status: "open" | "started" | "closed";
  readonly name: string;
  readonly threshold: number;
  readonly viewingThreshold: number;
  readonly network: string;
  readonly members: readonly KeeperLobbyMember[];
  readonly invite?: string;
  readonly plan?: {
    readonly session: string;
    readonly threshold: number;
    readonly viewingThreshold: number;
    readonly salt: string;
    readonly secrets: string;
  };
}

interface KeeperInvite {
  readonly lobby: string;
  readonly name: string;
  readonly hostName?: string;
  readonly threshold: number;
  readonly viewingThreshold: number;
  readonly network: string;
  readonly host: {
    readonly identity: string;
    readonly transport: string;
    readonly addresses: readonly string[];
  };
}

export const hex0x = (value: string) => (value.startsWith("0x") ? value : `0x${value}`);

export const participantOf = (m: {
  identity: string;
  transport: string;
  addresses: readonly string[];
}): KeeperParticipant => ({ identity: m.identity, transport: m.transport, addresses: m.addresses });

export function lobbyView(lobby: KeeperLobby): LobbyView {
  return {
    lobby: hex0x(lobby.lobby),
    role: lobby.role,
    status: lobby.status,
    name: lobby.name,
    threshold: lobby.threshold,
    viewingThreshold: lobby.viewingThreshold,
    network: lobby.network,
    invite: lobby.invite,
    session: lobby.plan?.session,
    members: lobby.members.map((m) => ({
      index: m.index,
      name: m.name,
      signerKey: m.signerKey,
      identity: m.identity,
      peerId: peerIdOf(m.identity),
      addresses: m.addresses,
      you: m.you,
    })),
  };
}

const bare = (hex: string) => hex.replace(/^0x/u, "");

export class KeeperApi {
  readonly client: KeeperClient;

  constructor(client: KeeperClient) {
    this.client = client;
  }

  async inspect(invite: string, network: string): Promise<InvitePreview> {
    const decoded = await this.client.call<KeeperInvite>("lobby.inspect", { invite });
    return {
      lobby: hex0x(decoded.lobby),
      name: decoded.name,
      hostName: decoded.hostName ?? "The host",
      threshold: decoded.threshold,
      viewingThreshold: decoded.viewingThreshold,
      network: decoded.network,
      sameNetwork: decoded.network === network,
      host: {
        identity: decoded.host.identity,
        peerId: peerIdOf(decoded.host.identity),
        addresses: decoded.host.addresses,
      },
    };
  }

  open(params: {
    name: string;
    threshold: number;
    viewingThreshold: number;
    network: string;
    member: { name: string; signerKey: Point };
  }): Promise<{ lobby: string; invite: string }> {
    return this.client.call("lobby.open", params);
  }

  join(invite: string, member: { name: string; signerKey: Point }): Promise<{ lobby: string }> {
    return this.client.call("lobby.join", { invite, member });
  }

  status(lobby: string): Promise<KeeperLobby> {
    return this.client.call("lobby.status", { lobby: bare(lobby) });
  }

  list(): Promise<KeeperLobby[]> {
    return this.client.call("lobby.list");
  }

  start(params: {
    lobby: string;
    threshold: number;
    viewingThreshold: number;
    salt: string;
    secrets: string;
  }): Promise<KeeperLobby> {
    return this.client.call("lobby.start", { ...params, lobby: bare(params.lobby) });
  }

  leave(lobby: KeeperLobby): Promise<unknown> {
    return this.client.call(lobby.role === "host" ? "lobby.close" : "lobby.leave", {
      lobby: bare(lobby.lobby),
    });
  }

  startDkg(
    session: string,
    threshold: number,
    members: readonly KeeperParticipant[],
  ): Promise<unknown> {
    return this.client.call("ceremony.dkg", { session, threshold, members });
  }

  ceremony(session: string): Promise<KeeperCeremony<KeeperDkgResult>> {
    return this.client.call("ceremony.status", { session });
  }

  register(session: string, manifest: KeeperManifest): Promise<unknown> {
    return this.client.call("vault.register", { session, manifest });
  }

  vault(vault: string): Promise<KeeperVault> {
    return this.client.call("vault.status", { vault });
  }

  audit(
    vault: string,
    limit: number,
  ): Promise<
    { at: number; requester: string; purpose: string; points: number; outcome: string }[]
  > {
    return this.client.call("audit.list", { vault, limit });
  }
}
