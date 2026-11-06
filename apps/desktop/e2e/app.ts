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
  close(): Promise<void>;
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

export async function launch(name: string): Promise<Instance> {
  const dir = await mkdtemp(join(tmpdir(), `tecpan-e2e-${name.toLowerCase()}-`));
  const app = await electron.launch({
    args: ["."],
    cwd: join(import.meta.dirname, ".."),
    env: { ...process.env, TECPAN_USER_DATA: dir, RUST_LOG: process.env.RUST_LOG ?? "warn" },
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
    close: async () => {
      await app.close().catch(() => undefined);
      await rm(dir, { recursive: true, force: true });
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
