import { PageHeader } from "@/components/ui/page-header";
import { Section } from "@/components/ui/section";
import { Steps } from "@/components/ui/steps";
import { CombineDiagram } from "@/figures/simple/combine";
import { ViewingIllustration } from "@/figures/simple/viewing";

const FLOW = [
  {
    title: "A payment arrives",
    body: "On Aztec it arrives as an encrypted note that only the vault's viewing key can open.",
  },
  {
    title: "The wallet asks",
    body: "Your wallet has no viewing key to use, so it asks the members' keepers for help instead.",
  },
  {
    title: "Keepers answer, with proof",
    body: "Each keeper answers with a piece computed from its share, and proves it used the right share.",
  },
  {
    title: "The note opens",
    body: "Any t answers combine into exactly what the full key would have produced. The key itself never appears.",
  },
] as const;

const FACTS = [
  {
    term: "Created without a dealer",
    body: "The members' keepers generate the key together. Each gets a share; nobody ever holds the whole key.",
  },
  {
    term: "A normal Aztec address",
    body: "The vault's address and encryption match what Aztec expects, verified against Aztec's own output.",
  },
  {
    term: "Shares stay put",
    body: "A share is never exported or backed up. It moves only when members change, sealed to its new holder.",
  },
] as const;

export function ViewingPage() {
  return (
    <>
      <PageHeader
        title={
          <>
            Reading the vault takes <em className="serif-em">t keepers.</em>
          </>
        }
        lede="Every Aztec account has a key that decrypts what it receives. In Clave that key is split across the members' keepers from day one, and no one ever holds all of it."
      />
      <ViewingIllustration />

      <Section
        id="flow"
        title={
          <>
            Reading one note, <em className="serif-em">step by step.</em>
          </>
        }
        lede="With three keepers on separate networks, an answer takes 7 to 12 milliseconds."
      >
        <Steps steps={FLOW} />
      </Section>

      <Section
        id="key"
        tone="dark"
        title={
          <>
            The key that <em className="serif-em text-signal">never exists.</em>
          </>
        }
        lede="Each keeper works only with its own share. The combination happens on the answers, never on the key."
      >
        <CombineDiagram className="mx-auto max-w-[860px]" />
        <dl className="mt-12 grid gap-8 md:grid-cols-3">
          {FACTS.map((f) => (
            <div key={f.term}>
              <dt className="text-[1.02rem] font-medium">{f.term}</dt>
              <dd className="mt-2 text-[0.94rem] leading-relaxed text-paper/75">{f.body}</dd>
            </div>
          ))}
        </dl>
      </Section>
    </>
  );
}
