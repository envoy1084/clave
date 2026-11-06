import { type FormEvent, useState } from "react";

import { useNavigate } from "@tanstack/react-router";

import { AlertTriangleIcon, ArrowRightIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";

import { Seats, Stepper } from "@/components/bits";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { l1Name } from "@/lib/format";
import { call } from "@/lib/ipc";
import { EASE_OUT } from "@/lib/motion";
import { useNodeStatus } from "@/lib/state";

import { rememberExpected } from "./expected";
import { FocusFrame } from "./focus-frame";

export function CreateVault() {
  const navigate = useNavigate();
  const node = useNodeStatus();
  const [name, setName] = useState("");
  const [members, setMembers] = useState(3);
  const [threshold, setThreshold] = useState(2);
  const [viewing, setViewing] = useState(2);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const changeMembers = (n: number) => {
    setMembers(n);
    setThreshold((k) => Math.min(k, n));
    setViewing((t) => Math.min(Math.max(t, 2), n));
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const lobby = await call("lobby.create", {
        name: name.trim(),
        threshold,
        viewingThreshold: viewing,
      });
      rememberExpected(lobby.lobby, members);
      await navigate({ to: "/lobby/$lobbyId", params: { lobbyId: lobby.lobby } });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };

  return (
    <FocusFrame back="/start" title="Open lobby">
      <form onSubmit={(e) => void submit(e)} className="grid grid-cols-[1fr_300px] gap-12">
        <div className="space-y-10">
          <div>
            <h1 className="serif-display text-[2.4rem] leading-[1.05]">Vault parameters</h1>
            <p className="mt-2 text-sm text-muted-foreground">
              Members see these in the invite before joining. Fixed at deployment; change later with
              rotate_membership or set_threshold.
            </p>
          </div>

          <label className="block space-y-2.5">
            <span className="text-sm font-medium">Name</span>
            <Input
              autoFocus
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Treasury"
              maxLength={48}
              data-testid="vault-name"
              className="h-11 rounded-xl px-3.5 text-[0.95rem]"
            />
          </label>

          <Rule
            title="Members (n)"
            body="Signers and viewing-share holders, including you. Up to 8 slots."
            control={
              <Stepper value={members} min={2} max={8} onChange={changeMembers} label="members" />
            }
          />
          <Rule
            title="Spending threshold (k)"
            body={`Any ${threshold} of the ${members} members can approve a payment together.`}
            control={
              <Stepper
                value={threshold}
                min={1}
                max={members}
                onChange={setThreshold}
                label="approvals"
              />
            }
            footer={<Seats k={threshold} n={members} tone="olive" />}
            warning={
              threshold === 1
                ? "k = 1: any single member's signature executes a payload."
                : undefined
            }
          />
          <Rule
            title="Viewing threshold (t_v)"
            body={`Keepers that must answer an ECDH request (address_secret·P) for the PXE to decrypt a note. Minimum 2, so no single keeper can read.`}
            control={
              <Stepper
                value={viewing}
                min={2}
                max={members}
                onChange={setViewing}
                label="readers"
              />
            }
            footer={<Seats k={viewing} n={members} tone="peri" />}
          />

          {error && (
            <p className="rounded-lg bg-coral-soft px-3 py-2 text-sm text-coral">{error}</p>
          )}
        </div>

        <aside className="sticky top-0 self-start">
          <div className="rounded-2xl bg-ink p-6 text-paper shadow-[0_24px_48px_-24px_rgb(21_24_15/0.6)]">
            <p className="text-xs text-paper/55">Lobby</p>
            <AnimatePresence mode="popLayout" initial={false}>
              <motion.p
                key={name.trim() || "placeholder"}
                initial={{ opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -4 }}
                transition={{ duration: 0.18, ease: EASE_OUT }}
                className="mt-1 truncate font-serif text-[1.7rem] leading-tight tracking-[-0.015em]"
              >
                {name.trim() || "Unnamed"}
              </motion.p>
            </AnimatePresence>
            <dl className="mt-6 space-y-4 text-sm">
              <Summary label="k" value={`${threshold} of ${members}`} tone="bg-signal" />
              <Summary label="t_v" value={`${viewing} of ${members}`} tone="bg-peri" />
              <Summary label="L1" value={l1Name(node.data?.l1ChainId)} tone="bg-paper/40" />
            </dl>
            <p className="mt-6 border-t border-paper/10 pt-4 text-xs leading-relaxed text-paper/55">
              The name stays in the lobby and in members' local records. k, t_v and the signer set
              go on chain only as commitments in the account's private config note.
            </p>
          </div>
          <Button
            size="xl"
            type="submit"
            className="mt-4 w-full"
            disabled={!name.trim() || busy}
            data-testid="vault-create"
          >
            {busy ? <Spinner /> : null}
            Open lobby
            {!busy && <ArrowRightIcon data-icon="inline-end" />}
          </Button>
        </aside>
      </form>
    </FocusFrame>
  );
}

function Rule({
  title,
  body,
  control,
  footer,
  warning,
}: {
  title: string;
  body: string;
  control: React.ReactNode;
  footer?: React.ReactNode;
  warning?: string;
}) {
  return (
    <section className="space-y-3">
      <div className="flex items-start justify-between gap-6">
        <div>
          <p className="text-sm font-medium">{title}</p>
          <p className="mt-1 max-w-[46ch] text-[0.825rem] leading-snug text-muted-foreground">
            {body}
          </p>
        </div>
        {control}
      </div>
      {footer}
      <AnimatePresence initial={false}>
        {warning && (
          <motion.p
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.2, ease: EASE_OUT }}
            className="flex items-center gap-2 overflow-hidden text-[0.8rem] text-[#8a5a00]"
          >
            <AlertTriangleIcon className="size-3.5 shrink-0" /> {warning}
          </motion.p>
        )}
      </AnimatePresence>
    </section>
  );
}

function Summary({ label, value, tone }: { label: string; value: string; tone: string }) {
  return (
    <div className="flex items-center justify-between">
      <dt className="flex items-center gap-2 text-paper/60">
        <span className={`size-1.5 rounded-full ${tone}`} />
        {label}
      </dt>
      <dd className="tabular font-medium">{value}</dd>
    </div>
  );
}
