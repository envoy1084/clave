// Golden vectors from @aztec-labs/* for tecpan-crypto's byte-compatibility tests. Deterministic: same Aztec version,
// same output. `--check` compares against the committed file instead of writing it.
import { readFile, writeFile } from "node:fs/promises";

import { Grumpkin } from "@aztec-labs/foundation/crypto/grumpkin";
import {
  poseidon2Hash,
  poseidon2HashWithSeparator,
  poseidon2Permutation,
} from "@aztec-labs/foundation/crypto/poseidon";
import { Fq, Fr } from "@aztec-labs/foundation/curves/bn254";
import { AztecAddress } from "@aztec-labs/stdlib/aztec-address";
import {
  computeAddress,
  computeAddressSecret,
  computePreaddress,
  deriveKeys,
  derivePublicKeyFromSecretKey,
} from "@aztec-labs/stdlib/keys";
import { appSiloEcdhSharedSecretPoint, deriveEcdhSharedSecretPoint } from "@aztec-labs/stdlib/logs";

const OUTPUT = new URL("../../crates/crypto/tests/vectors/aztec.json", import.meta.url);
const CONTRACT_ADDRESS_V3 = 993442748;
const APP_SILOED_ECDH_SHARED_SECRET = 1707851664;

const hex = (v) => v.toString();
const point = (p) => ({
  x: hex(p.x),
  y: hex(p.y),
  bytes: `0x${p.toBuffer().toString("hex")}`,
  compressed: `0x${p.toCompressedBuffer().toString("hex")}`,
});

const scalarMuls = await Promise.all(
  [1n, 2n, 7n, 0xdeadbeefn, Fq.MODULUS - 1n, (Fq.MODULUS - 1n) / 2n + 3n].map(async (s) => {
    const scalar = new Fq(s);
    return { scalar: hex(scalar), result: point(await derivePublicKeyFromSecretKey(scalar)) };
  }),
);

const toFields = (values) => values.map((v) => new Fr(v));
const poseidon2 = {
  permutation: {
    input: toFields([0n, 1n, 2n, 3n]).map(hex),
    output: (await poseidon2Permutation(toFields([0n, 1n, 2n, 3n]))).map(hex),
  },
  hashes: await Promise.all(
    [[1n], [1n, 2n], [1n, 2n, 3n], [1n, 2n, 3n, 4n], [7n, 8n, 9n, 10n, 11n]].map(
      async (values) => ({
        input: toFields(values).map(hex),
        output: hex(await poseidon2Hash(toFields(values))),
      }),
    ),
  ),
  separated: await Promise.all(
    [
      [CONTRACT_ADDRESS_V3, [5n, 6n]],
      [APP_SILOED_ECDH_SHARED_SECRET, [1n, 2n, 3n]],
    ].map(async ([separator, values]) => ({
      separator,
      input: toFields(values).map(hex),
      output: hex(await poseidon2HashWithSeparator(toFields(values), separator)),
    })),
  ),
};

/** Key derivation, address secret, ECDH and app siloing for one deterministic seed. */
async function account(seed) {
  const secretKey = new Fr(seed * 0x1234567n);
  const partialAddress = new Fr(seed * 0x9abcdefn + 11n);
  const { masterIncomingViewingSecretKey: ivsk, publicKeys } = await deriveKeys(secretKey);
  const publicKeysHash = await publicKeys.hash();
  const preaddress = await computePreaddress(publicKeysHash, partialAddress);
  const addressSecret = await computeAddressSecret(preaddress, ivsk);
  const ephSecret = new Fq(seed * 0x55555n + 3n);
  const ephPk = await derivePublicKeyFromSecretKey(ephSecret);
  const sharedPoint = await deriveEcdhSharedSecretPoint(addressSecret, ephPk);
  const app = new AztecAddress(new Fr(0xc0ffeen));
  return {
    ivsk: hex(ivsk),
    ivpk: point(publicKeys.ivpkM),
    publicKeysHash: hex(publicKeysHash),
    partialAddress: hex(partialAddress),
    preaddress: hex(preaddress),
    address: hex(await computeAddress(publicKeys, partialAddress)),
    addressSecret: hex(addressSecret),
    addressSecretNegated:
      addressSecret.toBigInt() !== ivsk.add(new Fq(preaddress.toBigInt())).toBigInt(),
    addressPoint: point(await derivePublicKeyFromSecretKey(addressSecret)),
    ecdh: {
      ephSecret: hex(ephSecret),
      ephPk: point(ephPk),
      sharedPoint: point(sharedPoint),
      app: hex(app),
      appSiloedSecret: hex(await appSiloEcdhSharedSecretPoint(sharedPoint, app)),
    },
  };
}

// Accounts covering both branches of the address-secret negation rule: the first three of each, in seed order.
const candidates = await Promise.all(Array.from({ length: 40 }, (_, i) => account(BigInt(i + 1))));
const negated = candidates.filter((a) => a.addressSecretNegated).slice(0, 3);
const plain = candidates.filter((a) => !a.addressSecretNegated).slice(0, 3);
if (negated.length < 3 || plain.length < 3)
  throw new Error("not enough accounts for both address-secret signs");
const accounts = [...plain, ...negated];

const vectors = {
  aztec: (await readFile(new URL("../../.aztecrc", import.meta.url), "utf8")).trim(),
  frModulus: `0x${Fr.MODULUS.toString(16)}`,
  fqModulus: `0x${Fq.MODULUS.toString(16)}`,
  domainSeparators: {
    contractAddressV3: CONTRACT_ADDRESS_V3,
    appSiloedEcdhSharedSecret: APP_SILOED_ECDH_SHARED_SECRET,
  },
  generator: point(Grumpkin.generator),
  poseidon2,
  scalarMuls,
  accounts,
};
const output = `${JSON.stringify(vectors, null, 2)}\n`;

if (process.argv.includes("--check")) {
  const committed = await readFile(OUTPUT, "utf8");
  if (committed !== output) {
    process.stderr.write(
      "crates/crypto/tests/vectors/aztec.json is stale: run `pnpm --filter @tecpan/aztec-vectors generate`\n",
    );
    process.exit(1);
  }
  process.stdout.write("aztec vectors up to date\n");
} else {
  await writeFile(OUTPUT, output);
  process.stdout.write(
    `wrote ${scalarMuls.length} scalar muls, ${accounts.length} accounts (3 plain, 3 negated)\n`,
  );
}
