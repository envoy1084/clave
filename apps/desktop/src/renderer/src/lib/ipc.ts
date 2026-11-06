// Typed access to the main process. Every call goes through the preload bridge and is validated there against the
// IPC contract; errors come back as rejections with a readable message.
import type { Method, Params, Results } from "@shared/contract.ts";
import type { AppEvent } from "@shared/types.ts";

export class IpcError extends Error {}

export async function call<M extends Method>(
  method: M,
  ...params: Params<M> extends undefined ? [] : [Params<M>]
): Promise<Results[M]> {
  const reply = await window.tecpan.invoke(method, params[0]);
  if (!reply.ok) throw new IpcError(reply.error.message);
  return reply.result as Results[M];
}

export function onEvent(listener: (event: AppEvent) => void): () => void {
  return window.tecpan.onEvent((event) => listener(event as AppEvent));
}
