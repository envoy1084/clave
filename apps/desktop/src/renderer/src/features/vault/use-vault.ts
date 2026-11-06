import { useQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";

import { call } from "@/lib/ipc";
import { keys, useAppState } from "@/lib/state";

/** The vault the current route names: its summary (from app state) and its on-chain details (from the engine). */
export function useVault() {
  const { address } = useParams({ from: "/shell/vault/$address" });
  const { data: state } = useAppState();
  const summary = state?.vaults.find((v) => v.address.toLowerCase() === address.toLowerCase());
  const details = useQuery({
    queryKey: keys.vault(address),
    queryFn: () => call("vault.details", { vault: address }),
    refetchInterval: 20_000,
  });
  return { address, summary, details, technical: state?.settings.technicalDetails ?? false };
}

export function useTokens(vault: string) {
  return useQuery({
    queryKey: keys.tokens(vault),
    queryFn: () => call("tokens.list", { vault }),
    refetchInterval: 30_000,
  });
}

export function useProposals(vault: string) {
  return useQuery({
    queryKey: keys.proposals(vault),
    queryFn: () => call("proposals.list", { vault }),
    refetchInterval: 15_000,
  });
}

export function useActivity(vault: string) {
  return useQuery({
    queryKey: keys.activity(vault),
    queryFn: () => call("vault.activity", { vault }),
  });
}

export function usePeers(vault: string) {
  return useQuery({
    queryKey: keys.peers(vault),
    queryFn: () => call("vault.peers", { vault }),
    refetchInterval: 5000,
  });
}

export function useContacts() {
  return useQuery({ queryKey: keys.contacts, queryFn: () => call("contacts.list") });
}
