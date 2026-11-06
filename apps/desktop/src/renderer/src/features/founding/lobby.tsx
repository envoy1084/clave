import { useState } from "react";

import { useQuery } from "@tanstack/react-query";
import { useNavigate, useParams } from "@tanstack/react-router";

import type { LobbyMember, LobbyView } from "@shared/types.ts";
import { ArrowRightIcon, LogOutIcon, QrCodeIcon } from "lucide-react";
import { AnimatePresence, LayoutGroup, motion } from "motion/react";
import { QRCodeSVG } from "qrcode.react";
import { toast } from "sonner";

import { Avatar, Seats, StatusDot } from "@/components/bits";
import { CopyIconSwap, CopyValue, useCopy } from "@/components/copy";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { short } from "@/lib/format";
import { call } from "@/lib/ipc";
import { keys, useAppState } from "@/lib/state";

import { Ceremony } from "./ceremony";
import { expectedMembers } from "./expected";
import { FocusFrame } from "./focus-frame";

export function LobbyScreen() {
  const { lobbyId } = useParams({ from: "/lobby/$lobbyId" });
  const lobby = useQuery({
    queryKey: keys.lobby(lobbyId),
    queryFn: () => call("lobby.get", { lobby: lobbyId }),
    refetchInterval: 4000,
  });
  const founding = useQuery({
    queryKey: keys.founding(lobbyId),
    queryFn: () => call("founding.get", { lobby: lobbyId }),
    refetchInterval: (query) => (query.state.data && !query.state.data.done ? 2000 : false),
  });

  const view = lobby.data;
  return (
    <FocusFrame back="/start" title={view?.name ?? "Lobby"} wide>
      {!view ? (
        <div className="grid h-64 place-items-center text-muted-foreground">
          {lobby.isError ? "Lobby not found in your keeper." : <Spinner />}
        </div>
      ) : founding.data ? (
        <Ceremony lobby={view} founding={founding.data} />
      ) : view.status === "started" && view.role === "guest" ? (
        <Review lobby={view} />
      ) : view.status === "closed" ? (
        <Closed />
      ) : (
        <Room lobby={view} />
      )}
    </FocusFrame>
  );
}

function Header({ lobby, subtitle }: { lobby: LobbyView; subtitle: string }) {
  return (
    <div className="flex items-end justify-between gap-8">
      <div>
        <h1 className="serif-display text-[2.6rem] leading-[1.02]" data-testid="lobby-name">
          {lobby.name}
        </h1>
        <p className="mt-2 text-sm text-muted-foreground">{subtitle}</p>
      </div>
      <div className="flex shrink-0 items-center gap-6 pb-1 text-sm">
        <span className="flex items-center gap-2.5">
          <span className="text-muted-foreground">Spend</span>
          <Seats
            k={lobby.threshold}
            n={Math.max(lobby.members.length, lobby.threshold)}
            tone="olive"
          />
        </span>
        <span className="flex items-center gap-2.5">
          <span className="text-muted-foreground">Read</span>
          <Seats
            k={lobby.viewingThreshold}
            n={Math.max(lobby.members.length, lobby.viewingThreshold)}
            tone="peri"
          />
        </span>
      </div>
    </div>
  );
}

