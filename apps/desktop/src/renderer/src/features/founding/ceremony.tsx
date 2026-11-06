import { useEffect, useState } from "react";

import { useNavigate } from "@tanstack/react-router";

import type { FoundingState, FoundingStep, FoundingStepId, LobbyView } from "@shared/types.ts";
import { ArrowRightIcon, CheckIcon, RotateCcwIcon, XIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";

import { AssemblingMark } from "@/components/brand/mark";
import { Terrain } from "@/components/brand/terrain";
import { CopyValue } from "@/components/copy";
import { Button } from "@/components/ui/button";
import { call } from "@/lib/ipc";
import { EASE_OUT, EASE_IN_OUT } from "@/lib/motion";
import { cn } from "@/lib/utils";

const COPY: Record<FoundingStepId, { title: string; body: string }> = {
  agree: { title: "Agree on the plan", body: "Members, rules and the vault's shared settings." },
  dkg: {
    title: "Make the viewing key together",
    body: "Each keeper makes a share. No one, you included, ever holds the whole key.",
  },
  derive: {
    title: "Work out the vault address",
    body: "Every member computes it independently and gets the same one.",
  },
  register: {
    title: "Register on this device",
    body: "Your keeper and private engine learn about the vault.",
  },
  deploy: {
    title: "Deploy on Aztec",
    body: "The host sends one private transaction that creates the vault.",
  },
  verify: {
    title: "Check the rules on chain",
    body: "Your device reads the vault back and compares it to the plan.",
  },
};

const piecesFor = (state: FoundingState): 0 | 1 | 2 | 3 => {
  const done = (id: FoundingStepId) => state.steps.find((s) => s.id === id)?.status === "done";
  if (done("verify")) return 3;
  if (done("register")) return 2;
  if (done("dkg")) return 1;
  return 0;
};

export function Ceremony({ lobby, founding }: { lobby: LobbyView; founding: FoundingState }) {
  const navigate = useNavigate();
  const pieces = piecesFor(founding);
  const retry = () => void call("founding.begin", { lobby: lobby.lobby });
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_360px] gap-12">
      <div>
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={founding.done ? "done" : founding.error ? "error" : "running"}
            initial={{ opacity: 0, y: 6, filter: "blur(2px)" }}
            animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
            exit={{ opacity: 0, y: -4, filter: "blur(2px)", transition: { duration: 0.12 } }}
            transition={{ duration: 0.3, ease: EASE_OUT }}
          >
            <h1 className="serif-display text-[2.6rem] leading-[1.02]" data-testid="ceremony-title">
              {founding.done
                ? `${lobby.name} is ready`
                : founding.error
                  ? "Founding stopped"
                  : `Founding ${lobby.name}`}
            </h1>
            <p className="mt-2 text-sm text-muted-foreground">
              {founding.done
                ? "Every member's device has checked the vault on chain."
                : founding.error
                  ? "Nothing was spent and no keys left anyone's device."
                  : "Keep Tecpan open. This usually takes under a minute."}
            </p>
          </motion.div>
        </AnimatePresence>

        <ol className="relative mt-10" data-testid="ceremony-steps">
          {founding.steps.map((step, i) => (
            <StepRow key={step.id} step={step} last={i === founding.steps.length - 1} />
          ))}
        </ol>

        <AnimatePresence>
          {founding.error && (
            <motion.div
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              className="mt-6 rounded-2xl bg-coral-soft p-5"
            >
              <p className="text-sm font-medium text-coral">{founding.error.message}</p>
              {founding.error.blamed && founding.error.blamed.length > 0 && (
                <p className="mt-1 text-sm text-coral/80">
                  Involved: {founding.error.blamed.join(", ")}
                </p>
              )}
              <div className="mt-4 flex gap-2">
                <Button variant="outline" onClick={retry}>
                  <RotateCcwIcon data-icon="inline-start" /> Try again
                </Button>
                {founding.error.step === "dkg" && (
                  <p className="self-center text-xs text-coral/80">
                    If it stops again, the host can start a new invite.
                  </p>
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      <aside className="sticky top-0 self-start">
        <div className="relative overflow-hidden rounded-3xl bg-[#0E1530] text-white shadow-[0_32px_64px_-32px_rgb(14_21_48/0.7)]">
          <Terrain
            seed={64}
            shade="bottom"
            clusters={[[60, 40, 6, 5]]}
            className="absolute inset-0 opacity-90"
          />
          <div className="relative grid h-[340px] place-items-center">
            <AssemblingMark
              pieces={pieces}
              className={cn("size-36", founding.done ? "text-signal" : "text-white")}
            />
          </div>
          <div className="relative border-t border-white/10 bg-[#0E1530]/70 px-6 py-5 backdrop-blur-sm">
            <AnimatePresence mode="wait" initial={false}>
              {founding.vault ? (
                <motion.div key="addr" initial={{ opacity: 0 }} animate={{ opacity: 1 }}>
                  <p className="text-xs text-white/55">Vault address</p>
                  <CopyValue
                    value={founding.vault}
                    head={14}
                    tail={8}
                    label="vault address"
                    className="mt-1 text-white/90 hover-fine:bg-white/10 hover-fine:text-white"
                  />
                </motion.div>
              ) : (
                <motion.p
                  key="wait"
                  className="text-sm text-white/60"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                >
                  The address appears once the viewing key exists.
                </motion.p>
              )}
            </AnimatePresence>
          </div>
        </div>
        <AnimatePresence>
          {founding.done && founding.vault && (
            <motion.div
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.25, duration: 0.35, ease: EASE_OUT }}
            >
              <Button
                size="xl"
                className="mt-4 w-full"
                onClick={() =>
                  founding.vault &&
                  void navigate({ to: "/vault/$address", params: { address: founding.vault } })
                }
                data-testid="ceremony-open"
              >
                Open {lobby.name}
                <ArrowRightIcon data-icon="inline-end" />
              </Button>
            </motion.div>
          )}
        </AnimatePresence>
      </aside>
    </div>
  );
}

function StepRow({ step, last }: { step: FoundingStep; last: boolean }) {
  const copy = COPY[step.id];
  const active = step.status === "active";
  return (
    <li
      className="relative flex gap-4 pb-7 last:pb-0"
      data-status={step.status}
      data-step={step.id}
    >
      {!last && (
        <span className="absolute top-8 bottom-1 left-[13px] w-px bg-[#e1dccf]" aria-hidden>
          <motion.span
            className="absolute inset-0 origin-top bg-ink"
            initial={false}
            animate={{ scaleY: step.status === "done" ? 1 : 0 }}
            transition={{ duration: 0.45, ease: EASE_IN_OUT }}
          />
        </span>
      )}
      <StepIcon status={step.status} />
      <div className="min-w-0 flex-1 pt-0.5">
        <div className="flex items-baseline justify-between gap-4">
          <p
            className={cn(
              "text-sm font-medium transition-colors duration-200",
              step.status === "pending" && "text-muted-foreground",
            )}
          >
            {copy.title}
          </p>
          <Timer step={step} />
        </div>
        <p className="mt-0.5 text-[0.8rem] leading-snug text-muted-foreground">
          {active || step.status === "done" ? (step.detail ?? copy.body) : copy.body}
        </p>
      </div>
    </li>
  );
}

function StepIcon({ status }: { status: FoundingStep["status"] }) {
  return (
    <span
      className={cn(
        "relative z-10 grid size-7 shrink-0 place-items-center rounded-full transition-[background-color,box-shadow] duration-300",
        status === "done" && "bg-ink text-signal",
        status === "active" && "bg-card shadow-[0_0_0_1px_var(--ink)]",
        status === "pending" && "bg-paper shadow-[0_0_0_1px_#d9d4c7]",
        status === "failed" && "bg-coral text-white",
      )}
    >
      <AnimatePresence mode="popLayout" initial={false}>
        {status === "done" ? (
          <motion.span
            key="d"
            initial={{ scale: 0.4, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            transition={{ type: "spring", duration: 0.4, bounce: 0.35 }}
          >
            <CheckIcon className="size-3.5" strokeWidth={2.75} />
          </motion.span>
        ) : status === "failed" ? (
          <motion.span
            key="f"
            initial={{ scale: 0.4, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
          >
            <XIcon className="size-3.5" strokeWidth={2.75} />
          </motion.span>
        ) : status === "active" ? (
          <motion.span
            key="a"
            className="size-3.5 rounded-full border-2 border-ink/15 border-t-ink"
            animate={{ rotate: 360 }}
            transition={{ repeat: Number.POSITIVE_INFINITY, duration: 0.7, ease: "linear" }}
          />
        ) : (
          <motion.span key="p" className="size-1.5 rounded-full bg-[#cdc7b8]" />
        )}
      </AnimatePresence>
    </span>
  );
}

/** Elapsed time while a step runs, its duration once done. */
function Timer({ step }: { step: FoundingStep }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (step.status !== "active") return;
    const id = window.setInterval(() => setNow(Date.now()), 100);
    return () => window.clearInterval(id);
  }, [step.status]);
  if (!step.startedAt || step.status === "pending") return null;
  const ms = (step.finishedAt ?? now) - step.startedAt;
  return (
    <span className="tabular shrink-0 font-mono text-[0.72rem] text-muted-foreground">
      {ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`}
    </span>
  );
}
