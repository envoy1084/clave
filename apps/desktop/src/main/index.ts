import { join } from "node:path";

import { type IpcError, parseCall } from "@shared/contract.ts";
import type { AppEvent } from "@shared/types.ts";
import { BrowserWindow, app, dialog, ipcMain, systemPreferences } from "electron";

import { Controller } from "./controller.ts";
import { selectUserData } from "./paths.ts";
import type { ApprovalPrompt } from "./signer.ts";
import { createWindow, hardenSession, isAppUrl, registerAppScheme } from "./window.ts";

// The app name keys the OS keychain item `safeStorage` uses ("<name> Safe Storage"). Development and release builds
// are signed differently, so they get separate items instead of fighting over one (macOS prompts on every switch).
app.setName(app.isPackaged ? "Tecpan" : "Tecpan Dev");
selectUserData();
registerAppScheme();
app.enableSandbox();

// One app instance per profile: a second launch focuses the existing window instead of racing for the same keys.
if (!app.requestSingleInstanceLock()) app.quit();

let window: BrowserWindow | undefined;

const emit = (event: AppEvent) => {
  if (window && !window.isDestroyed()) window.webContents.send("tecpan:event", event);
};

/** Native confirmation before every signature; Touch ID too where the Mac has it. */
async function confirm(prompt: ApprovalPrompt): Promise<boolean> {
  const owner = window && !window.isDestroyed() ? window : undefined;
  const options = {
    type: "question" as const,
    buttons: ["Approve", "Cancel"],
    defaultId: 1,
    cancelId: 1,
    title: "Approve with your key",
    message: `Approve for ${prompt.vaultName}?`,
    detail: `${prompt.summary}\n\nApproval message ${prompt.message}`,
  };
  const { response } = owner
    ? await dialog.showMessageBox(owner, options)
    : await dialog.showMessageBox(options);
  if (response !== 0) return false;
  if (process.platform === "darwin" && systemPreferences.canPromptTouchID()) {
    try {
      await systemPreferences.promptTouchID(`approve a payment from ${prompt.vaultName}`);
    } catch {
      return false;
    }
  }
  return true;
}

const controller = new Controller(emit, confirm);

ipcMain.handle("tecpan:invoke", async (event, method: unknown, params: unknown) => {
  // Only our own renderer may call in; frames from anywhere else (there should be none) are refused.
  if (!event.senderFrame || !isAppUrl(event.senderFrame.url))
    return { ok: false, error: { message: "Forbidden" } };
  try {
    const call = parseCall(method, params);
    return { ok: true, result: await controller.handle(call.method, call.params as never) };
  } catch (error) {
    return {
      ok: false,
      error: { message: error instanceof Error ? error.message : String(error) } satisfies IpcError,
    };
  }
});

app.on("second-instance", () => {
  if (window) {
    if (window.isMinimized()) window.restore();
    window.focus();
  }
});

app.on("web-contents-created", (_event, contents) => {
  contents.on("will-attach-webview", (e) => e.preventDefault());
  contents.setWindowOpenHandler(() => ({ action: "deny" }));
});

app.whenReady().then(
  async () => {
    hardenSession();
    window = createWindow(join(import.meta.dirname, "../preload/index.cjs"));
    window.on("closed", () => (window = undefined));
    try {
      await controller.start();
    } catch (error) {
      dialog.showErrorBox(
        "Tecpan could not start",
        error instanceof Error ? error.message : String(error),
      );
    }
    emit({ type: "state", state: controller.state() });
    app.on("activate", () => {
      if (BrowserWindow.getAllWindows().length === 0)
        window = createWindow(join(import.meta.dirname, "../preload/index.cjs"));
    });
    return undefined;
  },
  (error: unknown) => dialog.showErrorBox("Tecpan could not start", String(error)),
);

let quitting = false;
app.on("before-quit", (event) => {
  if (quitting) return;
  event.preventDefault();
  quitting = true;
  void controller.stop().finally(() => app.quit());
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
