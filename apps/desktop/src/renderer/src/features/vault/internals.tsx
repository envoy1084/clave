import type { ReactNode } from "react";

import { useQuery } from "@tanstack/react-query";

import { CheckCircle2Icon, FolderOpenIcon, RefreshCwIcon, XCircleIcon } from "lucide-react";

import { Avatar, Explain, StatusDot } from "@/components/bits";
import { CopyValue } from "@/components/copy";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ago, number } from "@/lib/format";
import { call } from "@/lib/ipc";
import { keys, useAppState, useNodeStatus } from "@/lib/state";

import { PageHeader, Panel } from "./page-header";
import { useVault } from "./use-vault";

export function Internals() {
  return (
    <>
      <PageHeader
        title="Internals"
        description="What this vault is made of, as your device sees it. Every value can be checked independently."
      />
      <Tabs defaultValue="contract" className="gap-6">
        <TabsList className="h-10 rounded-xl bg-paper-2 p-1">
          <TabsTrigger value="contract" className="rounded-lg px-3.5">
            Contract
          </TabsTrigger>
          <TabsTrigger value="committee" className="rounded-lg px-3.5">
            Viewing committee
          </TabsTrigger>
          <TabsTrigger value="keeper" className="rounded-lg px-3.5">
            Keeper
          </TabsTrigger>
          <TabsTrigger value="node" className="rounded-lg px-3.5">
            Node and PXE
          </TabsTrigger>
        </TabsList>
        <TabsContent value="contract">
          <Contract />
        </TabsContent>
        <TabsContent value="committee">
          <Committee />
        </TabsContent>
        <TabsContent value="keeper">
          <Keeper />
        </TabsContent>
        <TabsContent value="node">
          <Node />
        </TabsContent>
      </Tabs>
    </>
  );
}

function Rows({ children }: { children: ReactNode }) {
  return <dl className="divide-y divide-border px-5 pb-2">{children}</dl>;
}

function Row({ label, tip, children }: { label: string; tip?: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[220px_minmax(0,1fr)] items-start gap-6 py-3 text-sm">
      <dt className="pt-0.5 text-muted-foreground">
        {tip ? <Explain tip={tip}>{label}</Explain> : label}
      </dt>
      <dd className="min-w-0">{children}</dd>
    </div>
  );
}

function Loading() {
  return (
    <div className="space-y-3">
      <Skeleton className="h-48 rounded-2xl" />
      <Skeleton className="h-32 rounded-2xl" />
    </div>
  );
}

function Contract() {
  const { details } = useVault();
  const d = details.data;
  if (!d) return <Loading />;
  const { contract, config } = d;
  return (
    <div className="space-y-5">
      <Panel
        title="On-chain config"
        action={
          config ? (
            config.matchesSetup ? (
              <Badge className="bg-olive-soft text-olive-ink">
                <CheckCircle2Icon /> Matches the plan
              </Badge>
            ) : (
              <Badge variant="destructive">
                <XCircleIcon /> Differs from the plan
              </Badge>
            )
          ) : (
            <Badge variant="secondary">Not readable yet</Badge>
          )
        }
      >
        {config ? (
          <Rows>
            <Row
              label="Config version"
              tip="Bumped by every governance change; approvals name the version they were made for, so old ones stop working."
            >
              <span className="tabular">{config.version}</span>
            </Row>
            <Row label="Spending threshold">
              {config.threshold} of {config.signerCount}
            </Row>
            <Row label="Viewing threshold">
              {config.viewingThreshold} of {config.signerCount}
            </Row>
            <Row
              label="Signer set commitment"
              tip="A hash of every member's signing key, stored privately in the vault. Signer identities never appear in public state."
            >
              <CopyValue
                value={config.signerSetCommitment}
                head={14}
                tail={10}
                label="signer set commitment"
              />
            </Row>
            <Row
              label="Committee commitment"
              tip="A hash of the viewing committee: its threshold and every member's verification share."
            >
              <CopyValue
                value={config.committeeCommitment}
                head={14}
                tail={10}
                label="committee commitment"
              />
            </Row>
            <Row label="Read">{ago(config.readAt)}</Row>
          </Rows>
        ) : (
          <p className="px-5 pt-2 pb-5 text-sm text-muted-foreground">
            The engine could not read the vault's config note yet. It needs the network and two
            keepers.
          </p>
        )}
      </Panel>
      <Panel title="Contract instance">
        <Rows>
          <Row label="Address">
            <CopyValue value={contract.address} full label="address" />
          </Row>
          <Row
            label="Preaddress"
            tip="Hash of the instance; the address is the x-coordinate of preaddress·G plus the group viewing key."
          >
            <CopyValue value={contract.preaddress} head={14} tail={10} label="preaddress" />
          </Row>
          <Row label="Partial address">
            <CopyValue
              value={contract.partialAddress}
              head={14}
              tail={10}
              label="partial address"
            />
          </Row>
          <Row
            label="Contract class"
            tip="The vault's code. Every Tecpan vault shares it; no class or instance is published, since the vault has no public functions."
          >
            <CopyValue value={contract.classId} head={14} tail={10} label="class id" />
          </Row>
          <Row label="Initialization hash">
            <CopyValue
              value={contract.initializationHash}
              head={14}
              tail={10}
              label="initialization hash"
            />
          </Row>
          <Row label="Salt">
            <CopyValue value={contract.salt} head={14} tail={10} label="salt" />
          </Row>
          <Row
            label="Deployer"
            tip="Universal deployment: no deployer is part of the address, so it reveals nothing about who created it."
          >
            <span className="text-muted-foreground">Universal (none)</span>
          </Row>
        </Rows>
      </Panel>
      <Panel title="Public keys">
        <Rows>
          {Object.entries(contract.publicKeys).map(([name, value]) => (
            <Row key={name} label={name}>
              <CopyValue value={value} head={14} tail={10} label={name} />
            </Row>
          ))}
        </Rows>
      </Panel>
    </div>
  );
}

