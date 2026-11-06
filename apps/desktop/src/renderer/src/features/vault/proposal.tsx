import { useState } from "react";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";

import type { ExecutionProgress, ProposalView } from "@shared/types.ts";
import { CheckIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { toast } from "sonner";

import { Avatar, Explain } from "@/components/bits";
import { CopyValue } from "@/components/copy";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { date, formatUnits, short, until } from "@/lib/format";
import { call } from "@/lib/ipc";
import { EASE_OUT } from "@/lib/motion";
import { keys } from "@/lib/state";
import { cn } from "@/lib/utils";

import { useLabel } from "./overview";
import { Panel } from "./page-header";
import { ApprovalMeter, StatusLabel, intentLine, validApprovals } from "./proposal-bits";
import { useContacts, useVault } from "./use-vault";

export function ProposalDetail() {
  const { id } = useParams({ from: "/shell/vault/$address/proposals/$id" });
  const { address, summary, details } = useVault();
  const label = useLabel(useContacts().data);
  const proposal = useQuery({
    queryKey: keys.proposal(address, id),
    queryFn: () => call("proposals.get", { vault: address, id }),
    refetchInterval: 10_000,
  });
  const p = proposal.data;
  if (proposal.isLoading || !summary)
    return (
      <div className="space-y-4 pt-8">
        <Skeleton className="h-10 w-2/3" />
        <Skeleton className="h-64" />
      </div>
    );
  if (proposal.isError || !p)
    return (
      <p className="pt-10 text-sm text-coral">
        {proposal.error ? String(proposal.error) : "No such proposal."}
      </p>
    );
  const members = details.data?.committee.members ?? [];
  return (
    <>
      <header className="flex items-end justify-between gap-8 pt-6 pb-8">
        <div className="min-w-0">
          <Link
            to="/vault/$address/proposals"
            params={{ address }}
            className="text-xs text-muted-foreground transition-colors hover-fine:text-foreground"
          >
            Proposals
          </Link>
          <h1
            className="serif-display mt-1 text-[2.1rem] leading-[1.1]"
            data-testid="proposal-title"
          >
            {headline(p, label)}
          </h1>
          <p className="mt-2 flex items-center gap-3 text-sm text-muted-foreground">
            <StatusLabel status={p.status} />
            <span>
              proposed by {p.proposer.name}, deadline {until(p.expiresAt)}
            </span>
          </p>
          {p.intent.memo && <p className="mt-2 text-sm">{p.intent.memo}</p>}
        </div>
      </header>

      <div className="grid grid-cols-[minmax(0,1fr)_340px] gap-5">
        <div className="space-y-5">
          <Panel title="Calls">
            <ol className="divide-y divide-border border-t border-border">
              {p.calls.map((c, i) => (
                <li key={`${c.to}-${i}`} className="px-5 py-3">
                  <p className="font-mono text-[0.8rem]">
                    <span className="text-muted-foreground">{i}.</span> {c.fn}
                    <span className="text-muted-foreground">
                      ({c.args.length} args) on {c.toLabel ?? label(c.to)}
                      {c.isStatic ? ", static" : ""}
                    </span>
                  </p>
                  {c.args.length > 0 && (
                    <div className="mt-1.5 flex flex-wrap gap-x-3 gap-y-0.5">
                      {c.args.map((a, j) => (
                        <CopyValue
                          key={j}
                          value={argText(a)}
                          head={10}
                          tail={6}
                          label={`argument ${j}`}
                          className="text-[0.7rem] text-muted-foreground"
                        />
                      ))}
                    </div>
                  )}
                </li>
              ))}
            </ol>
          </Panel>
          <Panel title="Proposal">
            <dl className="divide-y divide-border border-t border-border px-5 text-sm">
              <Row label="Id">
                <CopyValue value={p.id} head={14} tail={8} label="proposal id" />
              </Row>
              <Row
                label="Approval message"
                tip="Poseidon2(tecpan.approval.v0, [vault, outer hash, config version, expires_at]). This is what each signer signs."
              >
                <CopyValue value={p.approvalMessage} head={14} tail={8} label="approval message" />
              </Row>
              <Row
                label="Outer hash"
                tip="Hash of the entrypoint payload. Your PXE recomputes it from the decoded calls before you sign."
              >
                <CopyValue value={p.outerHash} head={14} tail={8} label="outer hash" />
              </Row>
              <Row
                label="Tx nonce"
                tip="Pinned entrypoint nonce. A cancel proposal reuses it so this one can never execute."
              >
                <CopyValue value={p.txNonce} head={14} tail={8} label="tx nonce" />
              </Row>
              <Row label="Config version">
                <span
                  className={cn(
                    "font-mono text-[0.8rem]",
                    p.configVersion !== p.currentConfigVersion && "text-[#7a5200]",
                  )}
                >
                  v{p.configVersion}
                  {p.configVersion !== p.currentConfigVersion
                    ? ` (vault is at v${p.currentConfigVersion}; approvals are void)`
                    : ""}
                </span>
              </Row>
              <Row label="expires_at">
                <span className="font-mono text-[0.8rem]">
                  {p.expiresAt} ({date(p.expiresAt * 1000)})
                </span>
              </Row>
              <Row label="Fee">{p.feeMethod}</Row>
              {p.execution && (
                <Row label="Executed">
                  <CopyValue
                    value={p.execution.txHash}
                    head={14}
                    tail={8}
                    label="transaction hash"
                  />
                </Row>
              )}
            </dl>
          </Panel>
        </div>

        <aside className="space-y-5">
          <Panel
            title="Approvals"
            action={<ApprovalMeter approved={validApprovals(p)} threshold={p.threshold} />}
          >
            <ul className="divide-y divide-border border-t border-border" data-testid="approvals">
              {(members.length > 0
                ? members
                : p.approvals.map((a) => ({ index: a.index, name: a.name, identity: a.name }))
              ).map((m) => {
                const approval = p.approvals.find((a) => a.index === m.index);
                return (
                  <li key={m.index} className="flex items-center gap-3 px-5 py-2.5 text-sm">
                    <Avatar name={m.name} identity={m.identity} slot={m.index} size="sm" />
                    <span className="flex-1 truncate">
                      {m.name}
                      {m.index === p.mySlot + 1 && (
                        <span className="text-muted-foreground"> (you)</span>
                      )}
                    </span>
                    {approval ? (
                      approval.valid ? (
                        <span
                          className="flex items-center gap-1 text-xs text-olive-ink"
                          data-testid={`approved-${m.index}`}
                        >
                          <CheckIcon className="size-3.5" /> signed
                        </span>
                      ) : (
                        <span className="text-xs text-coral">invalid signature</span>
                      )
                    ) : (
                      <span className="text-xs text-muted-foreground">pending</span>
                    )}
                  </li>
                );
              })}
            </ul>
          </Panel>
          <Actions proposal={p} />
        </aside>
      </div>
    </>
  );
}

/** Field arguments that fit in u128 read as integers (amounts, ids); larger ones (addresses, hashes) stay hex. */
function argText(value: string): string {
  try {
    const n = BigInt(value);
    return n < 2n ** 128n ? n.toString() : value;
  } catch {
    return value;
  }
}

function headline(p: ProposalView, label: (a: string) => string): string {
  if (p.intent.kind === "transfer" && p.token)
    return `Transfer ${formatUnits(p.intent.amount, p.token.decimals)} ${p.token.symbol} to ${label(p.intent.to)}`;
  if (p.intent.kind === "cancel") return `Cancel ${short(p.intent.cancels, 8, 4)}`;
  const line = intentLine(p, label);
  return line.charAt(0).toUpperCase() + line.slice(1);
}

function Row({ label, tip, children }: { label: string; tip?: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[150px_minmax(0,1fr)] items-start gap-4 py-2.5">
      <dt className="text-muted-foreground">
        {tip ? <Explain tip={tip}>{label}</Explain> : label}
      </dt>
      <dd className="min-w-0">{children}</dd>
    </div>
  );
}

const PHASES: Record<ExecutionProgress["phase"], string> = {
  rebuilding: "Rebuilding the transaction and checking the outer hash",
  proving: "Proving and sending (client-side if proving is on)",
  sending: "Sending",
  mined: "Mined",
  failed: "Failed",
};

function Actions({ proposal: p }: { proposal: ProposalView }) {
  const navigate = useNavigate();
  const client = useQueryClient();
  const { address } = useVault();
  const [busy, setBusy] = useState<"approve" | "execute" | "cancel" | null>(null);
  const execution = useQuery<ExecutionProgress | null>({
    queryKey: keys.execution(address, p.id),
    queryFn: () => null,
    staleTime: Number.POSITIVE_INFINITY,
  });
  const open = p.status === "collecting" || p.status === "ready";

  const act = async (what: "approve" | "execute" | "cancel") => {
    setBusy(what);
    try {
      if (what === "approve") await call("proposals.approve", { vault: address, id: p.id });
      if (what === "execute") await call("proposals.execute", { vault: address, id: p.id });
      if (what === "cancel") {
        const cancel = await call("proposals.cancel", { vault: address, id: p.id });
        await navigate({ to: "/vault/$address/proposals/$id", params: { address, id: cancel.id } });
      }
      await client.invalidateQueries({ queryKey: keys.vaultData(address) });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      if (message !== "Approval declined") toast.error(`${what} failed`, { description: message });
    } finally {
      setBusy(null);
    }
  };

  const progress = execution.data;
  return (
    <div className="space-y-2">
      <AnimatePresence initial={false}>
        {progress && busy === "execute" && (
          <motion.p
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2, ease: EASE_OUT }}
            className="flex items-center gap-2 rounded-lg bg-paper-2 px-3 py-2 text-xs"
          >
            <Spinner className="size-3.5" /> {PHASES[progress.phase]}
          </motion.p>
        )}
      </AnimatePresence>
      {open && !p.approvedByMe && (
        <Button
          size="lg"
          className="w-full"
          disabled={busy !== null}
          onClick={() => void act("approve")}
          data-testid="proposal-approve"
        >
          {busy === "approve" && <Spinner />}
          Approve
        </Button>
      )}
      {p.status === "ready" && (
        <Button
          size="lg"
          variant={p.approvedByMe ? "default" : "outline"}
          className="w-full"
          disabled={busy !== null}
          onClick={() => void act("execute")}
          data-testid="proposal-execute"
        >
          {busy === "execute" && <Spinner />}
          Execute
        </Button>
      )}
      {open && p.intent.kind !== "cancel" && (
        <Button
          variant="ghost"
          className="w-full text-muted-foreground"
          disabled={busy !== null}
          onClick={() => void act("cancel")}
        >
          {busy === "cancel" && <Spinner />}
          Propose cancellation
        </Button>
      )}
      <p className="text-xs leading-snug text-muted-foreground">
        {p.status === "collecting" &&
          `Needs ${p.threshold - validApprovals(p)} more approval${p.threshold - validApprovals(p) === 1 ? "" : "s"}. Any member can execute once ${p.threshold} have signed.`}
        {p.status === "ready" &&
          "Threshold reached. Executing proves the entrypoint with the collected Schnorr witness."}
        {p.status === "stale" &&
          "The vault's config changed after this was proposed. Propose it again."}
        {p.status === "expired" && "Past expires_at. It can no longer execute."}
        {p.status === "executed" && "Executed. The receipt was replicated to every keeper."}
        {p.status === "cancelled" &&
          `Cancelled by ${short(p.cancelledBy ?? "", 8, 4)}; its nonce is consumed.`}
      </p>
    </div>
  );
}
