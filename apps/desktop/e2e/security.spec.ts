// The renderer is sandboxed: no Node, no Electron, and the bridge refuses anything outside the IPC contract.
import { expect, test } from "@playwright/test";

import { launch } from "./app.ts";

test("the renderer cannot reach beyond the IPC contract", async () => {
  const instance = await launch("Sec");
  try {
    const { page } = instance;
    await page.getByTestId("welcome-start").waitFor();
    const probe = await page.evaluate(async () => {
      const g = globalThis as Record<string, unknown>;
      const bridge = (
        window as unknown as {
          tecpan: {
            invoke(m: string, p?: unknown): Promise<{ ok: boolean; error?: { message: string } }>;
          };
        }
      ).tecpan;
      return {
        require: typeof g.require,
        process: typeof g.process,
        bridgeKeys: Object.keys(bridge).toSorted(),
        unknown: await bridge.invoke("shell.openExternal", { url: "https://example.com" }),
        badParams: await bridge.invoke("profile.create", {
          name: "x",
          nodeUrl: "file:///etc/passwd",
        }),
        protoMethod: await bridge.invoke("__proto__"),
        secure: window.isSecureContext,
        origin: location.origin,
      };
    });
    expect(probe.require).toBe("undefined");
    expect(probe.process).toBe("undefined");
    expect(probe.bridgeKeys).toEqual(["invoke", "onEvent", "platform"]);
    expect(probe.unknown).toEqual({ ok: false, error: { message: "Unknown method" } });
    expect(probe.badParams.ok).toBe(false);
    expect(probe.protoMethod).toEqual({ ok: false, error: { message: "Unknown method" } });
    expect(probe.secure).toBe(true);
    expect(probe.origin).toBe("app://tecpan");

    // Navigation away from the app and new windows are refused.
    await page.evaluate(() => {
      window.open("https://example.com");
      location.href = "https://example.com";
    });
    await page.waitForTimeout(500);
    expect(page.url()).toMatch(/^app:\/\/tecpan\//u);
    expect(instance.app.windows()).toHaveLength(1);
  } finally {
    await instance.close();
  }
});