function Room({ lobby }: { lobby: LobbyView }) {
  const navigate = useNavigate();
  const [starting, setStarting] = useState(false);
  const host = lobby.members.find((m) => m.index === 1);
  const expected = expectedMembers(lobby.lobby);
  const count = lobby.members.length;
  const minimum = Math.max(2, lobby.threshold, lobby.viewingThreshold);
  const missing = expected ? Math.max(0, expected - count) : 0;
  const canStart = count >= minimum;

  const start = async () => {
    setStarting(true);
    try {
      await call("lobby.start", {
        lobby: lobby.lobby,
        threshold: lobby.threshold,
        viewingThreshold: lobby.viewingThreshold,
      });
    } catch (e) {
      toast.error("lobby.start failed", {
        description: e instanceof Error ? e.message : String(e),
      });
      setStarting(false);
    }
  };

  const leave = async () => {
    await call("lobby.leave", { lobby: lobby.lobby });
    await navigate({ to: "/start" });
  };

  return (
    <div className="space-y-10">
      <Header
        lobby={lobby}
        subtitle={
          lobby.role === "host"
            ? "Lobby open. Members appear as their keepers connect with the invite secret."
            : `Waiting for ${host?.name ?? "the host"} to start the DKG.`
        }
      />
      <div className="grid grid-cols-[minmax(0,1fr)_340px] gap-8">
        <section>
          <div className="mb-3 flex items-baseline justify-between">
            <h2 className="text-sm font-medium">
              In the room <span className="tabular text-muted-foreground">{count}</span>
            </h2>
            {missing > 0 && (
              <p className="flex items-center gap-2 text-[0.8rem] text-muted-foreground">
                <StatusDot tone="busy" pulse /> Waiting for {missing} more
              </p>
            )}
          </div>
          <LayoutGroup>
            <ul className="space-y-2" data-testid="roster">
              <AnimatePresence initial={false}>
                {lobby.members.map((member) => (
                  <RosterRow key={member.identity} member={member} />
                ))}
                {Array.from({ length: missing }, (_, i) => (
                  <motion.li
                    key={`seat-${i}`}
                    layout
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0, transition: { duration: 0.12 } }}
                    className="flex items-center gap-3.5 rounded-xl border border-dashed border-[#d8d3c6] px-4 py-3.5"
                  >
                    <span className="size-8 rounded-full border border-dashed border-[#cfc9bb]" />
                    <span className="text-sm text-muted-foreground">Open seat</span>
                  </motion.li>
                ))}
              </AnimatePresence>
            </ul>
          </LayoutGroup>
        </section>

        <aside className="space-y-4">
          {lobby.role === "host" && lobby.invite ? (
            <InviteCard invite={lobby.invite} />
          ) : (
            <div className="rounded-2xl bg-card p-5 text-sm shadow-[0_0_0_1px_var(--border)]">
              <p className="flex items-center gap-2 font-medium">
                <StatusDot tone="busy" pulse /> You're in
              </p>
              <p className="mt-1.5 text-muted-foreground">
                When {host?.name ?? "the host"} starts, you'll review the final plan before any keys
                are made.
              </p>
            </div>
          )}

          {lobby.role === "host" ? (
            <div className="space-y-2">
              <Button
                size="xl"
                className="w-full"
                disabled={!canStart || starting}
                onClick={() => void start()}
                data-testid="lobby-start"
              >
                {starting && <Spinner />}
                {missing > 0 && canStart ? `Start DKG with ${count}` : "Start DKG"}
                {!starting && <ArrowRightIcon data-icon="inline-end" />}
              </Button>
              <p className="text-center text-xs text-muted-foreground">
                {canStart
                  ? "Sends the plan (members, k, t_v, salt, sealed protocol secrets) to every member."
                  : `Needs at least ${minimum} members for these rules.`}
              </p>
            </div>
          ) : (
            <Button
              variant="ghost"
              className="w-full text-muted-foreground"
              onClick={() => void leave()}
            >
              <LogOutIcon data-icon="inline-start" /> Leave
            </Button>
          )}
        </aside>
      </div>
    </div>
  );
}

function RosterRow({ member }: { member: LobbyMember }) {
  const { data: state } = useAppState();
  return (
    <motion.li
      layout
      initial={{ opacity: 0, y: 8, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, scale: 0.98, transition: { duration: 0.15 } }}
      transition={{ type: "spring", duration: 0.45, bounce: 0.15 }}
      className="flex items-center gap-3.5 rounded-xl bg-card px-4 py-3 shadow-[0_0_0_1px_var(--border),0_1px_2px_rgb(21_24_15/0.03)]"
    >
      <Avatar name={member.name} identity={member.identity} slot={member.index} />
      <div className="min-w-0 flex-1">
        <p className="flex items-center gap-2 text-sm font-medium">
          {member.name}
          {member.you && <Badge variant="secondary">You</Badge>}
          {member.index === 1 && <Badge variant="outline">Host</Badge>}
        </p>
        {state?.settings.technicalDetails ? (
          <CopyValue
            value={member.peerId}
            head={12}
            tail={6}
            label="peer id"
            className="text-[0.72rem] text-muted-foreground"
          />
        ) : (
          <p className="text-[0.8rem] text-muted-foreground">
            Signing key {short(member.signerKey.x, 6, 4)}
          </p>
        )}
      </div>
      <StatusDot tone="ok" />
    </motion.li>
  );
}

