import { Code, CodeBlock } from "@/components/ui/code";
import { NextPage } from "@/components/ui/next-page";
import { PageHeader } from "@/components/ui/page-header";
import { Container, Section } from "@/components/ui/section";
import { Steps } from "@/components/ui/steps";
import { ViewingFigure } from "@/figures/keepers/viewing-figure";
import { LINKS } from "@/lib/content";

const FLOW = [
  {
    title: "A note arrives",
    body: "Someone pays the vault. On Aztec the payment is an encrypted note that only the vault's viewing key can open.",
    detail: "note carries an ephemeral public key P",
  },
  {
    title: "The wallet asks",
    body: "Your wallet's key store has no viewing secret to use, so it sends P to the members' keepers instead.",
    detail: "patched PXE key store; batch request over the mesh",
  },
  {
    title: "Keepers answer, with proof",
    body: "Each keeper multiplies P by its share and proves it used the right share. A wrong answer is caught and its sender named.",
    detail: "Dᵢ = xᵢ · P, Chaum-Pedersen DLEQ",
  },
  {
    title: "The wallet combines",
    body: "Any t answers combine into exactly the value Aztec's own key store would have computed. The note opens; the key never existed.",
    detail: "Σ λᵢ · Dᵢ = address_secret · P",
  },
] as const;

const MATH = `member i:  xᵢ = σ · (sᵢ + h)            Aᵢ = xᵢ · G   (public)
partial:   Dᵢ = xᵢ · P                  DLEQ: log_G(Aᵢ) = log_P(Dᵢ)
combine:   address_secret · P = Σ_{i∈S} λᵢ · Dᵢ,   |S| = t_v`;

const FACTS = [
  {
    term: "Created without a dealer",
    body: "A Feldman DKG on Grumpkin gives each keeper a share. The group key is the vault's Ivpk, so its address is an ordinary Aztec address.",
  },
  {
    term: "Byte-for-byte Aztec",
    body: "Point encoding, the address-secret derivation and its sign rule, and the ECDH-to-AES step are pinned by golden tests against Aztec 6.0.0.",
  },
  {
    term: "Shares never travel",
    body: "A share is never exported or backed up. It moves only as sub-shares during resharing, sealed to the receiving keeper.",
  },
] as const;

export function ViewingPage() {
  return (
    <>
      <PageHeader
        title={
          <>
            Viewing needs a quorum. <em className="serif-em">The key never exists.</em>
          </>
        }
        lede="Every Aztec account has a secret that decrypts its incoming funds. In a Tecpan vault that secret is split across the members' keepers from the start. Reading a note takes any t of them, and no one, including the members, ever holds the whole thing."
        meta="tecpan-crypto · threshold ECDH · patched @aztec-labs/pxe key store"
      />
      <Container>
        <ViewingFigure />
      </Container>

      <Section
        id="flow"
        title={
          <>
            Reading one note, <em className="serif-em">step by step.</em>
          </>
        }
        lede="Measured end to end with three keepers on separate networks, reachable only through a relay: 7 to 12 ms per batch."
      >
        <Steps steps={FLOW} />
      </Section>

      <Section
        id="math"
        title="The arithmetic."
        lede={
          <>
            <Code>h</Code> is the vault&rsquo;s preaddress and <Code>σ</Code> the sign Aztec applies
            to the address secret. Both are public, so every keeper applies them to its own share
            without learning the secret.
          </>
        }
        aside={
          <a
            href={LINKS.viewing}
            className="text-[0.88rem] font-medium text-olive underline-offset-4 hover-fine:underline"
          >
            viewing-keys.md
          </a>
        }
      >
        <CodeBlock className="text-[0.74rem] sm:text-[0.84rem]">{MATH}</CodeBlock>
        <dl className="mt-10 grid gap-8 md:grid-cols-3">
          {FACTS.map((f) => (
            <div key={f.term}>
              <dt className="text-[0.98rem] font-medium">{f.term}</dt>
              <dd className="mt-2 text-[0.9rem] leading-relaxed text-ink-soft">{f.body}</dd>
            </div>
          ))}
        </dl>
      </Section>

      <NextPage
        to="/membership"
        label="How membership works"
        teaser="Remove a member and they lose the ability to read anything new, without moving funds to a new address."
      />
    </>
  );
}
