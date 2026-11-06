import { useState } from "react";

import type { CommitteeMember } from "@shared/types.ts";
import { ChevronDownIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";

import { Avatar, Explain } from "@/components/bits";
import { CopyValue } from "@/components/copy";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { ofN } from "@/lib/format";
import { EASE_OUT } from "@/lib/motion";
import { cn } from "@/lib/utils";

import { PageHeader } from "./page-header";
import { useVault } from "./use-vault";

export function Members() {
  const { summary, details, technical } = useVault();
  const members = details.data?.committee.members;
  if (!summary) return null;
  return (
    <>
      <PageHeader
        title="Members"
        description={`Everyone holds a signing key and runs a keeper. Any ${ofN(summary.threshold, summary.members)} approve payments; any ${ofN(summary.viewingThreshold, summary.members)} keepers together can read the vault.`}
      />
      <ul className="space-y-3" data-testid="members">
        {members
          ? members.map((m, i) => (
              <MemberRow key={m.identity} member={m} index={i} open={technical} />
            ))
          : Array.from({ length: summary.members }, (_, i) => (
              <Skeleton key={i} className="h-[72px] rounded-2xl" />
            ))}
      </ul>
    </>
  );
}

function MemberRow({
  member,
  index,
  open: initiallyOpen,
}: {
  member: CommitteeMember;
  index: number;
  open: boolean;
}) {
  const [open, setOpen] = useState(initiallyOpen);
  return (
    <motion.li
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: index * 0.04, duration: 0.3, ease: EASE_OUT }}
      className="overflow-hidden rounded-2xl bg-card shadow-[0_0_0_1px_var(--border),0_1px_2px_rgb(21_24_15/0.03)]"
    >
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        className="flex w-full items-center gap-4 px-5 py-4 text-left transition-colors duration-150 hover-fine:bg-paper/60"
      >
        <Avatar name={member.name} identity={member.identity} slot={member.index} size="lg" />
        <div className="min-w-0 flex-1">
          <p className="flex items-center gap-2 font-medium">
            {member.name}
            {member.you && <Badge variant="secondary">You</Badge>}
            {member.index === 1 && <Badge variant="outline">Founder</Badge>}
          </p>
          <p className="text-[0.8rem] text-muted-foreground">
            Member {member.index}, signer and viewing-share holder
          </p>
        </div>
        <span className="hidden font-mono text-[0.75rem] text-muted-foreground md:block">
          {member.peerId.slice(0, 16)}…
        </span>
        <ChevronDownIcon
          className={cn(
            "size-4 text-muted-foreground transition-transform duration-200 ease-(--ease-out)",
            open && "rotate-180",
          )}
        />
      </button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.26, ease: EASE_OUT }}
            className="overflow-hidden"
          >
            <dl className="grid grid-cols-[180px_minmax(0,1fr)] gap-x-6 gap-y-2.5 border-t border-border bg-paper/50 px-5 py-4 text-[0.8rem]">
              <Term tip="The public key this member's approvals are checked against, by the vault contract itself.">
                Signing key
              </Term>
              <dd className="min-w-0 space-y-0.5">
                <CopyValue value={member.signerKey.x} head={14} tail={8} label="signing key x" />
                <CopyValue value={member.signerKey.y} head={14} tail={8} label="signing key y" />
              </dd>
              <Term tip="This member's public share of the viewing key. Their keeper proves every answer against it, so a wrong answer is caught.">
                Verification share
              </Term>
              <dd className="min-w-0 space-y-0.5">
                <CopyValue
                  value={member.verificationShare.x}
                  head={14}
                  tail={8}
                  label="verification share x"
                />
                <CopyValue
                  value={member.verificationShare.y}
                  head={14}
                  tail={8}
                  label="verification share y"
                />
              </dd>
              <Term tip="The keeper's long-term Ed25519 identity; it signs every message the keeper sends.">
                Keeper identity
              </Term>
              <dd className="min-w-0">
                <CopyValue value={member.identity} head={14} tail={8} label="keeper identity" />
              </dd>
              <Term tip="How other keepers address this one on the peer-to-peer network. Derived from the identity.">
                Peer id
              </Term>
              <dd className="min-w-0">
                <CopyValue value={member.peerId} head={14} tail={8} label="peer id" />
              </dd>
              <Term tip="X25519 key used to seal key shares and mailbox messages to this keeper.">
                Transport key
              </Term>
              <dd className="min-w-0">
                <CopyValue value={member.transport} head={14} tail={8} label="transport key" />
              </dd>
              <Term tip="Where this keeper says it can be reached: direct addresses first, relay circuits last.">
                Addresses
              </Term>
              <dd className="min-w-0 space-y-0.5">
                {member.addresses.length === 0 ? (
                  <span className="text-muted-foreground">None advertised</span>
                ) : (
                  member.addresses.map((a) => (
                    <CopyValue key={a} value={a} full label="address" className="text-[0.72rem]" />
                  ))
                )}
              </dd>
            </dl>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.li>
  );
}

function Term({ children, tip }: { children: React.ReactNode; tip: string }) {
  return (
    <dt className="pt-0.5 text-muted-foreground">
      <Explain tip={tip}>{children}</Explain>
    </dt>
  );
}
