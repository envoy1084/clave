import { useEffect, useState } from "react";

import { Link, useRouterState } from "@tanstack/react-router";

import { AnimatePresence, motion } from "motion/react";

import { Wordmark } from "@/components/brand/mark";
import { Container } from "@/components/ui/section";
import { LINKS } from "@/lib/content";
import { quick } from "@/lib/motion";

import { NAV } from "./nav-items";

// The bar is navy on every page: it continues the hero on the home page and frames the light pages below it.
const linkClass =
  "rounded-lg px-3 py-2 text-[0.88rem] text-paper/70 transition-colors duration-150 hover-fine:text-paper data-[status=active]:text-paper data-[status=active]:font-medium";

/** True once the page has scrolled past the top few pixels. */
function useScrolled() {
  const [scrolled, setScrolled] = useState(false);
  useEffect(() => {
    const update = () => setScrolled(window.scrollY > 24);
    update();
    window.addEventListener("scroll", update, { passive: true });
    return () => window.removeEventListener("scroll", update);
  }, []);
  return scrolled;
}

export function SiteNav() {
  const [open, setOpen] = useState(false);
  const home = useRouterState({ select: (s) => s.location.pathname === "/" });
  const scrolled = useScrolled();
  // Over the home hero the bar is part of the landscape; anywhere else, or once scrolled, it is solid navy.
  const solid = !home || scrolled || open;
  return (
    <header
      className={`sticky top-0 z-40 text-paper transition-colors duration-300 ${solid ? "bg-navy" : "bg-transparent"}`}
    >
      <Container wide className="flex h-16 items-center justify-between gap-6">
        <Link
          to="/"
          className="text-[1.05rem]"
          aria-label="Clave home"
          onClick={() => setOpen(false)}
        >
          <Wordmark />
        </Link>
        <nav aria-label="Main" className="hidden items-center gap-1 md:flex">
          {NAV.map((item) => (
            <Link key={item.to} to={item.to} className={linkClass}>
              {item.label}
            </Link>
          ))}
        </nav>
        <div className="flex items-center gap-2">
          <a
            href={LINKS.repo}
            className="rounded-lg px-3 py-2 text-[0.88rem] font-medium shadow-[inset_0_0_0_1px_rgb(251_248_242/0.22)] transition-colors duration-150 hover-fine:bg-paper/10"
          >
            GitHub
          </a>
          <button
            type="button"
            className="grid size-10 place-items-center rounded-lg md:hidden"
            aria-expanded={open}
            aria-controls="mobile-nav"
            aria-label={open ? "Close menu" : "Open menu"}
            onClick={() => setOpen((o) => !o)}
          >
            <span className="relative block h-3 w-5" aria-hidden>
              <motion.span
                className="absolute inset-x-0 top-0 h-[1.5px] bg-paper"
                animate={open ? { y: 5, rotate: 45 } : { y: 0, rotate: 0 }}
                transition={quick}
              />
              <motion.span
                className="absolute inset-x-0 bottom-0 h-[1.5px] bg-paper"
                animate={open ? { y: -5.5, rotate: -45 } : { y: 0, rotate: 0 }}
                transition={quick}
              />
            </span>
          </button>
        </div>
      </Container>
      <AnimatePresence>
        {open && (
          <motion.nav
            id="mobile-nav"
            aria-label="Main"
            className="border-t border-paper/10 md:hidden"
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={quick}
          >
            <Container className="flex flex-col py-2">
              {NAV.map((item) => (
                <Link
                  key={item.to}
                  to={item.to}
                  className={`${linkClass} py-3 text-[1rem]`}
                  onClick={() => setOpen(false)}
                >
                  {item.label}
                </Link>
              ))}
            </Container>
          </motion.nav>
        )}
      </AnimatePresence>
    </header>
  );
}
