import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";

import { SiteLayout } from "@/layout/site-layout";
import { DevelopersPage } from "@/pages/developers/index";
import { HomePage } from "@/pages/home/home-page";
import { MembershipPage } from "@/pages/membership";
import { SpendingPage } from "@/pages/spending";
import { ViewingPage } from "@/pages/viewing";

const root = createRootRoute({ component: SiteLayout });

const routeTree = root.addChildren([
  createRoute({ getParentRoute: () => root, path: "/", component: HomePage }),
  createRoute({ getParentRoute: () => root, path: "/spending", component: SpendingPage }),
  createRoute({ getParentRoute: () => root, path: "/viewing", component: ViewingPage }),
  createRoute({ getParentRoute: () => root, path: "/membership", component: MembershipPage }),
  createRoute({ getParentRoute: () => root, path: "/developers", component: DevelopersPage }),
]);

export const router = createRouter({
  routeTree,
  scrollRestoration: true,
  defaultPreload: "intent",
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
