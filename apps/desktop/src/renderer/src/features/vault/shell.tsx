import type { ReactNode } from "react";

import { Link, Outlet, useLocation, useNavigate, useParams } from "@tanstack/react-router";

import type { VaultSummary } from "@shared/types.ts";
import {
  ChevronsUpDownIcon,
  CpuIcon,
  LayoutGridIcon,
  PlusIcon,
  Settings2Icon,
  TicketIcon,
  UsersRoundIcon,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";

import { Avatar, StatusDot } from "@/components/bits";
import { Mark, Wordmark } from "@/components/brand/mark";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { number, ofN } from "@/lib/format";
import { EASE_OUT } from "@/lib/motion";
import { useAppState, useNodeStatus } from "@/lib/state";
import { cn } from "@/lib/utils";

/** The workspace: an ink sidebar (vaults, sections, live status) beside the paper canvas. */
export function VaultShell() {
  const params = useParams({ strict: false }) as { address?: string };
  const location = useLocation();
  const { data: state } = useAppState();
  const vaults = state?.vaults ?? [];
  const current = vaults.find((v) => v.address.toLowerCase() === params.address?.toLowerCase());
  return (
    <div className="grid h-screen grid-cols-[248px_minmax(0,1fr)] overflow-hidden bg-paper">
      <aside className="flex min-h-0 flex-col bg-sidebar text-sidebar-foreground">
        <div className="app-drag h-12 shrink-0" />
        <div className="px-4 pb-3">
          <Wordmark className="px-1.5 text-[0.95rem] text-white" />
        </div>
        <div className="px-3">
          <VaultSwitcher vaults={vaults} current={current} />
        </div>
        <nav className="mt-5 space-y-0.5 px-3" aria-label="Vault">
          {current ? (
            <>
              <NavItem
                to="/vault/$address"
                address={current.address}
                icon={<LayoutGridIcon />}
                exact
              >
                Overview
              </NavItem>
              <NavItem
                to="/vault/$address/members"
                address={current.address}
                icon={<UsersRoundIcon />}
              >
                Members
              </NavItem>
              <NavItem to="/vault/$address/internals" address={current.address} icon={<CpuIcon />}>
                Internals
              </NavItem>
            </>
          ) : null}
        </nav>
        <div className="mt-auto space-y-0.5 px-3 pb-3">
          <NavItem to="/settings" icon={<Settings2Icon />}>
            Settings
          </NavItem>
          <SidebarStatus />
        </div>
      </aside>
      <main className="quiet-scroll relative min-w-0 overflow-y-auto">
        <div className="app-drag sticky top-0 z-10 h-12 bg-gradient-to-b from-paper via-paper/90 to-paper/0" />
        {/* Section switches are frequent: a quick fade only, no movement and no waiting for the old page to leave. */}
        <motion.div
          key={location.pathname}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 0.12, ease: EASE_OUT }}
          className="mx-auto max-w-[1080px] px-10 pb-20"
        >
          <Outlet />
        </motion.div>
      </main>
    </div>
  );
}

function NavItem({
  to,
  address,
  icon,
  children,
  exact = false,
}: {
  to: "/vault/$address" | "/vault/$address/members" | "/vault/$address/internals" | "/settings";
  address?: string;
  icon: ReactNode;
  children: ReactNode;
  exact?: boolean;
}) {
  return (
    <Link
      to={to}
      params={address ? { address } : undefined}
      activeOptions={{ exact }}
      className="group relative flex h-9 items-center gap-2.5 rounded-lg px-2.5 text-sm text-sidebar-muted transition-colors duration-150 hover-fine:bg-white/[0.05] hover-fine:text-white data-[status=active]:text-white [&_svg]:size-4 [&_svg]:shrink-0"
    >
      {({ isActive }) => (
        <>
          {isActive && (
            <motion.span
              layoutId="nav-active"
              className="absolute inset-0 rounded-lg bg-white/[0.08] shadow-[inset_0_0_0_1px_rgb(255_255_255/0.04)]"
              transition={{ type: "spring", duration: 0.22, bounce: 0 }}
            />
          )}
          <span className="relative flex items-center gap-2.5">
            <span className={cn(isActive ? "text-signal" : "text-current")}>{icon}</span>
            {children}
          </span>
        </>
      )}
    </Link>
  );
}

