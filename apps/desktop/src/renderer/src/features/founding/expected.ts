// How many members the host expects in a lobby: a per-device convenience that sets the "waiting for N more" line. The
// keeper only knows who has joined.
const key = (lobby: string) => `tecpan.expected.${lobby.toLowerCase()}`;

export function rememberExpected(lobby: string, members: number): void {
  try {
    localStorage.setItem(key(lobby), String(members));
  } catch {
    // Storage can be unavailable; the lobby still works, it just can't say how many are missing.
  }
}

export function expectedMembers(lobby: string): number | undefined {
  try {
    const value = Number(localStorage.getItem(key(lobby)));
    return Number.isInteger(value) && value >= 2 ? value : undefined;
  } catch {
    return undefined;
  }
}
