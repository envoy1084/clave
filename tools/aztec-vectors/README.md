# @clave/aztec-vectors

Generates golden vectors from `@aztec-labs/*` (pinned in the workspace catalog) for the byte-compatibility tests in
`crates/crypto`: Grumpkin generator and encodings, scalar multiplication, Poseidon2, key derivation, address secret
(both signs), ECDH and app siloing.

```sh
pnpm --filter @clave/aztec-vectors generate   # rewrite crates/crypto/tests/vectors/aztec.json
pnpm --filter @clave/aztec-vectors test       # fail if the committed vectors differ from the installed Aztec
```

Run `generate` and the Rust tests on every Aztec upgrade.
