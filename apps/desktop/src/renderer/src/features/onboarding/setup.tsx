import { type FormEvent, useState } from "react";

import { useNavigate } from "@tanstack/react-router";

import { ArrowRightIcon, CheckIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";

import { NodeLine, useNodeProbe } from "@/components/node-probe";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { short } from "@/lib/format";
import { call } from "@/lib/ipc";
import { EASE_OUT } from "@/lib/motion";
import { useAppState } from "@/lib/state";
import { cn } from "@/lib/utils";

import { OnboardingFrame, enter } from "./frame";

const LOCAL = "http://localhost:8080";

export function Setup() {
  const { data: state } = useAppState();
  const [phase, setPhase] = useState<"form" | "creating">(state?.profile ? "creating" : "form");
  return (
    <OnboardingFrame
      seed={phase === "form" ? 33 : 47}
      step={phase === "form" ? 1 : 2}
      steps={2}
      caption={
        phase === "form"
          ? "Co-signers will see this name next to your approvals."
          : "Your keys are made here, and they stay here."
      }
    >
      <AnimatePresence mode="wait" initial={false}>
        {phase === "form" ? (
          <motion.div key="form" {...enter}>
            <ProfileForm onCreated={() => setPhase("creating")} />
          </motion.div>
        ) : (
          <motion.div key="creating" {...enter}>
            <Creating />
          </motion.div>
        )}
      </AnimatePresence>
    </OnboardingFrame>
  );
}

function ProfileForm({ onCreated }: { onCreated: () => void }) {
  const [name, setName] = useState("");
  const [network, setNetwork] = useState<"local" | "custom">("local");
  const [custom, setCustom] = useState("https://");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const url = network === "local" ? LOCAL : custom.trim();
  const probe = useNodeProbe(url);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setError(null);
    setBusy(true);
    try {
      await call("profile.create", { name: name.trim(), nodeUrl: url });
      onCreated();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };

  return (
    <form onSubmit={(e) => void submit(e)} className="space-y-8">
      <div>
        <h1 className="serif-display text-[2.3rem] leading-[1.05]">
          What should co-signers call you?
        </h1>
        <p className="mt-2.5 text-sm text-muted-foreground">
          Shown to the people you share vaults with. You can change it later.
        </p>
      </div>
      <Input
        autoFocus
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Your name"
        maxLength={48}
        aria-label="Your name"
        data-testid="profile-name"
        className="h-12 rounded-xl px-4 text-base"
      />
      <div className="space-y-3">
        <p className="text-sm font-medium">Aztec network</p>
        <ToggleGroup
          value={[network]}
          onValueChange={(value) => value[0] && setNetwork(value[0] as "local" | "custom")}
          className="w-full rounded-xl bg-paper-2 p-1"
        >
          <ToggleGroupItem
            value="local"
            className="h-9 flex-1 rounded-lg data-[pressed]:bg-card data-[pressed]:shadow-sm"
          >
            Local network
          </ToggleGroupItem>
          <ToggleGroupItem
            value="custom"
            className="h-9 flex-1 rounded-lg data-[pressed]:bg-card data-[pressed]:shadow-sm"
          >
            Another node
          </ToggleGroupItem>
        </ToggleGroup>
        <AnimatePresence initial={false}>
          {network === "custom" && (
            <motion.div
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              transition={{ duration: 0.22, ease: EASE_OUT }}
              className="overflow-hidden"
            >
              <Input
                value={custom}
                onChange={(e) => setCustom(e.target.value)}
                placeholder="https://aztec-node.example.com"
                aria-label="Node URL"
                className="h-10 font-mono text-[0.8rem]"
              />
            </motion.div>
          )}
        </AnimatePresence>
        <NodeLine status={probe.data} loading={probe.isLoading} />
      </div>
      {error && <p className="rounded-lg bg-coral-soft px-3 py-2 text-sm text-coral">{error}</p>}
      <Button
        size="xl"
        type="submit"
        disabled={!name.trim() || busy}
        className="w-full"
        data-testid="profile-create"
      >
        {busy ? <Spinner /> : null}
        {busy ? "Creating your keys…" : "Create my keys"}
      </Button>
    </form>
  );
}

function Creating() {
  const navigate = useNavigate();
  const { data: state } = useAppState();
  const profile = state?.profile;
  const keeper = state?.keeper;
  const items = [
    {
      title: "Signing key",
      done: Boolean(profile),
      detail: profile
        ? `Sealed in your keychain, public key ${short(profile.signerKey.x, 8, 4)}`
        : "Generating",
    },
    {
      title: "Keeper",
      done: keeper?.status === "running",
      failed: keeper?.status === "failed",
      detail:
        keeper?.status === "running"
          ? `Online as ${short(keeper.peerId ?? "", 10, 6)}`
          : (keeper?.error ?? "Starting the process that guards your viewing share"),
    },
    {
      title: "Private engine",
      done: state?.engine.status === "running",
      failed: state?.engine.status === "failed",
      detail:
        state?.engine.status === "running"
          ? "Aztec PXE ready on this device"
          : (state?.engine.error ?? "Starting"),
    },
    {
      title: "Aztec account",
      done: Boolean(profile?.account),
      detail: profile?.account ? short(profile.account, 10, 6) : "Waiting for the network",
      optional: true,
    },
  ];
  const ready = items.filter((i) => !i.optional).every((i) => i.done);
  return (
    <div>
      <h1 className="serif-display text-[2.3rem] leading-[1.05]">
        {ready ? `You're set, ${profile?.name.split(" ")[0] ?? ""}.` : "Setting up this device"}
      </h1>
      <p className="mt-2.5 text-sm text-muted-foreground">
        {ready
          ? "Next, start a vault or join one you were invited to."
          : "This takes a few seconds."}
      </p>
      <ol className="mt-8 space-y-1">
        {items.map((item, i) => (
          <motion.li
            key={item.title}
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: i * 0.06, duration: 0.3, ease: EASE_OUT }}
            className="flex items-center gap-3.5 rounded-xl px-3 py-3"
          >
            <span
              className={cn(
                "grid size-7 shrink-0 place-items-center rounded-full transition-colors duration-300",
                item.done
                  ? "bg-ink text-signal"
                  : item.failed
                    ? "bg-coral-soft text-coral"
                    : "bg-paper-2 text-muted-foreground",
              )}
            >
              <AnimatePresence mode="popLayout" initial={false}>
                {item.done ? (
                  <motion.span
                    key="done"
                    initial={{ scale: 0.5, opacity: 0, filter: "blur(2px)" }}
                    animate={{ scale: 1, opacity: 1, filter: "blur(0px)" }}
                    transition={{ type: "spring", duration: 0.4, bounce: 0.3 }}
                  >
                    <CheckIcon className="size-3.5" strokeWidth={2.5} />
                  </motion.span>
                ) : (
                  <motion.span key="wait" exit={{ opacity: 0, scale: 0.6 }}>
                    <Spinner className="size-3.5" />
                  </motion.span>
                )}
              </AnimatePresence>
            </span>
            <div className="min-w-0">
              <p className="text-sm font-medium">{item.title}</p>
              <p className="truncate text-[0.8rem] text-muted-foreground">{item.detail}</p>
            </div>
          </motion.li>
        ))}
      </ol>
      <Button
        size="xl"
        className="mt-8 w-full"
        disabled={!ready}
        onClick={() => void navigate({ to: "/start" })}
        data-testid="setup-continue"
      >
        Continue
        <ArrowRightIcon data-icon="inline-end" />
      </Button>
    </div>
  );
}
