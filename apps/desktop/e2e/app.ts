// Launches isolated app instances (own data dir, keeper, engine, keychain entries) for end-to-end tests.
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { type ElectronApplication, type Page, _electron as electron } from "@playwright/test";

export const NODE_URL = process.env.AZTEC_NODE_URL ?? "http://localhost:8080";
const SHOTS = process.env.TECPAN_SHOTS;

export interface Instance {
  readonly name: string;
  readonly app: ElectronApplication;
  readonly page: Page;
  readonly dir: string;
  shot(label: string): Promise<void>;
  /** Quits the app; `keep` leaves its profile on disk for a restart. */
  close(keep?: boolean): Promise<void>;
}

export async function networkAvailable(): Promise<boolean> {
  try {
    const response = await fetch(NODE_URL, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "node_getBlockNumber", params: [] }),
      signal: AbortSignal.timeout(3000),
    });
    return response.ok;
  } catch {
    return false;
  }
}

/** Starts an instance; pass `dir` to restart a member on its existing profile (keys, keeper state, PXE). */
export async function launch(name: string, existing?: string): Promise<Instance> {
  const dir = existing ?? (await mkdtemp(join(tmpdir(), `tecpan-e2e-${name.toLowerCase()}-`)));
  const app = await electron.launch({
    args: ["."],
    cwd: join(import.meta.dirname, ".."),
    env: {
      ...process.env,
      TECPAN_USER_DATA: dir,
      RUST_LOG: process.env.RUST_LOG ?? "warn",
      // Native approval dialogs cannot be clicked from tests; development builds honour this, packaged ones never do.
      TECPAN_TEST_AUTO_APPROVE: "1",
    },
  });
  const page = await app.firstWindow();
  await page.setViewportSize({ width: 1280, height: 820 });
  return {
    name,
    app,
    page,
    dir,
    shot: async (label) => {
      if (!SHOTS) return;
      await page.waitForTimeout(700); // let entrances settle
      await page.screenshot({ path: join(SHOTS, `${name.toLowerCase()}-${label}.png`) });
    },
    close: async (keep = false) => {
      await app.close().catch(() => undefined);
      if (!keep) await rm(dir, { recursive: true, force: true });
    },
  };
}

/** First-run onboarding: name, local network, keys. Ends on the start screen. */
export async function onboard(instance: Instance): Promise<void> {
  const { page } = instance;
  await page.getByTestId("welcome-start").click();
  await page.getByTestId("profile-name").fill(instance.name);
  await page.getByTestId("node-online").waitFor();
  await page.getByTestId("profile-create").click();
  await page
    .getByTestId("setup-continue")
    .and(page.locator(":enabled"))
    .waitFor({ timeout: 90_000 });
  await instance.shot("setup-done");
  await page.getByTestId("setup-continue").click();
  await page.getByTestId("start-new").waitFor();
}

/**
 * The founding flow through the UI: the first member opens a lobby, the others join with its invite, everyone runs the
 * DKG, the host deploys and every member verifies. Returns the vault address.
 */
export async function foundVault(members: readonly Instance[], name: string): Promise<string> {
  const [host, ...guests] = members;
  if (!host) throw new Error("No members");
  await host.page.getByTestId("start-new").click();
  await host.page.getByTestId("vault-name").fill(name);
  await host.shot("create");
  await host.page.getByTestId("vault-create").click();
  const invite = (await host.page.getByTestId("invite-code").textContent())?.trim() ?? "";
  for (const guest of guests) {
    // oxlint-disable-next-line no-await-in-loop
    await guest.page.getByTestId("start-join").click();
    // oxlint-disable-next-line no-await-in-loop
    await guest.page.getByTestId("invite-input").fill(invite);
    // oxlint-disable-next-line no-await-in-loop
    await guest.page.getByTestId("invite-name").getByText(name).waitFor();
    // oxlint-disable-next-line no-await-in-loop
    await guest.shot("invite-preview");
    // oxlint-disable-next-line no-await-in-loop
    await guest.page.getByTestId("invite-join").click();
    // oxlint-disable-next-line no-await-in-loop
    await guest.page.getByTestId("lobby-name").waitFor();
  }
  await host.page
    .getByTestId("roster")
    .locator("li")
    .nth(members.length - 1)
    .waitFor();
  await host.shot("lobby");
  await host.page.getByTestId("lobby-start").click();
  await Promise.all(
    guests.map((g) => g.page.getByTestId("review-begin").click({ timeout: 60_000 })),
  );
  await host.page.waitForTimeout(1200);
  await host.shot("ceremony");
  const addresses = await Promise.all(
    members.map(async (member) => {
      await member.page
        .getByTestId("ceremony-title")
        .getByText(`${name} deployed`)
        .waitFor({ timeout: 8 * 60_000 });
      await member.shot("ceremony-done");
      await member.page.getByTestId("ceremony-open").click();
      await member.page.getByTestId("vault-title").getByText(name).waitFor();
      return decodeURIComponent(member.page.url()).match(/0x[0-9a-f]{64}/u)?.[0] ?? "";
    }),
  );
  if (new Set(addresses).size !== 1 || !addresses[0])
    throw new Error(`Members disagree on the vault: ${addresses.join(", ")}`);
  return addresses[0];
}
