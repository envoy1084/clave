import { useEffect, useState } from "react";

import { useQuery } from "@tanstack/react-query";

import type { NodeStatus } from "@shared/types.ts";

import { StatusDot } from "@/components/bits";
import { number } from "@/lib/format";
import { call } from "@/lib/ipc";

/** Probes a node URL (debounced) and says, in one line, whether Tecpan can use it. */
export function useNodeProbe(url: string) {
  const [debounced, setDebounced] = useState(url);
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(url), 350);
    return () => window.clearTimeout(timer);
  }, [url]);
  const valid = /^https?:\/\/.+/u.test(debounced);
  return useQuery({
    queryKey: ["node.probe", debounced],
    queryFn: () => call("node.probe", { nodeUrl: debounced }),
    enabled: valid,
    refetchInterval: (query) => (query.state.data?.reachable ? 8000 : 3000),
    staleTime: 0,
  });
}

export function NodeLine({
  status,
  loading,
}: {
  status: NodeStatus | undefined;
  loading?: boolean;
}) {
  if (loading || !status)
    return (
      <p className="flex items-center gap-2 text-[0.8rem] text-muted-foreground">
        <StatusDot tone="busy" pulse /> Looking for the node…
      </p>
    );
  if (!status.reachable)
    return (
      <p
        className="flex items-start gap-2 text-[0.8rem] leading-snug text-muted-foreground"
        data-testid="node-offline"
      >
        <StatusDot tone="off" className="mt-1" />
        <span>
          No Aztec node answers at this address yet. For a local network, run{" "}
          <code className="rounded bg-paper-2 px-1 py-0.5 font-mono text-[0.75rem] text-ink">
            aztec start --local-network
          </code>
          .
        </span>
      </p>
    );
  return (
    <p
      className="flex items-center gap-2 text-[0.8rem] text-muted-foreground"
      data-testid="node-online"
    >
      <StatusDot tone="ok" />
      <span>
        Connected to Aztec {status.nodeVersion}
        <span className="mx-1.5 text-border">/</span>
        block <span className="tabular text-ink">{number.format(status.blockNumber ?? 0)}</span>
      </span>
    </p>
  );
}
