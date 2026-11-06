import { useQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";

import { call } from "@/lib/ipc";
import { keys, useAppState } from "@/lib/state";

/** The vault the current route names: its summary (instant, from app state) and its details (from the engine). */
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
