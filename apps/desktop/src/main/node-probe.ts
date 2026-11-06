import type { NodeStatus } from "@shared/types.ts";
import { net } from "electron";

/** Asks an Aztec node who it is over JSON-RPC (used before the engine runs, e.g. while setting up a profile). */
export async function probeNode(url: string): Promise<NodeStatus> {
  const rpc = async <T>(method: string): Promise<T> => {
    const response = await net.fetch(url, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params: [] }),
      signal: AbortSignal.timeout(4000),
    });
    if (!response.ok) throw new Error(`The node answered ${response.status}`);
    const body = (await response.json()) as { result?: T; error?: { message: string } };
    if (body.error) throw new Error(body.error.message);
    return body.result as T;
  };
  try {
    const [info, block] = await Promise.all([
      rpc<{ nodeVersion: string; l1ChainId: number; rollupVersion: number | string }>(
        "node_getNodeInfo",
      ),
      rpc<number | string>("node_getBlockNumber"),
    ]);
    return {
      url,
      reachable: true,
      nodeVersion: info.nodeVersion,
      l1ChainId: info.l1ChainId,
      rollupVersion: Number(info.rollupVersion),
      blockNumber: Number(block),
      checkedAt: Date.now(),
    };
  } catch (error) {
    return {
      url,
      reachable: false,
      checkedAt: Date.now(),
      error: error instanceof Error && error.name !== "TimeoutError" ? error.message : "No answer",
    };
  }
}
