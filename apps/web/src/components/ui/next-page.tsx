import { Link } from "@tanstack/react-router";

import { Container } from "./section";

type To = "/spending" | "/viewing" | "/membership" | "/developers";

/** The link that carries a reader from one mechanism to the next. */
export function NextPage({ to, label, teaser }: { to: To; label: string; teaser: string }) {
  return (
    <Container className="pb-24">
      <Link
        to={to}
        className="group flex flex-col gap-2 rounded-3xl bg-ink px-6 py-8 text-paper transition-transform duration-200 active:scale-[0.99] sm:flex-row sm:items-end sm:justify-between sm:px-10 sm:py-10"
      >
        <span>
          <span className="block font-mono text-[0.72rem] text-paper/60">Next</span>
          <span className="serif-display mt-1 block text-[2rem] leading-tight sm:text-[2.6rem]">
            {label}
          </span>
        </span>
        <span className="max-w-[40ch] text-[0.92rem] text-paper/70 transition-colors duration-150 group-hover:text-paper sm:text-right">
          {teaser}
        </span>
      </Link>
    </Container>
  );
}
