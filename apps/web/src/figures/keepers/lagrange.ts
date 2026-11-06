function gcd(a: number, b: number): number {
  return b === 0 ? Math.abs(a) : gcd(b, a % b);
}

/** λ_i^S at zero as a reduced fraction over the integers: Π_{j ∈ S, j ≠ i} j / (j − i). The field version is the same. */
export function lagrange(i: number, set: readonly number[]): string {
  let num = 1;
  let den = 1;
  for (const j of set) {
    if (j === i) continue;
    num *= j;
    den *= j - i;
  }
  const g = gcd(num, den);
  const [n, d] = den / g < 0 ? [-num / g, -den / g] : [num / g, den / g];
  return d === 1 ? String(n) : `${n}/${d}`;
}
