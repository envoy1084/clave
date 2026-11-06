import { type FormEvent, useState } from "react";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";

import type { MemberReachability } from "@shared/types.ts";
import { QRCodeSVG } from "qrcode.react";
import { toast } from "sonner";

import { StatusDot, type Tone } from "@/components/bits";
import { CopyValue } from "@/components/copy";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { ago, formatUnits, number, short, until } from "@/lib/format";
import { call } from "@/lib/ipc";
import { keys, useNodeStatus } from "@/lib/state";

import { PageHeader, Panel } from "./page-header";
import { ApprovalMeter, StatusLabel, intentLine, validApprovals } from "./proposal-bits";
import { useContacts, usePeers, useProposals, useTokens, useVault } from "./use-vault";

export function Overview() {
  const navigate = useNavigate();
  const { address, summary, details } = useVault();
  const [receive, setReceive] = useState(false);
  if (!summary) return null;
  const config = details.data?.config;
  return (
    <>
      <PageHeader
        title={<span data-testid="vault-title">{summary.name}</span>}
        description={
          <span className="flex flex-wrap items-center gap-x-3 gap-y-1">
            <CopyValue value={address} head={10} tail={8} label="vault address" />
            <Sep />
            <span>
              spend <b className="font-medium text-foreground">{summary.threshold}</b> of{" "}
              {summary.members}
            </span>
            <Sep />
            <span>
              view <b className="font-medium text-foreground">{summary.viewingThreshold}</b> of{" "}
              {summary.members}
            </span>
            <Sep />
            <span>config v{config?.version ?? "…"}</span>
          </span>
        }
        actions={
          <>
            <Button variant="outline" onClick={() => setReceive(true)}>
              Receive
            </Button>
            <Button
              onClick={() =>
                void navigate({ to: "/vault/$address/proposals/new", params: { address } })
              }
              data-testid="new-proposal"
            >
              New proposal
            </Button>
          </>
        }
      />
      <div className="grid grid-cols-[minmax(0,1fr)_340px] gap-5">
        <div className="space-y-5">
          <Balances />
          <OpenProposals />
        </div>
        <div className="space-y-5">
          <Committee />
          <Health />
        </div>
      </div>
      <Dialog open={receive} onOpenChange={setReceive}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Receive</DialogTitle>
            <DialogDescription>
              Send private notes to this address. Members decrypt them through threshold ECDH.
            </DialogDescription>
          </DialogHeader>
          <div className="grid place-items-center rounded-xl bg-paper p-6">
            <QRCodeSVG value={address} size={188} bgColor="#FBF8F2" fgColor="#15180F" level="M" />
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

const Sep = () => <span className="h-3 w-px bg-border" aria-hidden />;

function Balances() {
  const { address } = useVault();
  const tokens = useTokens(address);
  const node = useNodeStatus();
  const client = useQueryClient();
  const [adding, setAdding] = useState(false);
  const [deploying, setDeploying] = useState(false);
  const local = node.data?.l1ChainId === 31337;

  const deployTest = async () => {
    setDeploying(true);
    try {
      const token = await call("tokens.deployTest", { vault: address });
      toast.success(`Deployed ${token.symbol}`, {
        description: "Minted 10,000 TTT privately to the vault.",
      });
      await client.invalidateQueries({ queryKey: keys.vaultData(address) });
    } catch (e) {
      toast.error("Deploy failed", { description: e instanceof Error ? e.message : String(e) });
    } finally {
      setDeploying(false);
    }
  };

  return (
    <Panel
      title="Private balances"
      action={
        <div className="flex gap-1.5">
          {local && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => void deployTest()}
              disabled={deploying}
              data-testid="deploy-test-token"
            >
              {deploying && <Spinner className="size-3.5" />}
              Deploy test token
            </Button>
          )}
          <Button variant="ghost" size="sm" onClick={() => setAdding(true)}>
            Add token
          </Button>
        </div>
      }
    >
      {tokens.isLoading ? (
        <div className="space-y-2 px-5 pt-2 pb-5">
          <Skeleton className="h-9" />
          <Skeleton className="h-9" />
        </div>
      ) : tokens.isError ? (
        <p className="px-5 pt-2 pb-5 text-sm text-coral">{String(tokens.error)}</p>
      ) : tokens.data && tokens.data.length > 0 ? (
        <table className="w-full text-sm" data-testid="balances">
          <thead>
            <tr className="text-left text-xs text-muted-foreground">
              <th className="py-2 pl-5 font-normal">Token</th>
              <th className="py-2 font-normal">Contract</th>
              <th className="py-2 pr-5 text-right font-normal">Balance</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border border-t border-border">
            {tokens.data.map(({ token, balance, error }) => (
              <tr key={token.address}>
                <td className="py-2.5 pl-5">
                  <span className="font-medium">{token.symbol}</span>{" "}
                  <span className="text-muted-foreground">{token.name}</span>
                </td>
                <td className="py-2.5">
                  <CopyValue value={token.address} head={8} tail={4} label="token contract" />
                </td>
                <td
                  className="tabular py-2.5 pr-5 text-right font-mono text-[0.85rem]"
                  data-testid={`balance-${token.symbol}`}
                >
                  {balance !== undefined ? (
                    formatUnits(balance, token.decimals)
                  ) : (
                    <span className="text-xs text-coral" title={error}>
                      unreadable
                    </span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <p className="px-5 pt-2 pb-5 text-sm text-muted-foreground">
          No tokens tracked. Add a token contract by address
          {local ? ", or deploy a test token" : ""}.
        </p>
      )}
      <AddToken open={adding} onOpenChange={setAdding} />
    </Panel>
  );
}

function AddToken({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { address } = useVault();
  const client = useQueryClient();
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const info = await call("tokens.add", { vault: address, token: token.trim() });
      toast.success(`Tracking ${info.symbol}`);
      await client.invalidateQueries({ queryKey: keys.tokens(address) });
      onOpenChange(false);
      setToken("");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add token</DialogTitle>
          <DialogDescription>
            A Token contract (aztec-nr standard). Its instance is fetched from the node and
            registered in your PXE.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={(e) => void submit(e)} className="space-y-3">
          <Input
            autoFocus
            value={token}
            onChange={(e) => setToken(e.target.value)}
            placeholder="0x… contract address"
            className="h-10 font-mono text-[0.8rem]"
            aria-label="Token contract address"
          />
          {error && <p className="text-sm text-coral">{error}</p>}
          <Button
            type="submit"
            className="w-full"
            disabled={!/^0x[0-9a-fA-F]{64}$/u.test(token.trim()) || busy}
          >
            {busy && <Spinner />}
            Add
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function OpenProposals() {
  const { address } = useVault();
  const proposals = useProposals(address);
  const contacts = useContacts();
  const label = useLabel(contacts.data);
  const open = (proposals.data ?? []).filter(
    (p) => p.status === "collecting" || p.status === "ready",
  );
  return (
    <Panel
      title="Open proposals"
      action={
        <Link
          to="/vault/$address/proposals"
          params={{ address }}
          className="text-xs text-muted-foreground transition-colors hover-fine:text-foreground"
        >
          All proposals
        </Link>
      }
    >
      {proposals.isLoading ? (
        <div className="px-5 pt-2 pb-5">
          <Skeleton className="h-12" />
        </div>
      ) : proposals.isError ? (
        <p className="px-5 pt-2 pb-5 text-sm text-coral">{String(proposals.error)}</p>
      ) : open.length === 0 ? (
        <p className="px-5 pt-2 pb-5 text-sm text-muted-foreground">No open proposals.</p>
      ) : (
        <ul className="divide-y divide-border border-t border-border">
          {open.map((p) => (
            <li key={p.id}>
              <Link
                to="/vault/$address/proposals/$id"
                params={{ address, id: p.id }}
                className="grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-4 px-5 py-3 transition-colors duration-150 hover-fine:bg-paper"
              >
                <span className="min-w-0">
                  <span className="block truncate text-sm">{intentLine(p, label)}</span>
                  <span className="font-mono text-[0.7rem] text-muted-foreground">
                    {short(p.id, 8, 4)}, by {p.proposer.name}, expires {until(p.expiresAt)}
                  </span>
                </span>
                <ApprovalMeter approved={validApprovals(p)} threshold={p.threshold} />
                <StatusLabel status={p.status} />
              </Link>
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}

/** Address book labels, else a shortened address. */
export function useLabel(contacts: readonly { address: string; label: string }[] | undefined) {
  return (address: string) =>
    contacts?.find((c) => c.address.toLowerCase() === address.toLowerCase())?.label ??
    short(address, 8, 4);
}

function reach(p: MemberReachability): { tone: Tone; text: string } {
  if (p.you) return { tone: "ok", text: "this device" };
  if (p.connected) return { tone: "ok", text: p.relayed ? "relayed" : "direct" };
  return { tone: "off", text: p.lastSeen ? `seen ${ago(p.lastSeen * 1000)}` : "not seen" };
}

function Committee() {
  const { address, summary } = useVault();
  const peers = usePeers(address);
  const reachable = (peers.data ?? []).filter((p) => p.you || p.connected).length;
  const tv = summary?.viewingThreshold ?? 2;
  return (
    <Panel
      title="Committee"
      action={
        peers.data && (
          <span className={`text-xs ${reachable >= tv ? "text-olive-ink" : "text-coral"}`}>
            ECDH {reachable >= tv ? "available" : "unavailable"} ({reachable}/{peers.data.length},
            t_v {tv})
          </span>
        )
      }
    >
      {peers.isError ? (
        <p className="px-5 pt-2 pb-5 text-sm text-muted-foreground">
          Reachability needs a keeper with peers.status.
        </p>
      ) : !peers.data ? (
        <div className="px-5 pt-2 pb-5">
          <Skeleton className="h-20" />
        </div>
      ) : (
        <ul className="divide-y divide-border border-t border-border" data-testid="committee">
          {peers.data.map((p) => {
            const r = reach(p);
            return (
              <li key={p.identity} className="flex items-center gap-3 px-5 py-2.5 text-sm">
                <StatusDot tone={r.tone} />
                <span className="min-w-0 flex-1 truncate">
                  {p.name}
                  <span className="ml-2 font-mono text-[0.7rem] text-muted-foreground">
                    {short(p.peerId, 8, 4)}
                  </span>
                </span>
                <span className="text-xs text-muted-foreground">
                  {r.text}
                  {p.rttMs !== null && p.connected && !p.you ? `, ${p.rttMs} ms` : ""}
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </Panel>
  );
}

function Health() {
  const { summary, details } = useVault();
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
            label: "On-chain config matches setup",
            detail: "signer set and committee commitments",
            tone: "ok",
          }
        : {
            label: "On-chain config differs",
            detail: "compare commitments under Internals",
            tone: "bad",
          }
      : {
          label: "Config note not read yet",
          detail: "needs the node and t_v keepers",
          tone: "busy",
        },
    d
      ? d.committee.serving
        ? {
            label: `Keeper serving share ${summary?.myIndex}`,
            detail: `epoch ${d.committee.epoch}`,
            tone: "ok",
          }
        : { label: "Keeper not serving", detail: d.committee.keeperStatus, tone: "warn" }
      : { label: "Keeper", detail: "…", tone: "busy" },
    node.data?.reachable
      ? {
          label: "PXE sync",
          detail:
            pxe.data?.syncedBlock !== undefined
              ? `block ${number.format(pxe.data.syncedBlock)} / ${number.format(node.data.blockNumber ?? 0)}`
              : "syncing",
          tone: "ok",
        }
      : { label: "Node unreachable", detail: node.data?.url ?? "", tone: "bad" },
  ];
  return (
    <Panel title="Health">
      <ul className="space-y-3 px-5 pt-3 pb-5" data-testid="vault-health">
        {rows.map((row) => (
          <li key={row.label} className="flex items-start gap-3">
            <StatusDot tone={row.tone} className="mt-1.5" />
            <div className="min-w-0">
              <p className="text-sm">{row.label}</p>
              <p className="truncate font-mono text-[0.72rem] text-muted-foreground">
                {row.detail}
              </p>
            </div>
          </li>
        ))}
      </ul>
    </Panel>
  );
}