function VaultSwitcher({
  vaults,
  current,
}: {
  vaults: readonly VaultSummary[];
  current: VaultSummary | undefined;
}) {
  const navigate = useNavigate();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        className="flex w-full items-center gap-3 rounded-xl bg-white/[0.06] px-3 py-2.5 text-left transition-colors duration-150 hover-fine:bg-white/[0.09] aria-expanded:bg-white/[0.09]"
        data-testid="vault-switcher"
      >
        <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-signal text-ink">
          <Mark className="size-[18px]" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm font-medium text-white">
            {current?.name ?? "No vault yet"}
          </span>
          <span className="block text-[0.72rem] text-sidebar-muted">
            {current ? `${ofN(current.threshold, current.members)} to spend` : "Start or join one"}
          </span>
        </span>
        <ChevronsUpDownIcon className="size-4 shrink-0 text-sidebar-muted" />
      </DropdownMenuTrigger>
      <DropdownMenuContent className="w-60" align="start" sideOffset={6}>
        {vaults.length > 0 && (
          <DropdownMenuGroup>
            {vaults.map((vault) => (
              <DropdownMenuItem
                key={vault.address}
                onClick={() =>
                  void navigate({ to: "/vault/$address", params: { address: vault.address } })
                }
                className="gap-2.5 py-2"
              >
                <Mark className="size-4 text-olive" />
                <span className="flex-1 truncate">{vault.name}</span>
                <span className="text-xs text-muted-foreground">
                  {ofN(vault.threshold, vault.members)}
                </span>
              </DropdownMenuItem>
            ))}
          </DropdownMenuGroup>
        )}
        {vaults.length > 0 && <DropdownMenuSeparator />}
        <DropdownMenuItem onClick={() => void navigate({ to: "/new" })} className="gap-2.5">
          <PlusIcon /> New vault
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => void navigate({ to: "/join" })} className="gap-2.5">
          <TicketIcon /> Join with an invite
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function SidebarStatus() {
  const { data: state } = useAppState();
  const node = useNodeStatus(state?.engine.status === "running");
  const keeper = state?.keeper;
  const reachable = node.data?.reachable;
  return (
    <div className="mt-2 space-y-2 rounded-xl bg-white/[0.04] px-3 py-3 text-[0.75rem]">
      <Tooltip>
        <TooltipTrigger render={<div className="flex items-center justify-between" />}>
          <span className="flex items-center gap-2 text-sidebar-muted">
            <StatusDot tone={reachable ? "ok" : reachable === false ? "bad" : "off"} />
            Aztec
          </span>
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.span
              key={node.data?.blockNumber ?? "none"}
              initial={{ opacity: 0, y: 4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.2, ease: EASE_OUT }}
              className="tabular font-mono text-white/80"
            >
              {node.data?.blockNumber !== undefined
                ? `#${number.format(node.data.blockNumber)}`
                : "offline"}
            </motion.span>
          </AnimatePresence>
        </TooltipTrigger>
        <TooltipContent side="right">{node.data?.url ?? "No node"}</TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger render={<div className="flex items-center justify-between" />}>
          <span className="flex items-center gap-2 text-sidebar-muted">
            <StatusDot
              tone={
                keeper?.status === "running" ? "ok" : keeper?.status === "failed" ? "bad" : "busy"
              }
              pulse={keeper?.status === "starting" || keeper?.status === "restarting"}
            />
            Keeper
          </span>
          <span className="text-white/80">
            {keeper?.status === "running" ? "Online" : (keeper?.status ?? "…")}
          </span>
        </TooltipTrigger>
        <TooltipContent side="right">
          {keeper?.error ?? keeper?.peerId ?? "Your keeper"}
        </TooltipContent>
      </Tooltip>
      {state?.profile && (
        <div className="flex items-center gap-2 border-t border-white/[0.06] pt-2.5 text-white/85">
          <Avatar
            name={state.profile.name}
            identity={keeper?.identity ?? state.profile.name}
            size="sm"
          />
          <span className="truncate">{state.profile.name}</span>
        </div>
      )}
    </div>
  );
}