function Committee() {
  const { details, summary } = useVault();
  const d = details.data;
  if (!d) return <Loading />;
  const c = d.committee;
  return (
    <div className="space-y-5">
      <Panel
        title="Committee"
        action={
          <span className="flex items-center gap-2 text-[0.8rem] text-muted-foreground">
            <StatusDot tone={c.serving ? "ok" : "warn"} />{" "}
            {c.serving ? "Your share is active" : `Keeper: ${c.keeperStatus}`}
          </span>
        }
      >
        <Rows>
          <Row
            label="Epoch"
            tip="The config version at which this committee became active. Shares and answers are bound to it."
          >
            <span className="tabular">{c.epoch}</span>
          </Row>
          <Row
            label="Threshold"
            tip="How many keepers' answers combine into one decryption. Fewer than this learn nothing."
          >
            {c.viewingThreshold} of {c.members.length}
          </Row>
          <Row
            label="Group viewing key"
            tip="The vault's incoming viewing public key, made in the ceremony. Its secret was never assembled anywhere."
          >
            <div className="space-y-0.5">
              <CopyValue value={c.groupKey.x} head={14} tail={10} label="group key x" />
              <CopyValue value={c.groupKey.y} head={14} tail={10} label="group key y" />
            </div>
          </Row>
          <Row label="Your share">Member {summary?.myIndex}</Row>
          <Row label="Ceremony session">
            <CopyValue value={d.founding.session} head={14} tail={10} label="session" />
          </Row>
          {d.founding.transcript && (
            <Row
              label="Transcript"
              tip="Every member confirmed this same transcript hash before the key was accepted."
            >
              <CopyValue value={d.founding.transcript} head={14} tail={10} label="transcript" />
            </Row>
          )}
        </Rows>
      </Panel>
      <Panel title="Verification shares">
        <ul className="divide-y divide-border px-5 pb-2">
          {c.members.map((m) => (
            <li key={m.identity} className="flex items-center gap-4 py-3 text-sm">
              <Avatar name={m.name} identity={m.identity} slot={m.index} size="sm" />
              <span className="w-28 truncate">{m.name}</span>
              <span className="tabular w-8 text-muted-foreground">Y{m.index}</span>
              <CopyValue
                value={m.verificationShare.x}
                head={18}
                tail={10}
                label="verification share"
              />
            </li>
          ))}
        </ul>
      </Panel>
    </div>
  );
}

