/**
 * Error kinds a client can act on; the CLI maps them to exit codes (architecture/cli.md):
 * `invalid` (2), `not-ready` (4), `locked`, `auth` and `failed` (1).
 */
export type ErrorKind = "invalid" | "not-ready" | "locked" | "auth" | "failed";

const CODES: Record<ErrorKind, number> = {
  invalid: -32602,
  "not-ready": 1001,
  locked: 1002,
  auth: 1003,
  failed: 1000,
};

export class DaemonError extends Error {
  readonly kind: ErrorKind;

  constructor(kind: ErrorKind, message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "DaemonError";
    this.kind = kind;
  }

  get code(): number {
    return CODES[this.kind];
  }
}

/** The JSON-RPC error for anything thrown while serving a call. */
export function rpcError(error: unknown): {
  code: number;
  message: string;
  data: { kind: ErrorKind };
} {
  const known = error instanceof DaemonError ? error : undefined;
  const kind = known?.kind ?? "failed";
  const message = error instanceof Error ? error.message : String(error);
  return { code: CODES[kind], message, data: { kind } };
}
