import { Code } from "@/components/ui/code";
import { NextPage } from "@/components/ui/next-page";
import { PageHeader } from "@/components/ui/page-header";
import { Container, Section } from "@/components/ui/section";
import { Steps } from "@/components/ui/steps";
import { MembershipFigure } from "@/figures/keepers/membership-figure";

const ROTATION = [
  {
    title: "Agree",
    body: "The change is a proposal like any other: the vault's current threshold must approve it.",
    detail: "rotate_membership is #[only_self]",
  },
  {
    title: "Reshare",
    body: "A qualified set of staying members deals the same secret onto a new polynomial for the new committee.",
    detail: "dealer i shares λᵢ · sᵢ; receivers check λᵢ · Yᵢ",
  },
  {
    title: "Verify",
    body: "Every new member checks each dealer and that the new shares still add up to the vault's unchanged public key.",
    detail: "Σ new constant terms = Ivpk, else GroupKeyMismatch",
  },
  {
    title: "Switch",
    body: "The new committee commitment is written to the config note. Keepers switch shares when it lands, and old approvals stop counting.",
    detail: "config version + 1; old shares erased",
  },
] as const;

// architecture/viewing-keys.md, "Guarantees and limits".
const GUARANTEES = [
  { property: "No one learns the viewing secret", holds: "Fewer than t members collude" },
  {
    property: "A cheating keeper cannot corrupt a decryption unnoticed",
    holds: "Always: every answer carries a DLEQ proof",
  },
  {
    property: "A removed member cannot read anything after the reshare",
    holds: "Fewer than t removed per rotation, and old shares erased",
  },
  { property: "The vault keeps its address", holds: "Always: the public key never changes" },
  { property: "What a member saw before removal", holds: "Cannot be taken back" },
] as const;

export function MembershipPage() {
  return (
    <>
      <PageHeader
        title={
          <>
            Remove a member. <em className="serif-em">Keep the address.</em>
          </>
        }
        lede="In most private wallets, losing trust in someone with the viewing key means moving everything to a new account. In Tecpan the remaining members reshare the key instead: same secret, new shares, and the person who left holds a share that no longer fits."
        meta={
          <>
            rotate_membership · signers and viewing committee change together · at most t_v − 1
            removed per rotation
          </>
        }
      />
      <Container>
        <MembershipFigure />
      </Container>

      <Section
        id="rotation"
        title={
          <>
            One rotation, <em className="serif-em">four steps.</em>
          </>
        }
      >
        <Steps steps={ROTATION} />
      </Section>

      <Section
        id="guarantees"
        title="Guarantees, and their limits."
        lede="Stated exactly as the design document states them, including the one thing resharing cannot do."
      >
        <table className="w-full text-left text-[0.92rem]">
          <thead>
            <tr className="text-[0.76rem] text-muted">
              <th scope="col" className="pb-3 font-normal">
                Property
              </th>
              <th scope="col" className="pb-3 pl-4 font-normal">
                Holds when
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line border-y border-line">
            {GUARANTEES.map((g) => (
              <tr key={g.property}>
                <th scope="row" className="py-4 pr-4 align-top font-medium">
                  {g.property}
                </th>
                <td className="py-4 pl-4 align-top text-ink-soft">{g.holds}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <p className="mt-6 max-w-[70ch] text-[0.86rem] leading-relaxed text-muted">
          Resharing and <Code>rotate_membership</Code> are built and tested. The end-to-end drill,
          removing a member from a live vault and showing they cannot read the next note, is the
          next milestone.
        </p>
      </Section>

      <NextPage
        to="/developers"
        label="Run it yourself"
        teaser="Clone the repository, start a local network and run the suites that produced every number on this site."
      />
    </>
  );
}
