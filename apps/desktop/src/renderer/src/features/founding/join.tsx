import { useEffect, useState } from "react";

import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { ClipboardPasteIcon, ShieldAlertIcon, ShieldCheckIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";

import { Mark } from "@/components/brand/mark";
import { CopyValue } from "@/components/copy";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import { call } from "@/lib/ipc";
import { EASE_OUT } from "@/lib/motion";
import { useAppState } from "@/lib/state";

import { FocusFrame } from "./focus-frame";

const PREFIX = "tecpan-invite-v0:";

export function JoinVault() {
  const navigate = useNavigate();
  const { data: state } = useAppState();
  const [invite, setInvite] = useState("");
  const [joining, setJoining] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const trimmed = invite.trim();
  const looksValid = trimmed.startsWith(PREFIX) && trimmed.length > PREFIX.length + 16;

  const preview = useQuery({
    queryKey: ["lobby.inspect", trimmed],
    queryFn: () => call("lobby.inspect", { invite: trimmed }),
    enabled: looksValid,
    retry: false,
  });

  // Offer the clipboard straight away when it already holds an invite.
  useEffect(() => {
    void call("clipboard.readInvite").then((text) => {
      if (text) setInvite((current) => current || text);
      return undefined;
    });
  }, []);

  const paste = async () => {
    const text = await call("clipboard.readInvite");
    if (text) setInvite(text);
    else setError("Clipboard has no tecpan-invite-v0 string.");
  };

  const join = async () => {
    setJoining(true);
    setError(null);
    try {
      const lobby = await call("lobby.join", { invite: trimmed });
      await navigate({ to: "/lobby/$lobbyId", params: { lobbyId: lobby.lobby } });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setJoining(false);
    }
  };

  const p = preview.data;
  return (
    <FocusFrame back="/start" title="Join lobby">
      <div className="mx-auto max-w-[560px]">
        <h1 className="serif-display text-[2.4rem] leading-[1.05]">Paste invite</h1>
        <p className="mt-2 text-sm text-muted-foreground">
          Decoded locally before any network traffic. Starts with{" "}
          <span className="font-mono text-ink">tecpan-invite</span>.
        </p>

        <div className="relative mt-8">
          <Textarea
            value={invite}
            onChange={(e) => {
              setInvite(e.target.value);
              setError(null);
            }}
            placeholder={`${PREFIX}…`}
            spellCheck={false}
            data-testid="invite-input"
            className="min-h-28 resize-none rounded-xl bg-card px-4 py-3.5 pr-28 font-mono text-[0.8rem] leading-relaxed break-all"
          />
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="absolute top-3 right-3"
            onClick={() => void paste()}
          >
            <ClipboardPasteIcon data-icon="inline-start" /> Paste
          </Button>
        </div>
        {trimmed && !looksValid && (
          <p className="mt-2 text-[0.8rem] text-muted-foreground">
            That doesn't look like a complete invite yet.
          </p>
        )}

        <AnimatePresence mode="popLayout">
          {looksValid && (
            <motion.div
              key={preview.isSuccess ? "preview" : preview.isError ? "error" : "loading"}
              initial={{ opacity: 0, y: 8, scale: 0.99 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.28, ease: EASE_OUT }}
              className="mt-6"
            >
              {preview.isLoading && (
                <div className="flex items-center gap-2 rounded-2xl bg-card px-5 py-6 text-sm text-muted-foreground shadow-[0_0_0_1px_var(--border)]">
                  <Spinner /> Decoding
                </div>
              )}
              {preview.isError && (
                <div className="rounded-2xl bg-coral-soft px-5 py-4 text-sm text-coral">
                  Not a valid invite (bad encoding, length or version).
                </div>
              )}
              {p && (
                <div className="overflow-hidden rounded-2xl bg-card shadow-[0_0_0_1px_var(--border),0_18px_40px_-24px_rgb(21_24_15/0.3)]">
                  <div className="flex items-start gap-4 px-6 pt-6">
                    <span className="grid size-11 shrink-0 place-items-center rounded-xl bg-ink text-signal">
                      <Mark className="size-6" />
                    </span>
                    <div className="min-w-0">
                      <p
                        className="truncate font-serif text-[1.65rem] leading-tight tracking-[-0.015em]"
                        data-testid="invite-name"
                      >
                        {p.name}
                      </p>
                      <p className="text-sm text-muted-foreground">host {p.hostName}</p>
                    </div>
                  </div>
                  <div className="mt-6 grid grid-cols-2 gap-px border-t border-border bg-border">
                    <div className="bg-card px-6 py-4">
                      <p className="text-xs text-muted-foreground">k</p>
                      <p className="mt-1 font-serif text-xl">{p.threshold} signatures</p>
                    </div>
                    <div className="bg-card px-6 py-4">
                      <p className="text-xs text-muted-foreground">t_v</p>
                      <p className="mt-1 font-serif text-xl">{p.viewingThreshold} keepers</p>
                    </div>
                  </div>
                  <div className="flex items-center justify-between gap-4 border-t border-border px-6 py-3.5 text-[0.8rem]">
                    {p.sameNetwork ? (
                      <span className="flex items-center gap-1.5 text-olive-ink">
                        <ShieldCheckIcon className="size-4" /> Network matches ({p.network})
                      </span>
                    ) : (
                      <span className="flex items-center gap-1.5 text-coral">
                        <ShieldAlertIcon className="size-4" /> Network mismatch: invite {p.network}
                      </span>
                    )}
                    {state?.settings.technicalDetails && (
                      <CopyValue value={p.host.peerId} head={10} tail={6} label="host peer id" />
                    )}
                  </div>
                </div>
              )}
            </motion.div>
          )}
        </AnimatePresence>

        {error && (
          <p className="mt-4 rounded-lg bg-coral-soft px-3 py-2 text-sm text-coral">{error}</p>
        )}

        <div className="mt-8 flex items-center justify-between gap-4">
          <p className="text-xs text-muted-foreground">
            Sends your display name, Grumpkin signing public key and keeper identity, transport key
            and addresses to the host keeper.
          </p>
          <Button
            size="xl"
            disabled={!p || !p.sameNetwork || joining}
            onClick={() => void join()}
            data-testid="invite-join"
          >
            {joining && <Spinner />}
            Join lobby
          </Button>
        </div>
      </div>
    </FocusFrame>
  );
}
