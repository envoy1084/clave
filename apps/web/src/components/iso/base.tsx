import { Dots, Line, Solid } from "./draw";
import type { BaseArt } from "./parts";

/** The base plate recipe drawn in painter's order: halo, feet back to front, slab, then what lies on its top. */
export function Base({ art }: { art: BaseArt }) {
  return (
    <g>
      <path className="iso-halo" d={art.halo} />
      {art.feet.map((parts, index) => (
        <g key={index}>
          {parts.map((paths, part) => (
            <Solid key={part} paths={paths} tone={part ? "mid" : "lo"} />
          ))}
        </g>
      ))}
      <Solid paths={art.slab} tone="mid" />
      <Line d={art.groove} tone="lo" />
      <Line d={art.screwRings} tone="lo" />
      <Dots points={art.screws} size={0.55} tone="mid" />
      <Solid paths={art.plate} tone="lo" crease="none" />
      <Line d={art.plateLine} tone="faint" />
      <Line d={art.plateRule} tone="lo" free />
      <Dots points={art.plateScrews} size={0.45} tone="mid" />
    </g>
  );
}
