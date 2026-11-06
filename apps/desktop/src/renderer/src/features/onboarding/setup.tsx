import { type FormEvent, useState } from "react";

import { useNavigate } from "@tanstack/react-router";

import { CheckIcon, XIcon } from "lucide-react";
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

const LOCAL = "http://localhost:8080";

export function ProfileForm({ onCreated }: { onCreated: () => void }) {
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
    <form onSubmit={(e) => void submit(e)} className="space-y-7">
      <h1 className="serif-display text-[2.4rem] leading-[1.05]">Create profile</h1>
      <label className="block space-y-2">
        <span className="text-sm font-medium">Display name</span>
        <Input
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          maxLength={48}
          data-testid="profile-name"
          className="h-11 rounded-xl px-3.5 text-[0.95rem]"
        />
        <span className="block text-xs text-muted-foreground">
          Shown to co-signers in lobbies and next to your approvals. Never written on chain.
        </span>
      </label>
      <div className="space-y-2.5">
        <p className="text-sm font-medium">Aztec node</p>
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
            Node URL
          </ToggleGroupItem>
        </ToggleGroup>
        {network === "local" ? (
          <p className="font-mono text-xs text-muted-foreground">{LOCAL}</p>
        ) : (
          <Input
            value={custom}
            onChange={(e) => setCustom(e.target.value)}
            aria-label="Node URL"
            className="h-10 font-mono text-[0.8rem]"
          />
        )}
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
        {busy && <Spinner />}
        {busy ? "Generating keys" : "Generate keys"}
      </Button>
    </form>
  );
}

export function Initializing() {
  const navigate = useNavigate();
  const { data: state } = useAppState();
  const profile = state?.profile;
  const keeper = state?.keeper;
  const engine = state?.engine;
  const items: { title: string; done: boolean; failed?: boolean; detail: string }[] = [
    {
      title: "Signing key",
      done: Boolean(profile),
      detail: profile
        ? `Grumpkin, sealed in keychain. pk.x ${short(profile.signerKey.x, 8, 6)}`
        : "Generating",
    },
    {
      title: "Keeper",
      done: keeper?.status === "running",
      failed: keeper?.status === "failed",
      detail:
        keeper?.status === "running"
          ? (keeper.peerId ?? "")
          : (keeper?.error ?? "Starting tecpan-keeper"),
    },
    {
      title: "PXE",
      done: engine?.status === "running",
      failed: engine?.status === "failed",
      detail:
        engine?.status === "running"
          ? "Engine process up, threshold key store loaded"
          : (engine?.error ?? "Starting"),
    },
    {
      title: "Account",
      done: Boolean(profile?.account),
      detail: profile?.account
        ? `Schnorr (initializerless) ${short(profile.account, 10, 6)}`
        : "Waiting for the node",
    },
  ];
  const ready = items.slice(0, 3).every((i) => i.done);
  return (
    <div>
      <h1 className="serif-display text-[2.4rem] leading-[1.05]">
        {ready ? "Profile ready" : "Initializing"}
      </h1>
      <ol className="mt-7 divide-y divide-border border-y border-border">
        {items.map((item) => (
          <li key={item.title} className="flex items-center gap-3.5 py-3.5">
            <span
              className={cn(
                "grid size-6 shrink-0 place-items-center rounded-full transition-colors duration-300",
                item.done
                  ? "bg-ink text-signal"
                  : item.failed
                    ? "bg-coral text-white"
                    : "bg-paper-2 text-muted-foreground",
              )}
            >
              <AnimatePresence mode="popLayout" initial={false}>
                <motion.span
                  key={item.done ? "done" : item.failed ? "failed" : "wait"}
                  initial={{ scale: 0.5, opacity: 0 }}
                  animate={{ scale: 1, opacity: 1 }}
                  exit={{ scale: 0.5, opacity: 0 }}
                  transition={{ duration: 0.2, ease: EASE_OUT }}
                >
                  {item.done ? (
                    <CheckIcon className="size-3.5" strokeWidth={2.5} />
                  ) : item.failed ? (
                    <XIcon className="size-3.5" strokeWidth={2.5} />
                  ) : (
                    <Spinner className="size-3" />
                  )}
                </motion.span>
              </AnimatePresence>
            </span>
            <span className="w-20 shrink-0 text-sm font-medium">{item.title}</span>
            <span className="min-w-0 truncate font-mono text-[0.75rem] text-muted-foreground">
              {item.detail}
            </span>
          </li>
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
      </Button>
    </div>
  );
}
