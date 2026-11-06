import { readFile } from "node:fs/promises";
import { extname, join, normalize, sep } from "node:path";

import { BrowserWindow, app, protocol, session } from "electron";

export const APP_SCHEME = "app";
const APP_ORIGIN = `${APP_SCHEME}://tecpan`;
const RENDERER_DEV_URL = process.env.ELECTRON_RENDERER_URL;

/** Production policy: everything from the app itself, nothing remote, no eval, no inline scripts. */
const CSP = [
  "default-src 'none'",
  "script-src 'self'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "font-src 'self'",
  "connect-src 'self'",
  "base-uri 'none'",
  "form-action 'none'",
  "frame-ancestors 'none'",
  "object-src 'none'",
].join("; ");
/** Development adds what Vite's dev server and React refresh need, still nothing beyond localhost. */
const DEV_CSP =
  "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; connect-src 'self' ws://localhost:* http://localhost:*; object-src 'none'; base-uri 'none'";

const MIME: Record<string, string> = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".webp": "image/webp",
  ".woff2": "font/woff2",
  ".json": "application/json",
};

/** Must run before `app.ready`: the renderer's own origin, so it is a secure context with a stable origin. */
export function registerAppScheme(): void {
  protocol.registerSchemesAsPrivileged([
    { scheme: APP_SCHEME, privileges: { standard: true, secure: true, supportFetchAPI: true } },
  ]);
}

/** Serves the built renderer from `app://tecpan/`, refusing paths outside it, with the CSP on every response. */
function serveRenderer(): void {
  const root = join(import.meta.dirname, "../renderer");
  protocol.handle(APP_SCHEME, async (request) => {
    const url = new URL(request.url);
    const relative = decodeURIComponent(url.pathname === "/" ? "/index.html" : url.pathname);
    const path = normalize(join(root, relative));
    if (url.host !== "tecpan" || !path.startsWith(root + sep))
      return new Response("Not found", { status: 404 });
    try {
      const body = await readFile(path);
      return new Response(body, {
        headers: {
          "content-type": MIME[extname(path)] ?? "application/octet-stream",
          "content-security-policy": CSP,
          "x-content-type-options": "nosniff",
        },
      });
    } catch {
      return new Response("Not found", { status: 404 });
    }
  });
}

/** Session-wide defaults: deny every permission, refuse downloads, add the CSP in development. */
export function hardenSession(): void {
  const ses = session.defaultSession;
  ses.setPermissionRequestHandler((_wc, _permission, callback) => callback(false));
  ses.setPermissionCheckHandler(() => false);
  ses.on("will-download", (event) => event.preventDefault());
  if (RENDERER_DEV_URL) {
    ses.webRequest.onHeadersReceived((details, callback) =>
      callback({
        responseHeaders: { ...details.responseHeaders, "Content-Security-Policy": [DEV_CSP] },
      }),
    );
  } else {
    serveRenderer();
  }
}

/** Whether `url` is our renderer (the only origin allowed to call the IPC API). */
export function isAppUrl(url: string): boolean {
  if (RENDERER_DEV_URL) return url.startsWith(RENDERER_DEV_URL);
  return url.startsWith(`${APP_ORIGIN}/`);
}

export function createWindow(preload: string): BrowserWindow {
  const window = new BrowserWindow({
    width: 1280,
    height: 820,
    minWidth: 960,
    minHeight: 640,
    show: false,
    title: "Tecpan",
    backgroundColor: "#FBF8F2",
    titleBarStyle: process.platform === "darwin" ? "hiddenInset" : "default",
    trafficLightPosition: { x: 18, y: 18 },
    webPreferences: {
      preload,
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
      nodeIntegrationInWorker: false,
      webviewTag: false,
      spellcheck: false,
      devTools: !app.isPackaged,
    },
  });
  const contents = window.webContents;
  // The app never opens windows, and nothing in it links out yet; a link-out allowlist comes with the first need.
  contents.setWindowOpenHandler(() => ({ action: "deny" }));
  contents.on("will-navigate", (event, url) => {
    if (!isAppUrl(url)) event.preventDefault();
  });
  contents.on("will-attach-webview", (event) => event.preventDefault());
  window.once("ready-to-show", () => window.show());
  void window.loadURL(RENDERER_DEV_URL ?? `${APP_ORIGIN}/index.html`);
  return window;
}
