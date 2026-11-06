// Shape of `window.tecpan`, exposed by the preload script.
interface TecpanBridge {
  invoke(
    method: string,
    params?: unknown,
  ): Promise<{ ok: true; result: unknown } | { ok: false; error: { message: string } }>;
  onEvent(listener: (event: unknown) => void): () => void;
  readonly platform: string;
}

interface Window {
  readonly tecpan: TecpanBridge;
}
