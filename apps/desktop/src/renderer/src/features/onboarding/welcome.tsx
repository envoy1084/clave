import { useNavigate } from "@tanstack/react-router";

import { Button } from "@/components/ui/button";

const SPEC = [
  {
    term: "Spend",
    body: "The account contract's entrypoint runs a payload only with k of n Schnorr signatures over (vault, outer hash, config version, expiry).",
  },
  {
    term: "View",
    body: "The incoming viewing key is never assembled. Your PXE gets address_secret·P from t_v keepers, each partial checked with a DLEQ proof.",
  },
  {
    term: "Custody",
    body: "Your Grumpkin signing key is sealed in the OS keychain. Your viewing share lives in a keeper process on this device.",
  },
];

export function Welcome() {
  const navigate = useNavigate();
  return (
    <div>
      <h1 className="serif-display text-[2.6rem] leading-[1.04]">
        Private k-of-n multisig on Aztec
      </h1>
      <dl className="mt-8 divide-y divide-border border-y border-border">
        {SPEC.map(({ term, body }) => (
          <div key={term} className="grid grid-cols-[88px_1fr] gap-4 py-3.5">
            <dt className="text-sm font-medium">{term}</dt>
            <dd className="text-[0.825rem] leading-relaxed text-muted-foreground">{body}</dd>
          </div>
        ))}
      </dl>
      <div className="mt-8 flex items-center gap-4">
        <Button
          size="xl"
          onClick={() => void navigate({ to: "/setup" })}
          data-testid="welcome-start"
        >
          Create profile
        </Button>
        <p className="max-w-[30ch] text-xs leading-snug text-muted-foreground">
          Generates a signing key, a keeper identity and an Aztec account on this device.
        </p>
      </div>
    </div>
  );
}
