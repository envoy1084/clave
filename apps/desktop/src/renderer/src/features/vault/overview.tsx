import { useState } from "react";

import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";

import { ArrowDownLeftIcon, ArrowRightIcon, EyeOffIcon } from "lucide-react";
import { QRCodeSVG } from "qrcode.react";

import { Avatar, Seats, StatusDot, type Tone } from "@/components/bits";
import { Terrain } from "@/components/brand/terrain";
import { CopyValue } from "@/components/copy";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import { date, number, ofN } from "@/lib/format";
import { call } from "@/lib/ipc";
import { keys, useNodeStatus } from "@/lib/state";

import { PageHeader, Panel } from "./page-header";
import { useVault } from "./use-vault";

export function Overview() {
  const { address, summary, details } = useVault();
  const [receive, setReceive] = useState(false);
  const d = details.data;
  if (!summary) return null;
  return (
    <>
      <PageHeader
        title={<span data-testid="vault-title">{summary.name}</span>}
        description={
          <span className="flex items-center gap-3">
            <CopyValue value={address} head={10} tail={8} label="vault address" />
            <span className="h-3 w-px bg-border" />
            {ofN(summary.threshold, summary.members)} approve
            <span className="h-3 w-px bg-border" />
            {ofN(summary.viewingThreshold, summary.members)} read
          </span>
        }
        actions={
          <Button variant="outline" onClick={() => setReceive(true)}>
            <ArrowDownLeftIcon data-icon="inline-start" /> Receive
          </Button>
        }
      />

      <div className="grid grid-cols-[minmax(0,1fr)_340px] gap-5">
        <div className="space-y-5">
          <section className="relative overflow-hidden rounded-2xl bg-[#0E1530] text-white shadow-[0_24px_48px_-28px_rgb(14_21_48/0.6)]">
            <Terrain
              seed={summary.foundedAt % 97}
              shade="left"
              clusters={[
                [520, 60, 9, 6],
                [610, 200, 6, 5],
              ]}
              className="absolute inset-0"
            />
            <div className="relative px-7 py-7">
              <p className="flex items-center gap-2 text-xs text-white/60">
                <EyeOffIcon className="size-3.5" /> Private balance
              </p>
              <p
                className="mt-3 font-serif text-[2.75rem] leading-none font-light tracking-[-0.02em] text-white/90"
                data-testid="vault-balance"
              >
                Nothing here yet
              </p>
              <p className="mt-3 max-w-[40ch] text-sm leading-relaxed text-white/70">
                Tokens sent to this vault appear here, readable only on members' devices, and only
                when {summary.viewingThreshold} keepers answer together.
              </p>
              <Button variant="signal" className="mt-6" onClick={() => setReceive(true)}>
                Get the vault address
              </Button>
            </div>
          </section>

          <Panel
            title="Members"
            action={
              <Link
                to="/vault/$address/members"
                params={{ address }}
                className="flex items-center gap-1 text-[0.8rem] text-muted-foreground transition-colors hover-fine:text-foreground"
              >
                All details <ArrowRightIcon className="size-3.5" />
              </Link>
            }
          >
            <ul className="divide-y divide-border px-5 pt-1 pb-2">
              {d
                ? d.committee.members.map((m) => (
                    <li key={m.identity} className="flex items-center gap-3 py-3">
                      <Avatar name={m.name} identity={m.identity} slot={m.index} />
                      <p className="min-w-0 flex-1 truncate text-sm font-medium">
                        {m.name}{" "}
                        {m.you && <span className="font-normal text-muted-foreground">(you)</span>}
                      </p>
                      <span className="text-[0.75rem] text-muted-foreground">Member {m.index}</span>
                    </li>
                  ))
                : Array.from({ length: summary.members }, (_, i) => (
                    <li key={i} className="flex items-center gap-3 py-3">
                      <Skeleton className="size-8 rounded-full" />
                      <Skeleton className="h-4 w-24" />
                    </li>
                  ))}
            </ul>
          </Panel>
        </div>

        <div className="space-y-5">
          <Health />
          <Panel title="Rules">
            <dl className="space-y-4 px-5 pt-3 pb-5 text-sm">
              <div>
                <dt className="flex items-center justify-between text-muted-foreground">
                  To spend{" "}
                  <span className="tabular text-foreground">
                    {ofN(summary.threshold, summary.members)}
                  </span>
                </dt>
                <dd className="mt-2">
                  <Seats k={summary.threshold} n={summary.members} tone="olive" />
                </dd>
              </div>
              <div>
                <dt className="flex items-center justify-between text-muted-foreground">
                  To read{" "}
                  <span className="tabular text-foreground">
                    {ofN(summary.viewingThreshold, summary.members)}
                  </span>
                </dt>
                <dd className="mt-2">
                  <Seats k={summary.viewingThreshold} n={summary.members} tone="peri" />
                </dd>
              </div>
            </dl>
          </Panel>
          <Panel title="Founded">
            <div className="space-y-2 px-5 pt-2 pb-5 text-sm">
              <p className="text-muted-foreground">{date(summary.foundedAt)}</p>
              {d?.founding.deployTx && (
                <p className="flex items-center justify-between gap-3">
                  <span className="text-muted-foreground">Deployment</span>
                  <CopyValue
                    value={d.founding.deployTx}
                    head={8}
                    tail={6}
                    label="deployment transaction"
                  />
                </p>
              )}
            </div>
          </Panel>
        </div>
      </div>

      <Dialog open={receive} onOpenChange={setReceive}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Receive into {summary.name}</DialogTitle>
            <DialogDescription>
              Send private tokens on Aztec to this address. Only members can see what arrives.
            </DialogDescription>
          </DialogHeader>
          <div className="grid place-items-center rounded-xl bg-paper p-6">
            <QRCodeSVG value={address} size={200} bgColor="#FBF8F2" fgColor="#15180F" level="M" />
          </div>
          <div className="rounded-xl bg-paper-2 px-4 py-3">
            <CopyValue
              value={address}
              full
              label="vault address"
              className="text-[0.75rem] leading-relaxed"
            />
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}

/** Live checks a member cares about: does the chain agree, is my keeper serving, is my engine in sync. */
function Health() {
  const { address, summary, details } = useVault();
  const node = useNodeStatus();
  const pxe = useQuery({
    queryKey: keys.pxe,
    queryFn: () => call("pxe.status"),
    refetchInterval: 8000,
  });
  const d = details.data;
  const rows: { label: string; detail: string; tone: Tone }[] = [
    d?.config
      ? d.config.matchesSetup
        ? {
            label: "Rules match on chain",
            detail: `Config version ${d.config.version}`,
            tone: "ok",
          }
        : {
            label: "Rules differ on chain",
            detail: "Check Internals before using this vault",
            tone: "bad",
          }
      : { label: "Reading the chain", detail: "Waiting for the vault's config", tone: "busy" },
    d
      ? d.committee.serving
        ? {
            label: "Your keeper is serving",
            detail: `Holds share ${summary?.myIndex} of ${summary?.members}`,
            tone: "ok",
          }
        : {
            label: "Your keeper is not serving",
            detail: `Status ${d.committee.keeperStatus}`,
            tone: "warn",
          }
      : { label: "Checking your keeper", detail: "…", tone: "busy" },
    node.data?.reachable
      ? {
          label: "Engine in sync",
          detail:
            pxe.data?.syncedBlock !== undefined
              ? `Block ${number.format(pxe.data.syncedBlock)} of ${number.format(node.data.blockNumber ?? 0)}`
              : "Syncing",
          tone: "ok",
        }
      : { label: "Network offline", detail: node.data?.url ?? "", tone: "bad" },
  ];
  return (
    <Panel title="Health">
      <ul className="space-y-3.5 px-5 pt-3 pb-5" data-testid="vault-health" data-vault={address}>
        {rows.map((row) => (
          <li key={row.label} className="flex items-start gap-3">
            <StatusDot tone={row.tone} pulse={row.tone === "busy"} className="mt-1.5" />
            <div className="min-w-0">
              <p className="text-sm">{row.label}</p>
              <p className="truncate text-[0.75rem] text-muted-foreground">{row.detail}</p>
            </div>
          </li>
        ))}
      </ul>
    </Panel>
  );
}
