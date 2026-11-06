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
import { Setup } from "@/features/onboarding/setup";
import { Welcome } from "@/features/onboarding/welcome";
import { Settings } from "@/features/settings/settings";
import { Start } from "@/features/start/start";
import { Internals } from "@/features/vault/internals";
import { Members } from "@/features/vault/members";
import { Overview } from "@/features/vault/overview";
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

const welcome = createRoute({ getParentRoute: () => root, path: "/welcome", component: Welcome });
const setup = createRoute({ getParentRoute: () => root, path: "/setup", component: Setup });
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
  welcome,
  setup,
  start,
  create,
  join,
  lobby,
  shell.addChildren([vault.addChildren([overview, members, internals]), settings]),
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
