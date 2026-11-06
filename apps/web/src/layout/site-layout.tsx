import { Outlet } from "@tanstack/react-router";

import { SiteFooter } from "./site-footer";
import { SiteNav } from "./site-nav";

export function SiteLayout() {
  return (
    <>
      <a
        href="#content"
        className="sr-only z-50 rounded-lg bg-ink px-3 py-2 text-paper focus:not-sr-only focus:fixed focus:top-3 focus:left-3"
      >
        Skip to content
      </a>
      <SiteNav />
      <main id="content">
        <Outlet />
      </main>
      <SiteFooter />
    </>
  );
}
