import { type ReactNode, useEffect, useState } from "react";

import type { Params } from "@shared/contract.ts";
import { FolderOpenIcon, PlusIcon, RefreshCwIcon, XIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { toast } from "sonner";

import { NodeLine, useNodeProbe } from "@/components/node-probe";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { PageHeader } from "@/features/vault/page-header";
import { call } from "@/lib/ipc";
import { EASE_OUT } from "@/lib/motion";
import { useAppState } from "@/lib/state";

const RELAY = /^\/(ip4|ip6|dns4|dns6|dns)\/.+\/p2p\/[1-9A-HJ-NP-Za-km-z]+$/u;

async function save(change: Params<"settings.update">) {
  try {
    await call("settings.update", change);
    toast.success("Saved");
  } catch (e) {
    toast.error("Not saved", { description: e instanceof Error ? e.message : String(e) });
  }
}

export function Settings() {
  const { data: state } = useAppState();
  if (!state?.profile) return null;
  const { settings, profile } = state;

  return (
    <>
      <PageHeader title="Settings" description="These apply to this device only." />
      <div className="max-w-[720px] space-y-10">
        <Section title="Profile" description="How co-signers see you in new vaults.">
          <NameField name={profile.name} />
        </Section>

        <Section
          title="Aztec node"
          description="Your engine reads the chain and sends transactions through this node. Changing it restarts the engine."
        >
          <NodeField url={settings.nodeUrl} onSave={(nodeUrl) => void save({ nodeUrl })} />
          <Toggle
            title="Prove transactions on this device"
            body="Required on real networks. A local network accepts simulated proofs, which are much faster."
            checked={settings.proverEnabled}
            onChange={(proverEnabled) => void save({ proverEnabled })}
          />
        </Section>

        <Section
          title="Relays"
          description="Relays forward encrypted traffic between keepers that can't reach each other directly, and hold sealed messages for members who are offline. They never see contents."
        >
          <Relays relays={settings.relays} onSave={(relays) => void save({ relays })} />
        </Section>

        <Section title="Interface">
          <Toggle
            title="Show technical details"
            body="Peer ids, keys and commitments appear inline across the app, not only under Internals."
            checked={settings.technicalDetails}
            onChange={(technicalDetails) => void save({ technicalDetails })}
          />
        </Section>

        <Section title="This device">
          <div className="flex flex-wrap gap-2">
            <Button variant="outline" onClick={() => void call("shell.openDataFolder")}>
              <FolderOpenIcon data-icon="inline-start" /> Open data folder
            </Button>
            <Button
              variant="outline"
              onClick={() =>
                void call("keeper.restart").then(() => toast.success("Keeper restarted"))
              }
            >
              <RefreshCwIcon data-icon="inline-start" /> Restart keeper
            </Button>
          </div>
          <p className="font-mono text-[0.75rem] text-muted-foreground" data-selectable>
            {state.dataDir}
          </p>
        </Section>
      </div>
    </>
  );
}

function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="grid grid-cols-[220px_minmax(0,1fr)] gap-8 border-t border-border pt-8 first:border-t-0 first:pt-0">
      <div>
        <h2 className="text-sm font-medium">{title}</h2>
        {description && (
          <p className="mt-1.5 text-[0.8rem] leading-snug text-muted-foreground">{description}</p>
        )}
      </div>
      <div className="space-y-4">{children}</div>
    </section>
  );
}

function Toggle({
  title,
  body,
  checked,
  onChange,
}: {
  title: string;
  body: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label className="flex items-start justify-between gap-6 rounded-xl bg-card px-4 py-3.5 shadow-[0_0_0_1px_var(--border)]">
      <span>
        <span className="block text-sm">{title}</span>
        <span className="mt-0.5 block text-[0.8rem] leading-snug text-muted-foreground">
          {body}
        </span>
      </span>
      <Switch checked={checked} onCheckedChange={onChange} className="mt-0.5" />
    </label>
  );
}

function NameField({ name }: { name: string }) {
  const [value, setValue] = useState(name);
  useEffect(() => setValue(name), [name]);
  const dirty = value.trim() !== name && value.trim().length > 0;
  return (
    <form
      className="flex gap-2"
      onSubmit={(e) => {
        e.preventDefault();
        void call("profile.rename", { name: value.trim() }).then(() =>
          toast.success("Name updated"),
        );
      }}
    >
      <Input
        value={value}
        onChange={(e) => setValue(e.target.value)}
        maxLength={48}
        className="h-10"
        aria-label="Your name"
      />
      <SaveButton show={dirty} />
    </form>
  );
}

function NodeField({ url, onSave }: { url: string; onSave: (url: string) => void }) {
  const [value, setValue] = useState(url);
  useEffect(() => setValue(url), [url]);
  const probe = useNodeProbe(value.trim());
  const dirty = value.trim() !== url;
  return (
    <div className="space-y-2.5">
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          onSave(value.trim());
        }}
      >
        <Input
          value={value}
          onChange={(e) => setValue(e.target.value)}
          className="h-10 font-mono text-[0.8rem]"
          aria-label="Node URL"
        />
        <SaveButton show={dirty} />
      </form>
      <NodeLine status={probe.data} loading={probe.isLoading} />
    </div>
  );
}

function SaveButton({ show }: { show: boolean }) {
  return (
    <AnimatePresence initial={false}>
      {show && (
        <motion.div
          initial={{ opacity: 0, scale: 0.96, width: 0 }}
          animate={{ opacity: 1, scale: 1, width: "auto" }}
          exit={{ opacity: 0, scale: 0.96, width: 0 }}
          transition={{ duration: 0.18, ease: EASE_OUT }}
          className="overflow-hidden"
        >
          <Button type="submit" className="h-10">
            Save
          </Button>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function Relays({
  relays,
  onSave,
}: {
  relays: readonly string[];
  onSave: (relays: string[]) => void;
}) {
  const [draft, setDraft] = useState("");
  const valid = RELAY.test(draft.trim());
  return (
    <div className="space-y-3">
      <ul className="space-y-2">
        <AnimatePresence initial={false}>
          {relays.map((relay) => (
            <motion.li
              key={relay}
              layout
              initial={{ opacity: 0, y: -4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, transition: { duration: 0.12 } }}
              className="flex items-center gap-3 rounded-xl bg-card py-2 pr-2 pl-4 shadow-[0_0_0_1px_var(--border)]"
            >
              <span className="min-w-0 flex-1 truncate font-mono text-[0.75rem]" data-selectable>
                {relay}
              </span>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Remove relay"
                onClick={() => onSave(relays.filter((r) => r !== relay))}
              >
                <XIcon />
              </Button>
            </motion.li>
          ))}
        </AnimatePresence>
        {relays.length === 0 && (
          <li className="text-[0.8rem] text-muted-foreground">
            No relays. Keepers connect directly.
          </li>
        )}
      </ul>
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (!valid) return;
          onSave([...relays, draft.trim()]);
          setDraft("");
        }}
      >
        <Input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder="/dns4/relay.example.com/udp/4001/quic-v1/p2p/12D3Koo…"
          className="h-10 font-mono text-[0.75rem]"
          aria-label="Relay address"
        />
        <Button type="submit" variant="outline" className="h-10" disabled={!valid}>
          <PlusIcon data-icon="inline-start" /> Add
        </Button>
      </form>
    </div>
  );
}
