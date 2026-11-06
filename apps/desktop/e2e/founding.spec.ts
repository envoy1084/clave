// Phase 10 exit criterion: three app instances (separate profiles) found a 2-of-3 vault with a 2-of-3 viewing
// committee on a local network, and all three verify the same on-chain config.
import { expect, test } from "@playwright/test";

import { type Instance, foundVault, launch, networkAvailable, onboard } from "./app.ts";

const available = await networkAvailable();

test.skip(!available, "needs `aztec start --local-network`");

test("three members found a 2-of-3 vault through the app", async () => {
  const members: Instance[] = [];
  try {
    members.push(...(await Promise.all(["Ana", "Bea", "Cai"].map((n) => launch(n)))));
    const [ana] = members as [Instance, Instance, Instance];
    await ana.shot("welcome");
    await Promise.all(members.map(onboard));
    await ana.shot("start");
    const vault = await foundVault(members, "Treasury");
    expect(vault).toMatch(/^0x[0-9a-f]{64}$/u);
    await Promise.all(
      members.map((m) =>
        expect(m.page.getByText("On-chain config matches setup")).toBeVisible({ timeout: 120_000 }),
      ),
    );
    await ana.shot("overview");
    await ana.page.getByRole("link", { name: "Members" }).click();
    await expect(ana.page.getByTestId("members").locator("> li")).toHaveCount(3);
    await ana.shot("members");
    await ana.page.getByRole("link", { name: "Internals" }).click();
    await expect(ana.page.getByText("Matches the plan")).toBeVisible({ timeout: 60_000 });
    await ana.shot("internals");
  } finally {
    await Promise.all(members.map((m) => m.close()));
  }
});
