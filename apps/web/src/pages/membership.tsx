import { PageHeader } from "@/components/ui/page-header";
import { Section } from "@/components/ui/section";
import { Steps } from "@/components/ui/steps";
import { MembershipIllustration } from "@/figures/simple/membership";
import { usePageMeta } from "@/lib/seo";

const ROTATION = [
  {
    title: "Agree",
    body: "A membership change is a proposal like any other, approved by the vault's current threshold.",
  },
  {
    title: "Reshare",
    body: "Enough of the staying members deal the same key out again, as new shares for the new group.",
  },
  {
    title: "Verify",
    body: "Every new member checks the dealers, and that the new shares still add up to the same key.",
  },
  {
    title: "Switch",
    body: "The vault records the new group. Keepers switch to the new shares and erase the old ones.",
  },
] as const;

// architecture/viewing-keys.md, "Guarantees and limits".
const GUARANTEES = [
  { property: "Nobody learns the viewing key", holds: "Fewer than t members collude" },
  { property: "A cheating keeper is caught", holds: "Always: every answer carries a proof" },
  {
    property: "A removed member cannot read new payments",
    holds: "Fewer than t removed at once, and old shares erased",
  },
  { property: "The vault keeps its address", holds: "Always" },
  { property: "What a member saw before leaving", holds: "Cannot be taken back" },
] as const;

export function MembershipPage() {
  usePageMeta("membership");
  return (
    <>
      <PageHeader
        title={
          <>
            Remove a member. <em className="serif-em">Keep the address.</em>
          </>
        }
        lede="In most private wallets, removing someone who can see the funds means moving everything to a new account. In Clave the remaining members reshare the key instead, and the share that left stops working."
      />
      <MembershipIllustration />

      <Section
        id="rotation"
        title={
          <>
            One change, <em className="serif-em">four steps.</em>
          </>
        }
      >
        <Steps steps={ROTATION} />
      </Section>

      <Section
        id="guarantees"
        tone="dark"
        title={
          <>
            What holds, <em className="serif-em text-signal">and its limits.</em>
          </>
        }
        lede="Including the one thing resharing cannot do."
      >
        <table className="w-full text-left text-[0.95rem]">
          <thead>
            <tr className="text-[0.85rem] text-paper/70">
              <th scope="col" className="pb-3 font-normal">
                Guarantee
              </th>
              <th scope="col" className="pb-3 pl-4 font-normal">
                Holds when
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-paper/10 border-y border-paper/10">
            {GUARANTEES.map((g) => (
              <tr key={g.property}>
                <th scope="row" className="py-4 pr-4 align-top font-medium">
                  {g.property}
                </th>
                <td className="py-4 pl-4 align-top text-paper/75">{g.holds}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Section>
    </>
  );
}
