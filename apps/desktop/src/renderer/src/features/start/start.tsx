import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";

import { ArrowRightIcon, Settings2Icon, TicketIcon } from "lucide-react";
import { motion } from "motion/react";

import { Avatar } from "@/components/bits";
import { Mark, Wordmark } from "@/components/brand/mark";
import { Terrain } from "@/components/brand/terrain";
import { Button } from "@/components/ui/button";
import { ofN } from "@/lib/format";
import { call } from "@/lib/ipc";
import { EASE_OUT } from "@/lib/motion";
import { keys, useAppState } from "@/lib/state";

/** Home before any vault exists: start one or join one, and resume any setup in progress. */
export function Start() {
  const navigate = useNavigate();
  const { data: state } = useAppState();
  const lobbies = useQuery({
    queryKey: keys.lobbies,
    queryFn: () => call("lobby.list"),
    refetchInterval: 5000,
  });
  const pending = (lobbies.data ?? []).filter((l) => l.status !== "closed");
  const profile = state?.profile;
  return (
    <div className="flex h-screen flex-col overflow-hidden bg-paper">
      <header className="app-drag flex h-14 shrink-0 items-center justify-between pr-4 pl-24">
        <Wordmark className="text-[0.95rem]" />
        <div className="app-no-drag flex items-center gap-2">
          {profile && (
            <span className="flex items-center gap-2 rounded-full py-1 pr-3 pl-1 text-sm text-muted-foreground">
              <Avatar
                name={profile.name}
                identity={state.keeper.identity ?? profile.name}
                size="sm"
              />
              {profile.name}
            </span>
          )}
          <Button
            variant="ghost"
            size="icon"
            aria-label="Settings"
            onClick={() => void navigate({ to: "/settings" })}
          >
            <Settings2Icon />
          </Button>
        </div>
      </header>
      <main className="quiet-scroll flex-1 overflow-y-auto px-10 pb-16">
        <div className="mx-auto max-w-[920px] pt-[8vh]">
          <motion.h1
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.4, ease: EASE_OUT }}
            className="serif-display text-[2.6rem] leading-[1.05]"
          >
            Vaults
          </motion.h1>
          <p className="mt-2 text-muted-foreground">
            A vault is founded by all its members together: the host opens a lobby, members join
            with its invite, then everyone runs the DKG.
          </p>

          <div className="mt-10 grid grid-cols-2 gap-5">
            <Choice
              index={0}
              to="/new"
              title="Open a lobby"
              body="Set k and t_v, share the invite, start the DKG when members have joined. You deploy the account contract."
              art={
                <div className="relative h-full overflow-hidden bg-[#0E1530]">
                  <Terrain
                    seed={58}
                    shade="none"
                    clusters={[[300, 120, 8, 6]]}
                    className="absolute inset-0"
                  />
                  <Mark className="absolute bottom-5 left-6 size-11 text-white" />
                </div>
              }
              testId="start-new"
            />
            <Choice
              index={1}
              to="/join"
              title="Join a lobby"
              body="Paste a tecpan-invite-v0 string. It is decoded locally first: host, k, t_v and network."
              art={
                <div className="relative grid h-full place-items-center overflow-hidden bg-paper-2">
                  <div className="flex items-center gap-3 rounded-xl bg-card px-4 py-3 shadow-[0_0_0_1px_var(--border),0_12px_32px_-12px_rgb(21_24_15/0.25)]">
                    <TicketIcon className="size-4 text-olive" />
                    <span className="font-mono text-xs text-muted-foreground">
                      tecpan-invite-v0:AhCq…9fRk
                    </span>
                  </div>
                </div>
              }
              testId="start-join"
            />
          </div>

          {pending.length > 0 && (
            <section className="mt-14">
              <h2 className="text-sm font-medium text-muted-foreground">Lobbies</h2>
              <ul className="mt-3 divide-y divide-border overflow-hidden rounded-xl bg-card shadow-[0_0_0_1px_var(--border)]">
                {pending.map((lobby) => (
                  <li key={lobby.lobby}>
                    <Link
                      to="/lobby/$lobbyId"
                      params={{ lobbyId: lobby.lobby }}
                      className="group flex items-center gap-4 px-5 py-4 transition-colors duration-150 hover-fine:bg-paper"
                    >
                      <Mark className="size-5 text-olive" />
                      <div className="min-w-0 flex-1">
                        <p className="font-medium">{lobby.name}</p>
                        <p className="text-[0.8rem] text-muted-foreground">
                          {lobby.role === "host" ? "You're hosting" : "You joined"},{" "}
                          {lobby.members.length} in the room,{" "}
                          {ofN(lobby.threshold, lobby.members.length)} to spend
                        </p>
                      </div>
                      <span className="text-sm text-muted-foreground transition-transform duration-200 ease-out group-hover:translate-x-0.5">
                        <ArrowRightIcon className="size-4" />
                      </span>
                    </Link>
                  </li>
                ))}
              </ul>
            </section>
          )}
        </div>
      </main>
    </div>
  );
}

function Choice({
  index,
  to,
  title,
  body,
  art,
  testId,
}: {
  index: number;
  to: "/new" | "/join";
  title: string;
  body: string;
  art: React.ReactNode;
  testId: string;
}) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: 0.08 + index * 0.06, duration: 0.45, ease: EASE_OUT }}
    >
      <Link
        to={to}
        data-testid={testId}
        className="group block overflow-hidden rounded-2xl bg-card shadow-[0_0_0_1px_var(--border),0_1px_2px_rgb(21_24_15/0.04)] transition-[transform,box-shadow] duration-200 ease-out hover-fine:shadow-[0_0_0_1px_rgb(21_24_15/0.16),0_18px_40px_-20px_rgb(21_24_15/0.3)] active:scale-[0.99]"
      >
        <div className="h-40">{art}</div>
        <div className="flex items-end justify-between gap-6 p-6">
          <div>
            <p className="font-serif text-2xl leading-tight tracking-[-0.015em]">{title}</p>
            <p className="mt-1.5 max-w-[36ch] text-sm leading-snug text-muted-foreground">{body}</p>
          </div>
          <span className="grid size-9 shrink-0 place-items-center rounded-full bg-paper-2 text-ink transition-[transform,background-color] duration-200 ease-out group-hover:translate-x-0.5 group-hover:bg-signal">
            <ArrowRightIcon className="size-4" />
          </span>
        </div>
      </Link>
    </motion.div>
  );
}
