import { useState } from "react";

import { Link, useNavigate } from "@tanstack/react-router";

import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { short, until } from "@/lib/format";

import { useLabel } from "./overview";
import { PageHeader } from "./page-header";
import { ApprovalMeter, StatusLabel, intentLine, validApprovals } from "./proposal-bits";
import { useContacts, useProposals, useVault } from "./use-vault";

export function Proposals() {
  const navigate = useNavigate();
  const { address } = useVault();
  const proposals = useProposals(address);
  const label = useLabel(useContacts().data);
  const [filter, setFilter] = useState<"open" | "all">("open");
  const rows = (proposals.data ?? []).filter(
    (p) => filter === "all" || p.status === "collecting" || p.status === "ready",
  );
  return (
    <>
      <PageHeader
        title="Proposals"
        description="Stored and replicated by member keepers. Each is rebuilt by your PXE before you sign or execute it."
        actions={
          <Button
            onClick={() =>
              void navigate({ to: "/vault/$address/proposals/new", params: { address } })
            }
          >
            New proposal
          </Button>
        }
      />
      <ToggleGroup
        value={[filter]}
        onValueChange={(v) => v[0] && setFilter(v[0] as "open" | "all")}
        className="mb-4 rounded-xl bg-paper-2 p-1"
      >
        <ToggleGroupItem
          value="open"
          className="h-8 rounded-lg px-3 data-[pressed]:bg-card data-[pressed]:shadow-sm"
        >
          Open
        </ToggleGroupItem>
        <ToggleGroupItem
          value="all"
          className="h-8 rounded-lg px-3 data-[pressed]:bg-card data-[pressed]:shadow-sm"
        >
          All
        </ToggleGroupItem>
      </ToggleGroup>
      <section className="overflow-hidden rounded-2xl bg-card shadow-[0_0_0_1px_var(--border)]">
        {proposals.isLoading ? (
          <div className="space-y-2 p-5">
            <Skeleton className="h-12" />
            <Skeleton className="h-12" />
          </div>
        ) : proposals.isError ? (
          <p className="p-5 text-sm text-coral">{String(proposals.error)}</p>
        ) : rows.length === 0 ? (
          <p className="p-5 text-sm text-muted-foreground">
            {filter === "open" ? "No open proposals." : "No proposals yet."}
          </p>
        ) : (
          <table className="w-full text-sm" data-testid="proposals">
            <thead>
              <tr className="text-left text-xs text-muted-foreground">
                <th className="py-2.5 pl-5 font-normal">Proposal</th>
                <th className="py-2.5 font-normal">Approvals</th>
                <th className="py-2.5 font-normal">Expiry</th>
                <th className="py-2.5 pr-5 text-right font-normal">Status</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border border-t border-border">
              {rows.map((p) => (
                <tr key={p.id} className="transition-colors duration-150 hover-fine:bg-paper">
                  <td className="py-3 pl-5">
                    <Link
                      to="/vault/$address/proposals/$id"
                      params={{ address, id: p.id }}
                      className="block"
                    >
                      <span className="block max-w-[46ch] truncate">{intentLine(p, label)}</span>
                      <span className="font-mono text-[0.7rem] text-muted-foreground">
                        {short(p.id, 8, 4)}, member {p.proposer.index} ({p.proposer.name})
                      </span>
                    </Link>
                  </td>
                  <td className="py-3">
                    <ApprovalMeter approved={validApprovals(p)} threshold={p.threshold} />
                  </td>
                  <td className="py-3 font-mono text-[0.75rem] text-muted-foreground">
                    {until(p.expiresAt)}
                  </td>
                  <td className="py-3 pr-5 text-right">
                    <StatusLabel status={p.status} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </>
  );
}
