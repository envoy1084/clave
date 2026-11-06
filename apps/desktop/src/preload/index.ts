// The renderer's only door into the app: two functions, no Node, no Electron objects. Calls are validated again in
// the main process against the IPC contract; this file only forwards them.
import { contextBridge, ipcRenderer, type IpcRendererEvent } from "electron";

const api = {
  invoke: (method: string, params?: unknown): Promise<unknown> =>
    ipcRenderer.invoke("tecpan:invoke", method, params),
  onEvent: (listener: (event: unknown) => void): (() => void) => {
    const handler = (_event: IpcRendererEvent, payload: unknown) => listener(payload);
    ipcRenderer.on("tecpan:event", handler);
    return () => ipcRenderer.removeListener("tecpan:event", handler);
  },
  platform: process.platform,
};

contextBridge.exposeInMainWorld("tecpan", api);