function InviteCard({ invite }: { invite: string }) {
  const { copied, copy } = useCopy(1800);
  const [qr, setQr] = useState(false);
  return (
    <div className="overflow-hidden rounded-2xl bg-ink text-paper shadow-[0_24px_48px_-24px_rgb(21_24_15/0.6)]">
      <div className="p-5">
        <p className="text-xs text-paper/55">Invite code</p>
        <p
          className="mt-2 line-clamp-3 font-mono text-[0.72rem] leading-relaxed break-all text-paper/90"
          data-testid="invite-code"
          data-selectable
        >
          {invite}
        </p>
      </div>
      <div className="flex gap-2 border-t border-paper/10 p-3">
        <Button
          variant="signal"
          className="flex-1"
          onClick={() => void copy(invite)}
          data-testid="invite-copy"
        >
          <CopyIconSwap copied={copied} />
          {copied ? "Copied" : "Copy invite"}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="text-paper hover-fine:bg-paper/10"
          aria-label="Show QR code"
          onClick={() => setQr(true)}
        >
          <QrCodeIcon />
        </Button>
      </div>
      <p className="border-t border-paper/10 px-5 py-3 text-xs leading-relaxed text-paper/55">
        Send it over a channel you trust. Anyone with this code can ask to join until you found the
        vault.
      </p>
      <Dialog open={qr} onOpenChange={setQr}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Invite</DialogTitle>
            <DialogDescription>tecpan-invite-v0 string as a QR code.</DialogDescription>
          </DialogHeader>
          <div className="grid place-items-center rounded-xl bg-paper p-6">
            <QRCodeSVG value={invite} size={232} bgColor="#FBF8F2" fgColor="#15180F" level="M" />
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}

/** A guest's confirmation before its keeper joins the key ceremony: the keeper never joins on a peer's word. */
function Review({ lobby }: { lobby: LobbyView }) {
  const [busy, setBusy] = useState(false);
  const me = lobby.members.find((m) => m.you);
  const host = lobby.members.find((m) => m.index === 1);
  const begin = async () => {
    setBusy(true);
    try {
      await call("founding.begin", { lobby: lobby.lobby });
    } catch (e) {
      toast.error("DKG start failed", { description: e instanceof Error ? e.message : String(e) });
      setBusy(false);
    }
  };
  return (
    <div className="mx-auto max-w-[620px] space-y-8">
      <div>
        <h1 className="serif-display text-[2.6rem] leading-[1.02]">Review the vault</h1>
        <p className="mt-2 text-sm text-muted-foreground">
          {host?.name ?? "The host"} started. Your keeper joins the DKG only after you confirm this
          plan.
        </p>
      </div>
      <div className="overflow-hidden rounded-2xl bg-card shadow-[0_0_0_1px_var(--border)]">
        <dl className="divide-y divide-border text-sm">
          <Fact term="Name" value={lobby.name} />
          <Fact term="Your slot" value={`Member ${me?.index ?? "?"} of ${lobby.members.length}`} />
          <Fact term="k" value={`Any ${lobby.threshold} of ${lobby.members.length} approve`} />
          <Fact
            term="t_v"
            value={`Any ${lobby.viewingThreshold} of ${lobby.members.length} keepers together`}
          />
        </dl>
        <ul className="flex flex-wrap gap-2 border-t border-border bg-paper px-5 py-4">
          {lobby.members.map((m) => (
            <li
              key={m.identity}
              className="flex items-center gap-2 rounded-full bg-card py-1 pr-3 pl-1 text-[0.8rem] shadow-[0_0_0_1px_var(--border)]"
            >
              <Avatar name={m.name} identity={m.identity} slot={m.index} size="sm" />
              {m.name}
            </li>
          ))}
        </ul>
      </div>
      <Button
        size="xl"
        className="w-full"
        onClick={() => void begin()}
        disabled={busy}
        data-testid="review-begin"
      >
        {busy && <Spinner />}
        Make my key share
        {!busy && <ArrowRightIcon data-icon="inline-end" />}
      </Button>
    </div>
  );
}

function Fact({ term, value }: { term: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-6 px-5 py-3.5">
      <dt className="text-muted-foreground">{term}</dt>
      <dd className="text-right font-medium">{value}</dd>
    </div>
  );
}

function Closed() {
  const navigate = useNavigate();
  return (
    <div className="mx-auto grid max-w-md place-items-center gap-4 pt-16 text-center">
      <p className="serif-display text-3xl">This setup was closed</p>
      <p className="text-sm text-muted-foreground">
        The host cancelled it before the vault was founded. Ask them for a new invite.
      </p>
      <Button variant="outline" onClick={() => void navigate({ to: "/start" })}>
        Back to start
      </Button>
    </div>
  );
}
