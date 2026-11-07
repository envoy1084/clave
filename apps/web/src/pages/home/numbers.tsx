import { Reveal } from "@/components/motion/reveal";
import { Container } from "@/components/ui/section";
import { GATES, TIMINGS } from "@/lib/content";

const entry = GATES[0];

// Measured by the repository's end-to-end suites on a laptop against a local network (lib/content.ts).
const FIGURES = [
  { value: TIMINGS[3].value, label: "for a 2-of-3 private transfer, proving included" },
  { value: TIMINGS[1].value, label: "for three keepers to answer a decryption, through a relay" },
  { value: TIMINGS[0].value, label: "to create a vault's viewing key across three keepers" },
  { value: entry.clave.toLocaleString("en-US"), label: "gates in the 8-signer entrypoint circuit" },
] as const;

export function Numbers() {
  return (
    <section aria-labelledby="numbers-title" className="py-16 sm:py-24">
      <Container>
        <h2
          id="numbers-title"
          className="serif-display text-[2.2rem] leading-[1.06] sm:text-[3rem]"
        >
          Benchmarks
        </h2>
        <Reveal>
          <dl className="mt-10 grid gap-px overflow-hidden rounded-3xl bg-line sm:grid-cols-2 lg:grid-cols-4">
            {FIGURES.map((f) => (
              <div key={f.label} className="flex flex-col-reverse bg-paper p-6">
                <dt className="mt-3 text-[0.9rem] leading-relaxed text-ink-soft">{f.label}</dt>
                <dd className="serif-display tabular text-[2.6rem] leading-none">{f.value}</dd>
              </div>
            ))}
          </dl>
        </Reveal>
      </Container>
    </section>
  );
}
