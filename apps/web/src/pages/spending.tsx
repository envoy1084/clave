import { PageHeader } from "@/components/ui/page-header";
import { Section } from "@/components/ui/section";
import { Steps } from "@/components/ui/steps";
import { ApprovalDiagram } from "@/figures/simple/approval";
import { SpendingIllustration } from "@/figures/simple/spending";
import { usePageMeta } from "@/lib/seo";

const LIFECYCLE = [
  {
    title: "Propose",
    body: "A member describes a payment. Their keeper shares it with the other members' keepers, encrypted end to end.",
  },
  {
    title: "Approve",
    body: "Each member's wallet rebuilds the transaction itself, shows exactly what it does, and only then signs.",
  },
  {
    title: "Execute",
    body: "Once there are enough signatures, any member proves the transaction on their machine and sends it.",
  },
  {
    title: "Or cancel",
    body: "A cancel is its own approved transaction that uses up the proposal, so old signatures can never be replayed.",
  },
] as const;

const CHECKS = [
  {
    title: "The signers are the vault's members",
    body: "The keys in the proof must match the member list stored privately in the vault.",
  },
  {
    title: "The deadline has not passed",
    body: "Every signature covers a deadline, so an approval cannot be used later than agreed.",
  },
  {
    title: "Enough of them signed",
    body: "Each signature is checked, and the payment runs only if at least k are valid.",
  },
] as const;

export function SpendingPage() {
  usePageMeta("spending");
  return (
    <>
      <PageHeader
        title={
          <>
            Every payment needs <em className="serif-em">k signatures.</em>
          </>
        }
        lede="A Clave vault is an Aztec account. It sends a payment only when the proof shows enough valid signatures from its members."
      />
      <SpendingIllustration />

      <Section
        id="lifecycle"
        title={
          <>
            From proposal <em className="serif-em">to payment.</em>
          </>
        }
        lede="Signatures are collected privately between members. Nothing touches the chain until there are enough."
      >
        <Steps steps={LIFECYCLE} />
      </Section>

      <Section
        id="checks"
        tone="dark"
        title={
          <>
            What the vault checks, <em className="serif-em text-signal">every time.</em>
          </>
        }
        lede="Members sign one short message. The vault's contract checks it inside the proof, so the rules cannot be skipped."
      >
        <div className="grid items-center gap-12 lg:grid-cols-[1.2fr_1fr]">
          <ApprovalDiagram />
          <ol className="space-y-7">
            {CHECKS.map((c) => (
              <li key={c.title}>
                <p className="text-[1.02rem] font-medium">{c.title}</p>
                <p className="mt-1 text-[0.94rem] leading-relaxed text-paper/75">{c.body}</p>
              </li>
            ))}
          </ol>
        </div>
      </Section>
    </>
  );
}
