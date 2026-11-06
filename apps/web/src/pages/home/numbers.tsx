import { Link } from "@tanstack/react-router";

import { Reveal } from "@/components/motion/reveal";
import { Container } from "@/components/ui/section";
import { GATES, TIMINGS } from "@/lib/content";

const entry = GATES[0];

const FIGURES = [
  { value: TIMINGS[3].value, label: "for a 2-of-3 private transfer, proving included" },
  { value: TIMINGS[1].value, label: "for three keepers to answer a decryption, through a relay" },
  { value: TIMINGS[0].value, label: "to generate a vault's viewing key across three keepers" },
  { value: entry.tecpan.toLocaleString("en-US"), label: "gates in the 8-slot entrypoint circuit" },
] as const;

export function Numbers() {
  return (
    <section aria-labelledby="numbers-title" className="py-16 sm:py-24">
      <Container>
        <div className="flex flex-wrap items-end justify-between gap-4">
          <h2
            id="numbers-title"
            className="serif-display text-[2.2rem] leading-[1.06] sm:text-[3rem]"
          >
            Measured <em className="serif-em">on a laptop.</em>
          </h2>
          <Link
            to="/developers"
            hash="measurements"
            className="text-[0.9rem] font-medium text-olive underline-offset-4 hover-fine:underline"
          >
            All measurements
          </Link>
        </div>
        <dl className="mt-10 grid gap-px overflow-hidden rounded-3xl bg-line sm:grid-cols-2 lg:grid-cols-4">
          {FIGURES.map((f, i) => (
            <div key={f.label} className="bg-paper p-6">
              <Reveal delay={i * 0.06}>
                <dd className="serif-display tabular text-[2.6rem] leading-none">{f.value}</dd>
                <dt className="mt-3 text-[0.88rem] leading-relaxed text-ink-soft">{f.label}</dt>
              </Reveal>
            </div>
          ))}
        </dl>
        <p className="mt-4 text-[0.78rem] text-muted">
          macOS arm64 against an Aztec 6.0.0 local network, from the repository&rsquo;s end-to-end
          suites.
        </p>
      </Container>
    </section>
  );
}
