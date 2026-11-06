import { Link } from "@tanstack/react-router";

import { Wordmark } from "@/components/brand/mark";
import { Terrain } from "@/components/brand/terrain";
import { Container } from "@/components/ui/section";
import { LINKS } from "@/lib/content";

import { NAV } from "./nav-items";

const DOCS = [
  { href: LINKS.contract, label: "contract.md" },
  { href: LINKS.viewing, label: "viewing-keys.md" },
  { href: LINKS.pxe, label: "pxe-integration.md" },
  { href: LINKS.keeper, label: "keeper.md" },
  { href: LINKS.networking, label: "networking.md" },
] as const;

export function SiteFooter() {
  return (
    <footer className="relative overflow-hidden bg-night text-paper">
      <Terrain seed={21} shade="left" className="absolute inset-0 opacity-50" />
      <Container className="relative grid gap-12 py-16 sm:grid-cols-[1.4fr_1fr_1fr]">
        <div>
          <Wordmark className="text-[1.3rem]" />
          <p className="mt-3 max-w-[34ch] text-[0.88rem] leading-relaxed text-paper/70">
            A private k-of-n multisig with threshold viewing, built on Aztec.
          </p>
        </div>
        <nav aria-label="Pages" className="flex flex-col gap-2 text-[0.88rem]">
          {NAV.map((item) => (
            <Link
              key={item.to}
              to={item.to}
              className="text-paper/80 underline-offset-4 hover-fine:text-paper hover-fine:underline"
            >
              {item.label}
            </Link>
          ))}
        </nav>
        <nav aria-label="Design documents" className="flex flex-col gap-2 font-mono text-[0.8rem]">
          {DOCS.map((d) => (
            <a
              key={d.href}
              href={d.href}
              className="text-paper/80 underline-offset-4 hover-fine:text-paper hover-fine:underline"
            >
              {d.label}
            </a>
          ))}
          <a
            href={LINKS.repo}
            className="mt-2 font-sans text-[0.88rem] font-medium text-signal underline-offset-4 hover-fine:underline"
          >
            Source on GitHub
          </a>
        </nav>
      </Container>
    </footer>
  );
}
