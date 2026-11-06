import { CodeBlock } from "@/components/ui/code";
import { PageHeader } from "@/components/ui/page-header";
import { Section } from "@/components/ui/section";
import { LINKS, TIMINGS } from "@/lib/content";

import { GateChart } from "./gate-chart";

const PREREQUISITES = ["Node 24", "pnpm 12", "Rust 1.96.0", "Aztec 6.0.0", "cargo-deny"] as const;

const COMMANDS = `git clone ${LINKS.repo}
cd tecpan && pnpm install
pnpm test                    # aztec test, cargo test, vitest

aztec start --local-network  # in a second terminal
pnpm test:e2e                # deploy vaults, threshold viewing, proposals
pnpm test:network            # Docker: a relay and 3 keepers on separate networks`;

const REPO = [
  {
    path: "contracts/vault_contract",
    what: "TecpanVault, the account contract (Noir, Aztec.nr v6)",
  },
  { path: "contracts/vault_test", what: "TestEnvironment tests for authorization and governance" },
  {
    path: "crates/crypto",
    what: "tecpan-crypto: Grumpkin, DKG, resharing, threshold ECDH, Aztec golden tests",
  },
  { path: "crates/wire", what: "tecpan-wire: message types and canonical encoding" },
  {
    path: "crates/keeper",
    what: "tecpan-keeper: share custody, ceremonies, libp2p mesh, proposals",
  },
  { path: "crates/relay", what: "tecpan-relay: untrusted circuit relay and ciphertext mailboxes" },
  { path: "crates/napi", what: "@tecpan/napi: Node bindings for the crypto and wire crates" },
  {
    path: "packages/vault-wallet",
    what: "PXE host: vault account class, witness encoder, threshold key store",
  },
  { path: "patches/", what: "the minimal @aztec-labs/pxe patch, every site marked tecpan:" },
  { path: "architecture/", what: "design documents for every verified decision" },
] as const;

const CLI = [
  { cmd: "tecpan init", what: "create a profile: signing key, keeper identity, daemon" },
  { cmd: "tecpan lobby open --members 3 -k 2 --tv 2", what: "found a vault with other members" },
  { cmd: "tecpan balance", what: "private balances, read through t keepers" },
  { cmd: "tecpan propose transfer 25 ttt --to bea", what: "propose a private transfer" },
  { cmd: "tecpan approve <id>", what: "review the rebuilt transaction, then sign" },
  { cmd: "tecpan execute <id>", what: "prove and send once k approvals are in" },
] as const;

const DOCS = [
  {
    href: LINKS.contract,
    name: "contract.md",
    what: "storage, authorization, wire formats, gate profile",
  },
  {
    href: LINKS.viewing,
    name: "viewing-keys.md",
    what: "DKG, threshold ECDH, resharing, guarantees",
  },
  {
    href: LINKS.pxe,
    name: "pxe-integration.md",
    what: "the key store patch and the upgrade procedure",
  },
  { href: LINKS.keeper, name: "keeper.md", what: "custody, storage, RPC and ceremonies" },
  {
    href: LINKS.networking,
    name: "networking.md",
    what: "mesh, sync, mailboxes and the relay trust model",
  },
  {
    href: LINKS.architecture,
    name: "architecture/",
    what: "every page, with the phase it landed in",
  },
] as const;

export function DevelopersPage() {
  return (
    <>
      <PageHeader
        title={
          <>
            Everything here <em className="serif-em">runs on your machine.</em>
          </>
        }
        lede="Tecpan is open source and pinned to Aztec 6.0.0. The suites below deploy vaults, run key generation between keepers and send private transfers on a local network; they are where every number on this site comes from."
        meta="v0 · local network · testnet release after the CLI"
      />

      <Section id="run" title="Run it locally." className="pt-0 sm:pt-0">
        <div className="grid gap-8 lg:grid-cols-[13rem_minmax(0,1fr)]">
          <div>
            <p className="text-[0.92rem] font-medium">Prerequisites</p>
            <ul className="mt-3 space-y-1.5 font-mono text-[0.8rem] text-ink-soft">
              {PREREQUISITES.map((p) => (
                <li key={p}>{p}</li>
              ))}
            </ul>
          </div>
          <CodeBlock>{COMMANDS}</CodeBlock>
        </div>
      </Section>

      <Section id="repository" title="What is in the repository.">
        <ul className="divide-y divide-line border-y border-line">
          {REPO.map((r) => (
            <li key={r.path} className="grid gap-1 py-3.5 sm:grid-cols-[16rem_1fr] sm:gap-6">
              <a
                href={`${LINKS.repo}/tree/main/${r.path}`}
                className="font-mono text-[0.84rem] underline-offset-4 hover-fine:underline"
              >
                {r.path}
              </a>
              <span className="text-[0.9rem] text-ink-soft">{r.what}</span>
            </li>
          ))}
        </ul>
      </Section>

      <Section
        id="measurements"
        title="Measurements."
        lede="Gate counts from aztec profile gates at MAX_SIGNERS = 8, next to Aztec's single-key Schnorr account. Timings from the end-to-end suites on macOS arm64."
      >
        <div className="grid gap-12 lg:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)]">
          <GateChart />
          <dl className="divide-y divide-line border-y border-line">
            {TIMINGS.map((t) => (
              <div key={t.what} className="grid grid-cols-[1fr_auto] gap-x-6 gap-y-1 py-4">
                <dt className="text-[0.92rem] font-medium">{t.what}</dt>
                <dd className="tabular row-span-2 self-center font-mono text-[1.05rem]">
                  {t.value}
                </dd>
                <dd className="text-[0.8rem] text-muted">{t.where}</dd>
              </div>
            ))}
          </dl>
        </div>
      </Section>

      <Section
        id="cli"
        title="The tecpan CLI."
        lede="In development, and not in the repository yet. It will be the main way to use a vault: a Rust command line talking to a local daemon that hosts the PXE and supervises your keeper. A few of the planned commands:"
      >
        <ul className="divide-y divide-line border-y border-line">
          {CLI.map((c) => (
            <li
              key={c.cmd}
              className="grid gap-1 py-3.5 sm:grid-cols-[minmax(0,22rem)_1fr] sm:gap-6"
            >
              <code className="font-mono text-[0.84rem]">{c.cmd}</code>
              <span className="text-[0.9rem] text-ink-soft">{c.what}</span>
            </li>
          ))}
        </ul>
      </Section>

      <Section
        id="docs"
        title="Design documents."
        lede="Each page describes what is built and verified, nothing planned."
        className="pb-24 sm:pb-32"
      >
        <ul className="grid gap-px overflow-hidden rounded-3xl bg-line sm:grid-cols-2 lg:grid-cols-3">
          {DOCS.map((d) => (
            <li key={d.name} className="bg-paper">
              <a
                href={d.href}
                className="block h-full p-5 transition-colors duration-150 hover-fine:bg-paper-2 sm:p-6"
              >
                <span className="font-mono text-[0.86rem]">{d.name}</span>
                <span className="mt-2 block text-[0.88rem] text-ink-soft">{d.what}</span>
              </a>
            </li>
          ))}
        </ul>
      </Section>
    </>
  );
}
