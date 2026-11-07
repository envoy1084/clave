import { Link } from "@tanstack/react-router";

import { Wordmark } from "@/components/brand/mark";
import { Terrain } from "@/components/brand/terrain";
import { Container } from "@/components/ui/section";
import { LINKS } from "@/lib/content";

import { NAV } from "./nav-items";

const linkClass = "text-paper/80 underline-offset-4 hover-fine:text-paper hover-fine:underline";

export function SiteFooter() {
  return (
    <footer className="relative overflow-hidden bg-navy text-paper">
      <Terrain seed={21} shade="left" className="absolute inset-0 opacity-50" />
      <Container className="relative flex flex-col gap-10 py-16 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <Wordmark className="text-[1.3rem]" />
          <p className="mt-3 max-w-[34ch] text-[0.9rem] leading-relaxed text-paper/75">
            A private k-of-n multisig with threshold viewing, built on Aztec.
          </p>
        </div>
        <nav aria-label="Pages" className="flex flex-col gap-2 text-[0.92rem] sm:items-end">
          {NAV.map((item) => (
            <Link key={item.to} to={item.to} className={linkClass}>
              {item.label}
            </Link>
          ))}
          <a
            href={LINKS.repo}
            className="mt-2 font-medium text-signal underline-offset-4 hover-fine:underline"
          >
            Source on GitHub
          </a>
        </nav>
      </Container>
    </footer>
  );
}
