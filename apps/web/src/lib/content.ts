// Every fact the page states, with where it comes from in the repository. Numbers are measurements, never estimates.

export const LINKS = {
  repo: "https://github.com/envoy1084/tecpan",
  architecture: "https://github.com/envoy1084/tecpan/tree/main/architecture",
  contract: "https://github.com/envoy1084/tecpan/blob/main/architecture/contract.md",
  viewing: "https://github.com/envoy1084/tecpan/blob/main/architecture/viewing-keys.md",
  keeper: "https://github.com/envoy1084/tecpan/blob/main/architecture/keeper.md",
  networking: "https://github.com/envoy1084/tecpan/blob/main/architecture/networking.md",
  pxe: "https://github.com/envoy1084/tecpan/blob/main/architecture/pxe-integration.md",
  cli: "https://github.com/envoy1084/tecpan/blob/main/architecture/cli.md",
} as const;

/** architecture/contract.md, "Gate profile" (aztec profile gates, MAX_SIGNERS = 8). */
export const GATES = [
  { circuit: "entrypoint", tecpan: 27_176, schnorr: 15_002 },
  { circuit: "verify_private_authwit", tecpan: 18_665, schnorr: 6_489 },
  { circuit: "constructor", tecpan: 25_351, schnorr: 7_905 },
  { circuit: "rotate_membership", tecpan: 85_734, schnorr: null },
] as const;

/** Timings from the end-to-end suites (progress log, phases 9–11), macOS arm64, Aztec local network. */
export const TIMINGS = [
  { what: "DKG, 3 keepers", value: "≈ 0.4 s", where: "three keepers on one machine" },
  {
    what: "Threshold ECDH batch, 3 keepers via a relay",
    value: "7–12 ms",
    where: "keepers on separate Docker networks",
  },
  { what: "Vault deployment", value: "≈ 4 s", where: "simulated proof, includes mining" },
  {
    what: "2-of-3 private transfer",
    value: "6.9 s",
    where: "client-side proving, includes mining",
  },
] as const;

/** research/progress.md status table, as shipped in the repository. */
export const STATUS = [
  {
    name: "Vault account contract",
    detail: "k-of-n Schnorr entrypoint, private config notes, governance",
    done: true,
  },
  {
    name: "Threshold cryptography",
    detail: "Feldman DKG, resharing, threshold ECDH with DLEQ proofs",
    done: true,
  },
  {
    name: "PXE integration",
    detail: "patched key store; the incoming viewing secret is never loaded",
    done: true,
  },
  {
    name: "Keeper and relay",
    detail: "share custody, ceremonies, libp2p mesh, ciphertext mailboxes",
    done: true,
  },
  {
    name: "Proposals end to end",
    detail: "propose, approve, execute and cancel across members",
    done: true,
  },
  {
    name: "CLI and daemon",
    detail: "tecpan and tecpand: found, propose, review, approve, reject, execute",
    done: true,
  },
  {
    name: "Membership drill",
    detail: "remove a member, prove it cannot read new notes",
    done: false,
  },
  { name: "Testnet release", detail: "binaries, hosted relay, live vault", done: false },
] as const;