function Keeper() {
  const { address, details } = useVault();
  const { data: state } = useAppState();
  const keeper = state?.keeper;
  const audit = useQuery({
    queryKey: keys.audit(address),
    queryFn: () => call("vault.audit", { vault: address, limit: 100 }),
    refetchInterval: 5000,
  });
  return (
    <div className="space-y-5">
      <Panel
        title="Your keeper"
        action={
          <Button variant="ghost" size="sm" onClick={() => void call("keeper.restart")}>
            <RefreshCwIcon data-icon="inline-start" /> Restart
          </Button>
        }
      >
        <Rows>
          <Row label="Status">
            <span className="flex items-center gap-2">
              <StatusDot tone={keeper?.status === "running" ? "ok" : "warn"} /> {keeper?.status}
              {keeper?.version && <span className="text-muted-foreground">v{keeper.version}</span>}
            </span>
          </Row>
          {keeper?.identity && (
            <Row label="Identity">
              <CopyValue value={keeper.identity} head={14} tail={10} label="identity" />
            </Row>
          )}
          {keeper?.peerId && (
            <Row label="Peer id">
              <CopyValue value={keeper.peerId} head={14} tail={10} label="peer id" />
            </Row>
          )}
          {keeper?.transport && (
            <Row label="Transport key">
              <CopyValue value={keeper.transport} head={14} tail={10} label="transport key" />
            </Row>
          )}
          <Row
            label="Addresses"
            tip="What this keeper advertises to other members. Direct addresses come first, relay circuits last."
          >
            <div className="space-y-0.5">
              {(keeper?.addresses ?? []).map((a) => (
                <CopyValue key={a} value={a} full label="address" className="text-[0.75rem]" />
              ))}
            </div>
          </Row>
          <Row label="Vault status">{details.data?.committee.keeperStatus ?? "…"}</Row>
        </Rows>
      </Panel>
      <Panel
        title="Viewing requests"
        action={
          <span className="text-[0.75rem] text-muted-foreground">
            Every request, served or refused
          </span>
        }
      >
        {audit.data && audit.data.length > 0 ? (
          <div className="px-5 pb-3">
            <table className="w-full text-[0.8rem]">
              <thead>
                <tr className="text-left text-muted-foreground">
                  <th className="py-2 font-normal">When</th>
                  <th className="py-2 font-normal">From</th>
                  <th className="py-2 font-normal">Purpose</th>
                  <th className="py-2 text-right font-normal">Points</th>
                  <th className="py-2 text-right font-normal">Outcome</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {audit.data.map((entry, i) => (
                  <tr key={`${entry.at}-${i}`}>
                    <td className="py-2 text-muted-foreground">{ago(entry.at * 1000)}</td>
                    <td className="py-2">
                      {entry.requesterName ?? `${entry.requester.slice(0, 10)}…`}
                    </td>
                    <td className="py-2">
                      <Badge variant="outline">{entry.purpose}</Badge>
                    </td>
                    <td className="tabular py-2 text-right">{number.format(entry.points)}</td>
                    <td className="py-2 text-right">
                      <span
                        className={entry.outcome === "served" ? "text-olive-ink" : "text-coral"}
                      >
                        {entry.outcome}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="px-5 pt-2 pb-5 text-sm text-muted-foreground">
            No requests yet. Engines ask keepers when they decrypt notes or compute tags for this
            vault.
          </p>
        )}
      </Panel>
    </div>
  );
}

function Node() {
  const node = useNodeStatus();
  const pxe = useQuery({
    queryKey: keys.pxe,
    queryFn: () => call("pxe.status"),
    refetchInterval: 4000,
  });
  const n = node.data;
  const lag =
    n?.blockNumber !== undefined && pxe.data?.syncedBlock !== undefined
      ? n.blockNumber - pxe.data.syncedBlock
      : undefined;
  return (
    <div className="space-y-5">
      <Panel title="Aztec node">
        <Rows>
          <Row label="URL">
            <CopyValue value={n?.url ?? ""} full label="node url" />
          </Row>
          <Row label="Status">
            <span className="flex items-center gap-2">
              <StatusDot tone={n?.reachable ? "ok" : "bad"} />{" "}
              {n?.reachable ? "Reachable" : (n?.error ?? "Unreachable")}
            </span>
          </Row>
          {n?.nodeVersion && <Row label="Version">{n.nodeVersion}</Row>}
          {n?.l1ChainId !== undefined && (
            <Row label="L1 chain id">
              <span className="tabular">{n.l1ChainId}</span>
            </Row>
          )}
          {n?.rollupVersion !== undefined && (
            <Row label="Rollup version">
              <span className="tabular font-mono text-[0.8rem]">{n.rollupVersion}</span>
            </Row>
          )}
          {n?.blockNumber !== undefined && (
            <Row label="Latest block">
              <span className="tabular">{number.format(n.blockNumber)}</span>
            </Row>
          )}
        </Rows>
      </Panel>
      <Panel
        title="Private execution environment (PXE)"
        action={
          <Button variant="ghost" size="sm" onClick={() => void call("shell.openDataFolder")}>
            <FolderOpenIcon data-icon="inline-start" /> Data folder
          </Button>
        }
      >
        <Rows>
          <Row
            label="Synced to block"
            tip="The last block your engine has scanned for this vault's notes."
          >
            <span className="tabular">
              {pxe.data?.syncedBlock !== undefined ? number.format(pxe.data.syncedBlock) : "…"}
              {lag !== undefined && (
                <span className="ml-2 text-muted-foreground">
                  {lag <= 1 ? "up to date" : `${lag} behind`}
                </span>
              )}
            </span>
          </Row>
          <Row
            label="Client-side proving"
            tip="Off on a local network, which accepts simulated proofs. On for real networks."
          >
            {pxe.data?.proverEnabled ? "On" : "Off (simulated proofs)"}
          </Row>
          <Row label="Prover backend">{pxe.data?.backend ?? "…"}</Row>
          <Row label="Data directory">
            <CopyValue value={pxe.data?.dataDir ?? ""} full label="data directory" />
          </Row>
        </Rows>
      </Panel>
    </div>
  );
}
