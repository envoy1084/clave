import { Code, CodeBlock } from "@/components/ui/code";
import { NextPage } from "@/components/ui/next-page";
import { PageHeader } from "@/components/ui/page-header";
import { Container, Section } from "@/components/ui/section";
import { Steps } from "@/components/ui/steps";
import { LockFigure } from "@/figures/lock/lock-figure";
import { LINKS } from "@/lib/content";

const LIFECYCLE = [
  {
    title: "Propose",
    body: "A member describes a payment or contract call. Their keeper shares it with the other members' keepers, sealed end to end.",
    detail: "summary + calls (≤ 64 KiB), expires_at, config_version",
  },
  {
    title: "Approve",
    body: "Each member's wallet rebuilds the transaction from its calls, shows exactly what it does, and only then signs.",
    detail: "Schnorr over approval = Poseidon2(…, outer_hash, …)",
  },
  {
    title: "Execute",
    body: "Any member with enough approvals proves the transaction locally and sends it. Members who were offline catch up from their keeper.",
    detail: "PXE serves the 49-field witness; entrypoint checks it",
  },
  {
    title: "Or cancel",
    body: "A proposal is cancelled by spending its nonce in a separate, approved transaction, so old signatures can never be replayed.",
    detail: "cancel burns the nonce; needs k approvals itself",
  },
] as const;

const APPROVAL = `approval = Poseidon2("clave.approval.v0",
  [vault_address, outer_hash, config_version, expires_at])`;

const WITNESS = [
  { fields: "[0, 16)", what: "Signer keys x, y in slot order; empty slots are zero" },
  { fields: "[16, 48)", what: "Per slot s.lo, s.hi, e.lo, e.hi; zero means no approval" },
  { fields: "48", what: "expires_at, range checked to u64" },
] as const;

const CHECKS = [
  {
    check: "signer_set_commitment",
    what: "The witness keys hash to the commitment stored in the private config note.",
  },
  {
    check: "anchor_timestamp ≤ expires_at",
    what: "The deadline is bound into every signature and capped by the config age.",
  },
  {
    check: "approvals ≥ threshold",
    what: "Every slot is verified; empty slots against the generator, so the circuit has one shape.",
  },
] as const;

export function SpendingPage() {
  return (
    <>
      <PageHeader
        title={
          <>
            Spending is <em className="serif-em">k of n,</em> proven.
          </>
        }
        lede="A Clave vault is an Aztec account contract. It runs a payment only if the proof shows enough valid signatures from its members. There is no admin key and no server that can approve on anyone's behalf."
        meta="ClaveVault · entrypoint and verify_private_authwit · MAX_SIGNERS 8"
      />
      <Container>
        <LockFigure fig="Fig. 1" />
        <p className="mt-4 max-w-[72ch] text-[0.84rem] leading-relaxed text-muted">
          The box is the auth witness. Eight slots, five of them belonging to members of a 3-of-5
          vault; three are blank but still checked, so a 2-of-2 vault and an 8-of-8 vault produce
          the same circuit. A turned key is a valid signature; the bolt withdraws at the threshold.
        </p>
      </Container>

      <Section
        id="lifecycle"
        title={
          <>
            From proposal <em className="serif-em">to transaction.</em>
          </>
        }
        lede="Approvals are collected off-chain between members' keepers. Nothing touches the chain until the vault has enough of them."
      >
        <Steps steps={LIFECYCLE} />
      </Section>

      <Section
        id="witness"
        title="What the entrypoint checks."
        lede={
          <>
            The PXE answers the circuit&rsquo;s request for the auth witness of the
            transaction&rsquo;s <Code>outer_hash</Code>. The contract trusts none of it until each
            piece is checked.
          </>
        }
        aside={
          <a
            href={LINKS.contract}
            className="text-[0.88rem] font-medium text-olive underline-offset-4 hover-fine:underline"
          >
            contract.md
          </a>
        }
      >
        <div className="grid gap-10 lg:grid-cols-2">
          <div>
            <CodeBlock>{APPROVAL}</CodeBlock>
            <table className="mt-6 w-full text-left text-[0.86rem]">
              <caption className="mb-2 text-left text-[0.92rem] font-medium">
                Auth witness, 49 fields
              </caption>
              <tbody className="divide-y divide-line border-y border-line">
                {WITNESS.map((row) => (
                  <tr key={row.fields}>
                    <th
                      scope="row"
                      className="w-24 py-3 pr-4 align-top font-mono text-[0.8rem] font-normal"
                    >
                      {row.fields}
                    </th>
                    <td className="py-3 text-ink-soft">{row.what}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <ol className="space-y-6">
            {CHECKS.map((c, i) => (
              <li key={c.check} className="grid grid-cols-[2rem_1fr] gap-x-3">
                <span className="font-mono text-[0.75rem] text-muted">
                  {String(i + 1).padStart(2, "0")}
                </span>
                <div>
                  <p className="font-mono text-[0.86rem]">{c.check}</p>
                  <p className="mt-1 text-[0.9rem] leading-relaxed text-ink-soft">{c.what}</p>
                </div>
              </li>
            ))}
            <li className="grid grid-cols-[2rem_1fr] gap-x-3">
              <span className="font-mono text-[0.75rem] text-muted">04</span>
              <div>
                <p className="font-mono text-[0.86rem]">no nullifying reads</p>
                <p className="mt-1 text-[0.9rem] leading-relaxed text-ink-soft">
                  The config note is read without being consumed, so the same check works inside the
                  static <Code>verify_private_authwit</Code> call other contracts make.
                </p>
              </div>
            </li>
          </ol>
        </div>
      </Section>

      <NextPage
        to="/viewing"
        label="How viewing works"
        teaser="Spending is half of it. See how the vault's notes are read without anyone holding the key."
      />
    </>
  );
}
