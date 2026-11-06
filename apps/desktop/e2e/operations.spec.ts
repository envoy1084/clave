// Phase 11 exit criterion: a private transfer is proposed on member A, approved on B while C is offline, and executed
// on C after it reconnects and catches up; every member's balance reflects it. UI only, three profiles.
/* oxlint-disable no-await-in-loop -- members act one after another, as people would. */
import { expect, test } from "@playwright/test";

import { type Instance, foundVault, launch, networkAvailable, onboard } from "./app.ts";

const available = await networkAvailable();

test.skip(!available, "needs `aztec start --local-network`");

const balance = (m: Instance) => m.page.getByTestId("balance-TTT");
/** In-app navigation (the router uses hash history on the app:// origin). */
const go = (m: Instance, path: string) =>
  m.page.evaluate((hash) => {
    location.hash = hash;
  }, path);

test("propose on A, approve on B, C catches up offline and executes", async () => {
  const members: Instance[] = [];
  try {
    members.push(...(await Promise.all(["Ana", "Bea", "Cai"].map((n) => launch(n)))));
    await Promise.all(members.map(onboard));
    const [ana, bea] = members as [Instance, Instance, Instance];
    let cai = members[2] as Instance;
    const vault = await foundVault(members, "Ops");

    // Ana deploys a test token and mints 10,000 TTT privately to the vault.
    await ana.page.getByTestId("deploy-test-token").click();
    await expect(balance(ana)).toHaveText("10,000", { timeout: 180_000 });
    await ana.shot("balances");

    // Ana proposes 25 TTT to Bea's own account.
    const beaAccount = await bea.page.evaluate(async () => {
      const reply = (await window.tecpan.invoke("app.state")) as {
        ok: true;
        result: { profile: { account: string } };
      };
      return reply.result.profile.account;
    });
    await ana.page.getByTestId("new-proposal").click();
    await ana.page.getByTestId("proposal-to").fill(beaAccount);
    await ana.page.getByTestId("proposal-amount").fill("25");
    await ana.shot("proposal-new");
    await ana.page.getByTestId("proposal-create").click();
    await expect(ana.page.getByTestId("proposal-title")).toHaveText(/^Transfer 25 TTT to /u, {
      timeout: 120_000,
    });
    const proposal =
      decodeURIComponent(ana.page.url()).match(/proposals\/(0x[0-9a-f]{64})/u)?.[1] ?? "";
    expect(proposal).toMatch(/^0x[0-9a-f]{64}$/u);
    await ana.page.getByTestId("proposal-approve").click();
    await expect(ana.page.getByTestId("approved-1")).toBeVisible({ timeout: 120_000 });
    await ana.shot("proposal-approved");

    // Cai goes offline before Bea approves.
    const caiDir = cai.dir;
    await cai.close(true);

    // Bea sees the replicated proposal, checks it and approves: the threshold is reached.
    await bea.page.getByRole("link", { name: "Proposals", exact: true }).click();
    await bea.page
      .getByTestId("proposals")
      .getByText("transfer 25 TTT")
      .click({ timeout: 120_000 });
    await bea.page.getByTestId("proposal-approve").click();
    await expect(bea.page.getByTestId("approved-2")).toBeVisible({ timeout: 120_000 });
    await expect(bea.page.getByText("Ready", { exact: true })).toBeVisible({ timeout: 60_000 });
    await bea.shot("proposal-ready");

    // Cai restarts on the same profile, catches up through sync or its relay mailbox, and executes.
    cai = await launch("Cai", caiDir);
    members[2] = cai;
    await cai.page.getByTestId("vault-title").waitFor({ timeout: 120_000 });
    await go(cai, `/vault/${vault}/proposals/${proposal}`);
    await expect(cai.page.getByTestId("approved-2")).toBeVisible({ timeout: 180_000 });
    await cai.page.getByTestId("proposal-execute").click();
    await expect(cai.page.getByText("Executed", { exact: true }).first()).toBeVisible({
      timeout: 300_000,
    });
    await cai.shot("proposal-executed");

    // Every member: the proposal is executed and the vault holds 9,975 TTT.
    for (const member of [ana, bea, cai]) {
      await go(member, `/vault/${vault}`);
      await expect(balance(member)).toHaveText("9,975", { timeout: 180_000 });
    }
    await ana.shot("balances-after");
    await ana.page.getByRole("link", { name: "Activity", exact: true }).click();
    await expect(ana.page.getByTestId("activity").getByText("Executed")).toBeVisible({
      timeout: 60_000,
    });
    await ana.shot("activity");
  } finally {
    await Promise.all(members.map((m) => m.close()));
  }
});
