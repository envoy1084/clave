import {
  Outlet,
  createHashHistory,
  createRootRoute,
  createRoute,
  createRouter,
  redirect,
} from "@tanstack/react-router";

import type { AppState } from "@shared/types.ts";

import { CreateVault } from "@/features/founding/create";
import { JoinVault } from "@/features/founding/join";
import { LobbyScreen } from "@/features/founding/lobby";
import { Onboarding } from "@/features/onboarding/onboarding";
import { Contacts } from "@/features/settings/contacts";
import { Settings } from "@/features/settings/settings";
import { Start } from "@/features/start/start";
import { Activity } from "@/features/vault/activity";
import { Internals } from "@/features/vault/internals";
import { Members } from "@/features/vault/members";
import { Overview } from "@/features/vault/overview";
import { ProposalDetail } from "@/features/vault/proposal";
import { NewProposal } from "@/features/vault/proposal-new";
import { Proposals } from "@/features/vault/proposals";
import { VaultShell } from "@/features/vault/shell";
import { call } from "@/lib/ipc";
import { keys, queryClient, useEventBridge } from "@/lib/state";

const appState = () =>
  queryClient.ensureQueryData<AppState>({ queryKey: keys.state, queryFn: () => call("app.state") });

/** Screens behind a profile send first-time users to onboarding. */
const requireProfile = async () => {
  if (!(await appState()).profile) throw redirect({ to: "/welcome" });
};

function Root() {
  useEventBridge();
  return <Outlet />;
}

const root = createRootRoute({ component: Root });

const index = createRoute({
  getParentRoute: () => root,
  path: "/",
  beforeLoad: async () => {
    const state = await appState();
    if (!state.profile) throw redirect({ to: "/welcome" });
    const first = state.vaults[0];
    if (first) throw redirect({ to: "/vault/$address", params: { address: first.address } });
    throw redirect({ to: "/start" });
  },
});

// One parent for every onboarding step, so the frame stays mounted while the steps transition inside it.
const onboarding = createRoute({
  getParentRoute: () => root,
  id: "onboarding",
  component: Onboarding,
});
const welcome = createRoute({ getParentRoute: () => onboarding, path: "/welcome" });
const setup = createRoute({ getParentRoute: () => onboarding, path: "/setup" });
const start = createRoute({
  getParentRoute: () => root,
  path: "/start",
  beforeLoad: requireProfile,
  component: Start,
});
const create = createRoute({
  getParentRoute: () => root,
  path: "/new",
  beforeLoad: requireProfile,
  component: CreateVault,
});
const join = createRoute({
  getParentRoute: () => root,
  path: "/join",
  beforeLoad: requireProfile,
  component: JoinVault,
});
const lobby = createRoute({
  getParentRoute: () => root,
  path: "/lobby/$lobbyId",
  beforeLoad: requireProfile,
  component: LobbyScreen,
});

const shell = createRoute({
  getParentRoute: () => root,
  id: "shell",
  beforeLoad: requireProfile,
  component: VaultShell,
});
const vault = createRoute({ getParentRoute: () => shell, path: "/vault/$address" });
const overview = createRoute({ getParentRoute: () => vault, path: "/", component: Overview });
const members = createRoute({ getParentRoute: () => vault, path: "/members", component: Members });
const proposals = createRoute({
  getParentRoute: () => vault,
  path: "/proposals",
  component: Proposals,
});
const newProposal = createRoute({
  getParentRoute: () => vault,
  path: "/proposals/new",
  component: NewProposal,
});
const proposal = createRoute({
  getParentRoute: () => vault,
  path: "/proposals/$id",
  component: ProposalDetail,
});
const activity = createRoute({
  getParentRoute: () => vault,
  path: "/activity",
  component: Activity,
});
const contacts = createRoute({
  getParentRoute: () => shell,
  path: "/contacts",
  component: Contacts,
});
const internals = createRoute({
  getParentRoute: () => vault,
  path: "/internals",
  component: Internals,
});
const settings = createRoute({
  getParentRoute: () => shell,
  path: "/settings",
  component: Settings,
});

const tree = root.addChildren([
  index,
  onboarding.addChildren([welcome, setup]),
  start,
  create,
  join,
  lobby,
  shell.addChildren([
    vault.addChildren([overview, proposals, newProposal, proposal, activity, members, internals]),
    contacts,
    settings,
  ]),
]);

export const router = createRouter({
  routeTree: tree,
  history: createHashHistory(),
  defaultPreload: "intent",
  scrollRestoration: true,
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
