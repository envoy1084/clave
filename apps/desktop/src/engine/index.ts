import type { Results } from "@shared/contract.ts";
// Engine entry point, forked by the main process as an Electron `utilityProcess`. Receives its init (including the
// member's account secrets) once, then serves calls; it never touches the approval signing key.
import type { EngineToMain, HostMethod, MainToEngine } from "@shared/engine-protocol.ts";

import { Engine } from "./engine.ts";

const port = process.parentPort;
const post = (message: EngineToMain) => port.postMessage(message);

let engine: Engine | undefined;
let nextHostCall = 0;
const hostCalls = new Map<
  number,
  { resolve: (value: unknown) => void; reject: (error: Error) => void }
>();

function host(method: HostMethod, params: unknown): Promise<unknown> {
  const id = ++nextHostCall;
  return new Promise((resolve, reject) => {
    hostCalls.set(id, { resolve, reject });
    post({ kind: "host-call", id, method, params });
  });
}

async function initialize(message: Extract<MainToEngine, { kind: "init" }>): Promise<void> {
  engine = new Engine(message.init, {
    emit: (event) => post({ kind: "event", event }),
    saveVault: async (record) => {
      await host("vault.save", record);
    },
    adoptAccount: (account) => post({ kind: "ready", account }),
    call: host,
  });
  await engine.start();
  post({ kind: "ready" });
}

port.on("message", ({ data }: { data: MainToEngine }) => {
  switch (data.kind) {
    case "init":
      initialize(data).catch((error: unknown) => {
        process.stderr.write(
          `engine failed to start: ${error instanceof Error ? (error.stack ?? error.message) : String(error)}\n`,
        );
        process.exit(1);
      });
      break;
    case "call": {
      const current = engine;
      if (!current) {
        post({ kind: "reply", id: data.id, error: "The engine is still starting" });
        break;
      }
      current.call(data.method as keyof Results, data.params).then(
        (result) => post({ kind: "reply", id: data.id, result }),
        (error: unknown) =>
          post({
            kind: "reply",
            id: data.id,
            error: error instanceof Error ? error.message : String(error),
          }),
      );
      break;
    }
    case "host-reply": {
      const pending = hostCalls.get(data.id);
      hostCalls.delete(data.id);
      if (data.error === undefined) pending?.resolve(data.result);
      else pending?.reject(new Error(data.error));
      break;
    }
  }
});

process.on("unhandledRejection", (error) =>
  process.stderr.write(`engine: unhandled rejection: ${String(error)}\n`),
);
