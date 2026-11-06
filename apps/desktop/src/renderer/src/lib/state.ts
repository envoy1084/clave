import { useEffect } from "react";

import { QueryClient, useQuery, useQueryClient } from "@tanstack/react-query";

// Live app state: TanStack Query caches every engine/main call; events from the main process update the cache in
// place, so screens re-render without polling.
import type {
  AppEvent,
  AppState,
  ExecutionProgress,
  FoundingState,
  LobbyView,
  NodeStatus,
} from "@shared/types.ts";

import { call, onEvent } from "./ipc.ts";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, refetchOnWindowFocus: false, staleTime: 2000 },
  },
});

export const keys = {
  state: ["app.state"] as const,
  node: ["node.status"] as const,
  lobby: (id: string) => ["lobby", id.toLowerCase()] as const,
  lobbies: ["lobby.list"] as const,
  founding: (id: string) => ["founding", id.toLowerCase()] as const,
  vault: (address: string) => ["vault.details", address.toLowerCase()] as const,
  audit: (address: string) => ["vault.audit", address.toLowerCase()] as const,
  pxe: ["pxe.status"] as const,
  /** Everything that changes when a vault's records or notes change. */
  vaultData: (address: string) => ["vault-data", address.toLowerCase()] as const,
  tokens: (address: string) => ["vault-data", address.toLowerCase(), "tokens"] as const,
  proposals: (address: string) => ["vault-data", address.toLowerCase(), "proposals"] as const,
  proposal: (address: string, id: string) =>
    ["vault-data", address.toLowerCase(), "proposal", id.toLowerCase()] as const,
  activity: (address: string) => ["vault-data", address.toLowerCase(), "activity"] as const,
  peers: (address: string) => ["vault-peers", address.toLowerCase()] as const,
  execution: (address: string, id: string) =>
    ["execution", address.toLowerCase(), id.toLowerCase()] as const,
  contacts: ["contacts.list"] as const,
};

/** Installs the event bridge once, at the root. */
export function useEventBridge(): void {
  const client = useQueryClient();
  useEffect(
    () =>
      onEvent((event: AppEvent) => {
        switch (event.type) {
          case "state":
            client.setQueryData<AppState>(keys.state, event.state);
            break;
          case "node":
            client.setQueryData<NodeStatus>(keys.node, event.node);
            break;
          case "lobby":
            client.setQueryData<LobbyView>(keys.lobby(event.lobby.lobby), event.lobby);
            void client.invalidateQueries({ queryKey: keys.lobbies });
            break;
          case "founding":
            client.setQueryData<FoundingState>(keys.founding(event.founding.lobby), event.founding);
            break;
          case "vault-activity":
            void client.invalidateQueries({ queryKey: keys.vaultData(event.vault) });
            break;
          case "execution":
            client.setQueryData<ExecutionProgress>(
              keys.execution(event.execution.vault, event.execution.proposal),
              event.execution,
            );
            break;
        }
      }),
    [client],
  );
}

export function useAppState() {
  return useQuery({
    queryKey: keys.state,
    queryFn: () => call("app.state"),
    staleTime: Number.POSITIVE_INFINITY,
  });
}

export function useNodeStatus(enabled = true) {
  return useQuery({
    queryKey: keys.node,
    queryFn: () => call("node.status"),
    enabled,
    refetchInterval: 15_000,
  });
}
