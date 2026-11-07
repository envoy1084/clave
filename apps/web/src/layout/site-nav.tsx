import { useState } from "react";

import { Link } from "@tanstack/react-router";

import { AnimatePresence, motion } from "motion/react";

import { Wordmark } from "@/components/brand/mark";
import { Container } from "@/components/ui/section";
import { LINKS } from "@/lib/content";
import { quick } from "@/lib/motion";

import { NAV } from "./nav-items";

const linkClass =
  "rounded-lg px-3 py-2 text-[0.88rem] text-ink-soft transition-colors duration-150 hover-fine:text-ink data-[status=active]:text-ink data-[status=active]:font-medium";

export function SiteNav() {
  const [open, setOpen] = useState(false);
  return (
    <header className="sticky top-0 z-40 bg-paper/85 backdrop-blur-md">
      <Container className="flex h-16 items-center justify-between gap-6">
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
            className="rounded-lg px-3 py-2 text-[0.88rem] font-medium shadow-[inset_0_0_0_1px_var(--color-line)] transition-colors duration-150 hover-fine:bg-paper-2"
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
                className="absolute inset-x-0 top-0 h-[1.5px] bg-ink"
                animate={open ? { y: 5, rotate: 45 } : { y: 0, rotate: 0 }}
                transition={quick}
              />
              <motion.span
                className="absolute inset-x-0 bottom-0 h-[1.5px] bg-ink"
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
            className="border-t border-line md:hidden"
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
