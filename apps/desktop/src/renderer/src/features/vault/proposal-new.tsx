import { useState } from "react";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import type { Params } from "@shared/contract.ts";
import type { ProposalPreview } from "@shared/types.ts";
import { PlusIcon, XIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";

import { CopyValue } from "@/components/copy";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { formatUnits, number, parseUnits } from "@/lib/format";
import { call } from "@/lib/ipc";
import { EASE_OUT } from "@/lib/motion";
import { keys } from "@/lib/state";

import { PageHeader, Panel } from "./page-header";
import { useContacts, useTokens, useVault } from "./use-vault";

const EXPIRY = [
  { hours: 24, label: "24 h" },
  { hours: 72, label: "3 d" },
  { hours: 168, label: "7 d" },
  { hours: 720, label: "30 d" },
];
const ADDRESS = /^0x[0-9a-fA-F]{64}$/u;
const field =
  "h-10 w-full rounded-lg border border-input bg-card px-3 text-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/40";

interface CallDraft {
  readonly key: number;
  to: string;
  fn: string;
  args: string[];
}

export function NewProposal() {
  const navigate = useNavigate();
  const client = useQueryClient();
  const { address } = useVault();
  const tokens = useTokens(address);
  const contacts = useContacts();
  const [kind, setKind] = useState<"transfer" | "calls">("transfer");
  const [token, setToken] = useState("");
  const [to, setTo] = useState("");
  const [amount, setAmount] = useState("");
  const [memo, setMemo] = useState("");
  const [hours, setHours] = useState(168);
  const [calls, setCalls] = useState<CallDraft[]>([{ key: 0, to: "", fn: "", args: [] }]);
  const [preview, setPreview] = useState<ProposalPreview | null>(null);
  const [busy, setBusy] = useState<"preview" | "create" | null>(null);
  const [error, setError] = useState<string | null>(null);

  const selected = tokens.data?.find((t) => t.token.address === token) ?? tokens.data?.[0];
  const units = selected ? parseUnits(amount, selected.token.decimals) : undefined;
  const overBalance =
    units !== undefined && selected?.balance !== undefined && units > BigInt(selected.balance);

  const intent = (): Params<"proposals.create">["intent"] | undefined => {
    const note = memo.trim() || undefined;
    if (kind === "transfer") {
      if (!selected || !ADDRESS.test(to.trim()) || units === undefined) return undefined;
      return {
        kind: "transfer",
        token: selected.token.address,
        to: to.trim(),
        amount: units.toString(),
        memo: note,
      };
    }
    if (calls.some((c) => !ADDRESS.test(c.to.trim()) || !c.fn)) return undefined;
    return {
      kind: "calls",
      calls: calls.map((c) => ({ to: c.to.trim(), fn: c.fn, args: c.args })),
      memo: note,
    };
  };
  const ready = intent();

  const run = async (what: "preview" | "create") => {
    if (!ready) return;
    setBusy(what);
    setError(null);
    try {
      if (what === "preview")
        setPreview(
          await call("proposals.preview", { vault: address, intent: ready, expiresInHours: hours }),
        );
      else {
        const created = await call("proposals.create", {
          vault: address,
          intent: ready,
          expiresInHours: hours,
        });
        await client.invalidateQueries({ queryKey: keys.vaultData(address) });
        await navigate({
          to: "/vault/$address/proposals/$id",
          params: { address, id: created.id },
        });
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <>
      <PageHeader
        title="New proposal"
        description="Builds the payload, pins a tx nonce and computes the approval message under the current config version."
      />
      <div className="grid grid-cols-[minmax(0,1fr)_320px] gap-5">
        <Panel>
          <div className="space-y-6 p-5">
            <ToggleGroup
              value={[kind]}
              onValueChange={(v) => {
                if (!v[0]) return;
                setKind(v[0] as "transfer" | "calls");
                setPreview(null);
              }}
              className="rounded-xl bg-paper-2 p-1"
            >
              <ToggleGroupItem
                value="transfer"
                className="h-8 rounded-lg px-3 data-[pressed]:bg-card data-[pressed]:shadow-sm"
              >
                Token transfer
              </ToggleGroupItem>
              <ToggleGroupItem
                value="calls"
                className="h-8 rounded-lg px-3 data-[pressed]:bg-card data-[pressed]:shadow-sm"
              >
                Contract calls
              </ToggleGroupItem>
            </ToggleGroup>

            {kind === "transfer" ? (
              <div className="space-y-5">
                <Field
                  label="Token"
                  hint="Private transfer: Token.transfer(to, amount) from the vault's notes."
                >
                  {tokens.data && tokens.data.length > 0 ? (
                    <select
                      className={field}
                      value={selected?.token.address ?? ""}
                      onChange={(e) => setToken(e.target.value)}
                      data-testid="proposal-token"
                    >
                      {tokens.data.map((t) => (
                        <option key={t.token.address} value={t.token.address}>
                          {t.token.symbol}, balance{" "}
                          {t.balance !== undefined
                            ? formatUnits(t.balance, t.token.decimals)
                            : "unreadable"}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <p className="text-sm text-muted-foreground">
                      No tokens tracked. Add one from the overview.
                    </p>
                  )}
                </Field>
                <Field label="Recipient" hint="Aztec address. Address book labels are suggested.">
                  <Input
                    value={to}
                    onChange={(e) => setTo(e.target.value)}
                    list="contacts"
                    placeholder="0x…"
                    className="h-10 font-mono text-[0.8rem]"
                    data-testid="proposal-to"
                  />
                  <datalist id="contacts">
                    {(contacts.data ?? []).map((c) => (
                      <option key={c.address} value={c.address}>
                        {c.label}
                      </option>
                    ))}
                  </datalist>
                </Field>
                <Field
                  label="Amount"
                  hint={
                    selected
                      ? `${selected.token.decimals} decimals. Sent as base units.`
                      : undefined
                  }
                  error={overBalance ? "More than the vault's private balance" : undefined}
                >
                  <div className="flex gap-2">
                    <Input
                      value={amount}
                      onChange={(e) => setAmount(e.target.value)}
                      inputMode="decimal"
                      placeholder="0.0"
                      className="h-10 font-mono"
                      data-testid="proposal-amount"
                    />
                    {selected?.balance !== undefined && (
                      <Button
                        variant="outline"
                        type="button"
                        className="h-10"
                        onClick={() =>
                          setAmount(
                            formatUnits(
                              selected.balance ?? "0",
                              selected.token.decimals,
                            ).replaceAll(",", ""),
                          )
                        }
                      >
                        Max
                      </Button>
                    )}
                  </div>
                  {units !== undefined && (
                    <p className="mt-1 font-mono text-[0.7rem] text-muted-foreground">
                      {units.toString()} base units
                    </p>
                  )}
                </Field>
              </div>
            ) : (
              <CallsEditor calls={calls} onChange={(next) => (setCalls(next), setPreview(null))} />
            )}

            <div className="grid grid-cols-2 gap-5">
              <Field label="Memo" hint="Stored in the proposal summary, visible to members only.">
                <Input
                  value={memo}
                  onChange={(e) => setMemo(e.target.value)}
                  maxLength={280}
                  className="h-10"
                />
              </Field>
              <Field label="Approval deadline" hint="expires_at, checked against block timestamp.">
                <ToggleGroup
                  value={[String(hours)]}
                  onValueChange={(v) => v[0] && setHours(Number(v[0]))}
                  className="rounded-xl bg-paper-2 p-1"
                >
                  {EXPIRY.map((e) => (
                    <ToggleGroupItem
                      key={e.hours}
                      value={String(e.hours)}
                      className="h-8 flex-1 rounded-lg data-[pressed]:bg-card data-[pressed]:shadow-sm"
                    >
                      {e.label}
                    </ToggleGroupItem>
                  ))}
                </ToggleGroup>
              </Field>
            </div>
          </div>
        </Panel>

        <aside className="space-y-3">
          <Panel title="Simulation">
            <div className="px-5 pt-2 pb-5">
              <AnimatePresence mode="popLayout" initial={false}>
                <motion.div
                  key={preview ? (preview.ok ? "ok" : "err") : "none"}
                  initial={{ opacity: 0, y: 4 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.18, ease: EASE_OUT }}
                >
                  {!preview ? (
                    <p className="text-sm text-muted-foreground">
                      Runs the payload with the vault's class overridden by a stub account, so no
                      approvals are needed.
                    </p>
                  ) : (
                    <div className="space-y-3 text-sm">
                      <p className={preview.ok ? "text-olive-ink" : "text-coral"}>
                        {preview.ok ? "Simulation succeeded" : `Reverted: ${preview.error}`}
                      </p>
                      {preview.gas && (
                        <p className="font-mono text-[0.72rem] text-muted-foreground">
                          gas l2 {number.format(preview.gas.l2)}, da {number.format(preview.gas.da)}
                        </p>
                      )}
                      <ol className="space-y-1.5">
                        {preview.calls.map((c, i) => (
                          <li key={`${c.to}-${i}`} className="font-mono text-[0.72rem]">
                            <span className="text-foreground">{c.fn}</span>
                            <span className="text-muted-foreground">
                              {" "}
                              on {c.toLabel ?? `${c.to.slice(0, 10)}…`}
                            </span>
                          </li>
                        ))}
                      </ol>
                    </div>
                  )}
                </motion.div>
              </AnimatePresence>
            </div>
          </Panel>
          {error && (
            <p className="rounded-lg bg-coral-soft px-3 py-2 text-sm text-coral">{error}</p>
          )}
          <Button
            variant="outline"
            className="w-full"
            disabled={!ready || busy !== null}
            onClick={() => void run("preview")}
          >
            {busy === "preview" && <Spinner />}
            Simulate
          </Button>
          <Button
            size="lg"
            className="w-full"
            disabled={!ready || busy !== null || overBalance}
            onClick={() => void run("create")}
            data-testid="proposal-create"
          >
            {busy === "create" && <Spinner />}
            Create proposal
          </Button>
          <p className="text-xs leading-snug text-muted-foreground">
            You still approve it separately. Creating stores it in your keeper, which replicates it
            to the committee.
          </p>
        </aside>
      </div>
    </>
  );
}

function Field({
  label,
  hint,
  error,
  children,
}: {
  label: string;
  hint?: string;
  error?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="block space-y-1.5">
      <span className="text-sm font-medium">{label}</span>
      {children}
      {error ? (
        <span className="block text-xs text-coral">{error}</span>
      ) : (
        hint && <span className="block text-xs text-muted-foreground">{hint}</span>
      )}
    </label>
  );
}

function CallsEditor({
  calls,
  onChange,
}: {
  calls: CallDraft[];
  onChange: (calls: CallDraft[]) => void;
}) {
  const update = (key: number, change: Partial<CallDraft>) =>
    onChange(calls.map((c) => (c.key === key ? { ...c, ...change } : c)));
  return (
    <div className="space-y-3">
      {calls.map((c, i) => (
        <CallRow
          key={c.key}
          index={i}
          draft={c}
          onChange={(change) => update(c.key, change)}
          onRemove={
            calls.length > 1 ? () => onChange(calls.filter((x) => x.key !== c.key)) : undefined
          }
        />
      ))}
      {calls.length < 8 && (
        <Button
          variant="ghost"
          size="sm"
          type="button"
          onClick={() =>
            onChange([
              ...calls,
              { key: Math.max(...calls.map((c) => c.key)) + 1, to: "", fn: "", args: [] },
            ])
          }
        >
          <PlusIcon data-icon="inline-start" /> Add call
        </Button>
      )}
      <p className="text-xs text-muted-foreground">
        Calls run in order inside one transaction from the vault. The contract's artifact must be
        registered in your PXE.
      </p>
    </div>
  );
}

function CallRow({
  index,
  draft,
  onChange,
  onRemove,
}: {
  index: number;
  draft: CallDraft;
  onChange: (change: Partial<CallDraft>) => void;
  onRemove?: () => void;
}) {
  const valid = ADDRESS.test(draft.to.trim());
  const functions = useQuery({
    queryKey: ["contracts.functions", draft.to.trim().toLowerCase()],
    queryFn: () => call("contracts.functions", { contract: draft.to.trim() }),
    enabled: valid,
    retry: false,
  });
  const fn = functions.data?.find((f) => f.name === draft.fn);
  return (
    <div className="space-y-2.5 rounded-xl border border-border p-3.5">
      <div className="flex items-center justify-between">
        <span className="font-mono text-xs text-muted-foreground">call {index}</span>
        {onRemove && (
          <Button
            variant="ghost"
            size="icon-xs"
            type="button"
            aria-label="Remove call"
            onClick={onRemove}
          >
            <XIcon />
          </Button>
        )}
      </div>
      <Input
        value={draft.to}
        onChange={(e) => onChange({ to: e.target.value, fn: "", args: [] })}
        placeholder="contract address 0x…"
        className="h-9 font-mono text-[0.78rem]"
        aria-label="Contract address"
      />
      {valid &&
        (functions.isError ? (
          <p className="text-xs text-coral">{String(functions.error)}</p>
        ) : (
          <select
            className={field}
            value={draft.fn}
            aria-label="Function"
            onChange={(e) => {
              const next = functions.data?.find((f) => f.name === e.target.value);
              onChange({ fn: e.target.value, args: next ? next.params.map(() => "") : [] });
            }}
          >
            <option value="">{functions.isLoading ? "Loading ABI" : "Function"}</option>
            {(functions.data ?? [])
              .filter((f) => f.kind !== "utility")
              .map((f) => (
                <option key={f.name} value={f.name}>
                  {f.kind} {f.name}({f.params.map((p) => p.type).join(", ")})
                </option>
              ))}
          </select>
        ))}
      {fn?.params.map((p, i) => (
        <Input
          key={p.name}
          value={draft.args[i] ?? ""}
          onChange={(e) =>
            onChange({ args: draft.args.map((a, j) => (j === i ? e.target.value : a)) })
          }
          placeholder={`${p.name}: ${p.type}`}
          className="h-9 font-mono text-[0.78rem]"
          aria-label={p.name}
        />
      ))}
      {fn && fn.params.length === 0 && <CopyValue value={`${fn.name}()`} label="function" />}
    </div>
  );
}
