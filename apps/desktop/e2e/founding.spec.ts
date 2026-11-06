/* oxlint-disable no-await-in-loop -- members act one after another, as people would. */
// Phase 10 exit criterion: three app instances (separate profiles) found a 2-of-3 vault with a 2-of-3 viewing
// committee on a local network, and all three see the same verified on-chain config.
import { expect, test } from "@playwright/test";

import { type Instance, launch, networkAvailable, onboard } from "./app.ts";

const available = await networkAvailable();

test.skip(!available, "needs `aztec start --local-network`");

test("three members found a 2-of-3 vault through the app", async () => {
  const members: Instance[] = [];
  try {
    members.push(...(await Promise.all(["Ana", "Bea", "Cai"].map(launch))));
    const [ana, bea, cai] = members as [Instance, Instance, Instance];
    await ana.shot("welcome");
    await Promise.all(members.map(onboard));
    await ana.shot("start");

    // Ana starts the vault and shares the invite.
    await ana.page.getByTestId("start-new").click();
    await ana.page.getByTestId("vault-name").fill("Founders vault");
    await ana.shot("create");
    await ana.page.getByTestId("vault-create").click();
    const invite = (await ana.page.getByTestId("invite-code").textContent())?.trim() ?? "";
    expect(invite).toMatch(/^tecpan-invite-v0:/u);

    // Bea and Cai preview the invite, then join.
    for (const guest of [bea, cai]) {
      await guest.page.getByTestId("start-join").click();
      await guest.page.getByTestId("invite-input").fill(invite);
      await expect(guest.page.getByTestId("invite-name")).toHaveText("Founders vault");
      await guest.shot("invite-preview");
      await guest.page.getByTestId("invite-join").click();
      await expect(guest.page.getByTestId("lobby-name")).toHaveText("Founders vault");
    }

    // Everyone sees the full room; Ana founds the vault.
    for (const member of members)
      await expect(member.page.getByTestId("roster").locator("li")).toHaveCount(3);
    await ana.shot("lobby");
    await bea.shot("lobby");
    await ana.page.getByTestId("lobby-start").click();

    // Guests review the plan and make their key share.
    for (const guest of [bea, cai]) {
      await guest.page.getByTestId("review-begin").click({ timeout: 60_000 });
    }
    await ana.page.waitForTimeout(1500);
    await ana.shot("ceremony");

    // Every member's device completes the ceremony, sees the deployment and verifies the config.
    for (const member of members) {
      await expect(member.page.getByTestId("ceremony-title")).toHaveText(
        "Founders vault is ready",
        { timeout: 8 * 60_000 },
      );
    }
    await ana.shot("ceremony-done");
    const addresses: string[] = [];
    for (const member of members) {
      await member.page.getByTestId("ceremony-open").click();
      await expect(member.page.getByTestId("vault-title")).toHaveText("Founders vault");
      addresses.push(decodeURIComponent(member.page.url()).match(/0x[0-9a-f]{64}/u)?.[0] ?? "");
      await expect(member.page.getByText("Rules match on chain")).toBeVisible({ timeout: 120_000 });
    }
    expect(new Set(addresses).size).toBe(1);
    expect(addresses[0]).toMatch(/^0x[0-9a-f]{64}$/u);
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
