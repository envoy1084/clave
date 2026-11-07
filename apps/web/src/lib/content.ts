// Every fact the page states, with where it comes from in the repository. Numbers are measurements, never estimates.

export const LINKS = {
  repo: "https://github.com/envoy1084/clave",
} as const;

/** architecture/contract.md, "Gate profile" (aztec profile gates, MAX_SIGNERS = 8). */
export const GATES = [
  { circuit: "entrypoint", clave: 27_176, schnorr: 15_002 },
  { circuit: "verify_private_authwit", clave: 18_665, schnorr: 6_489 },
  { circuit: "constructor", clave: 25_351, schnorr: 7_905 },
  { circuit: "rotate_membership", clave: 85_734, schnorr: null },
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
