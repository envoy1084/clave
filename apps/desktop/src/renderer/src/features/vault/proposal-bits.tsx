import type { ProposalStatus, ProposalView } from "@shared/types.ts";

import { formatUnits, short } from "@/lib/format";
import { cn } from "@/lib/utils";

const STATUS: Record<ProposalStatus, { label: string; className: string }> = {
  collecting: { label: "Collecting", className: "bg-peri-soft text-peri-ink" },
  ready: { label: "Ready", className: "bg-olive-soft text-olive-ink" },
  executed: { label: "Executed", className: "bg-paper-2 text-ink" },
  cancelled: { label: "Cancelled", className: "bg-paper-2 text-muted-foreground" },
  expired: { label: "Expired", className: "bg-paper-2 text-muted-foreground" },
  stale: { label: "Stale config", className: "bg-[#fbf0d6] text-[#7a5200]" },
};

export function StatusLabel({ status }: { status: ProposalStatus }) {
  const s = STATUS[status];
  return (
    <span
      className={cn(
        "inline-flex h-5 items-center rounded-md px-1.5 text-[0.7rem] font-medium",
        s.className,
      )}
    >
      {s.label}
    </span>
  );
}

/** k slots; filled as valid approvals arrive. */
export function ApprovalMeter({ approved, threshold }: { approved: number; threshold: number }) {
  return (
    <span
      className="inline-flex items-center gap-2"
      aria-label={`${approved} of ${threshold} approvals`}
    >
      <span className="flex gap-0.5">
        {Array.from({ length: threshold }, (_, i) => (
          <span
            key={i}
            className={cn(
              "h-1.5 w-3.5 rounded-sm transition-colors duration-300",
              i < approved ? "bg-olive" : "bg-[#e1ddd1]",
            )}
          />
        ))}
      </span>
      <span className="tabular font-mono text-[0.72rem] text-muted-foreground">
        {Math.min(approved, threshold)}/{threshold}
      </span>
    </span>
  );
}

/** One line describing what a proposal does. */
export function intentLine(p: ProposalView, label: (address: string) => string): string {
  const { intent } = p;
  if (intent.kind === "transfer")
    return `transfer ${p.token ? `${formatUnits(intent.amount, p.token.decimals)} ${p.token.symbol}` : `${intent.amount} units`} to ${label(intent.to)}`;
  if (intent.kind === "cancel") return `cancel ${short(intent.cancels, 8, 4)} (burn nonce)`;
  if (intent.calls.length === 0) return intent.memo ?? "no calls";
  return intent.calls.map((c) => `${c.fn} on ${label(c.to)}`).join(", ");
}

export const validApprovals = (p: ProposalView) => p.approvals.filter((a) => a.valid).length;
