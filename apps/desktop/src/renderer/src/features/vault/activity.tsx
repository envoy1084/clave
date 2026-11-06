import { Link } from "@tanstack/react-router";

import { CopyValue } from "@/components/copy";
import { Skeleton } from "@/components/ui/skeleton";
import { date, formatUnits, number } from "@/lib/format";

import { useLabel } from "./overview";
import { PageHeader } from "./page-header";
import { StatusLabel, intentLine } from "./proposal-bits";
import { useActivity, useContacts, useVault } from "./use-vault";

export function Activity() {
  const { address } = useVault();
  const activity = useActivity(address);
  const label = useLabel(useContacts().data);
  return (
    <>
      <PageHeader
        title="Activity"
        description="Finished proposals from the keepers, and incoming private transfers decrypted by your PXE."
      />
      <section className="overflow-hidden rounded-2xl bg-card shadow-[0_0_0_1px_var(--border)]">
        {activity.isLoading ? (
          <div className="space-y-2 p-5">
            <Skeleton className="h-10" />
            <Skeleton className="h-10" />
          </div>
        ) : activity.isError ? (
          <p className="p-5 text-sm text-coral">{String(activity.error)}</p>
        ) : !activity.data || activity.data.length === 0 ? (
          <p className="p-5 text-sm text-muted-foreground">
            Nothing yet. Executed, cancelled and expired proposals, and received notes, appear here.
          </p>
        ) : (
          <ul className="divide-y divide-border" data-testid="activity">
            {activity.data.map((entry) =>
              entry.kind === "received" ? (
                <li
                  key={`${entry.txHash}-${entry.amount}`}
                  className="grid grid-cols-[90px_minmax(0,1fr)_auto] items-center gap-4 px-5 py-3 text-sm"
                >
                  <span className="text-xs font-medium text-olive-ink">Received</span>
                  <span className="min-w-0">
                    <span className="block">
                      {formatUnits(entry.amount, entry.token.decimals)} {entry.token.symbol} from{" "}
                      {label(entry.from)}
                    </span>
                    <CopyValue
                      value={entry.txHash}
                      head={10}
                      tail={6}
                      label="transaction hash"
                      className="text-[0.7rem] text-muted-foreground"
                    />
                  </span>
                  <span className="font-mono text-[0.72rem] text-muted-foreground">
                    block {number.format(entry.block)}
                  </span>
                </li>
              ) : (
                <li key={entry.proposal.id}>
                  <Link
                    to="/vault/$address/proposals/$id"
                    params={{ address, id: entry.proposal.id }}
                    className="grid grid-cols-[90px_minmax(0,1fr)_auto] items-center gap-4 px-5 py-3 text-sm transition-colors duration-150 hover-fine:bg-paper"
                  >
                    <StatusLabel status={entry.proposal.status} />
                    <span className="truncate">{intentLine(entry.proposal, label)}</span>
                    <span className="font-mono text-[0.72rem] text-muted-foreground">
                      {entry.proposal.execution
                        ? date(entry.proposal.execution.at * 1000)
                        : `expired ${date(entry.proposal.expiresAt * 1000)}`}
                    </span>
                  </Link>
                </li>
              ),
            )}
          </ul>
        )}
      </section>
    </>
  );
}
