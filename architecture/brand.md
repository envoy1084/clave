# Brand

Clave sits in the Aztec ecosystem and looks like it belongs there: warm paper and olive ink, a light editorial serif,
nature imagery gradient-mapped and partly resolved into pixels (privacy, rendered). The tokens are close siblings of
Aztec's own, not copies.

## Mark

A threshold: an arch in two halves standing on a sill. The halves meet but never join, so the doorway exists only when
both sides are there. Drawn on a 40-unit grid (`apps/web/src/components/brand/mark.tsx`):

```text
left half   M7 31V20A13 13 0 0 1 18 7.15V13.82A6.5 6.5 0 0 0 13.5 20V31Z
right half  M33 31V20A13 13 0 0 0 22 7.15V13.82A6.5 6.5 0 0 1 26.5 20V31Z
sill        rect x=4 y=33.5 w=32 h=4
```

Use it in ink on paper, signal on ink, or ink on signal. The site icon (`apps/web/public/icon.svg`) is the mark
in signal on an ink rounded square.

## Colour

| Token      | Value     | Use                                                                    |
| ---------- | --------- | ---------------------------------------------------------------------- |
| Paper      | `#FBF8F2` | Background                                                             |
| Paper 2    | `#F2EEE5` | Quiet surfaces, segmented controls                                     |
| Ink        | `#15180F` | Text, primary buttons, dark bands                                      |
| Signal     | `#C8F500` | One thing per screen (the main action on ink, live status); never text |
| Olive      | `#4F6300` | Actions and success on paper (signal fails contrast as text)           |
| Periwinkle | `#A7A9FF` | Viewing and the committee                                              |
| Coral      | `#C4432A` | Errors and danger                                                      |

Light theme only.

## Type

Free stand-ins for Aztec's licensed pairing (ABC Arizona Serif, ABC Oracle), bundled with the site:

- **Newsreader** (light, optical sizes): headings; its italic carries the key phrase of a heading.
- **Geist**: interface and body.
- **Geist Mono**: addresses, keys and hashes only.

## Imagery

`Terrain` generates the landscape: fractal noise, contrast-stretched, gradient-mapped through deep blue, cyan and
lilac to a rare signal-green glint, with clusters of pixels that slowly resolve and dissolve (off for reduced motion).
Mosaic clusters are placed within the visible band of each panel.

## Figures

Mechanisms are explained with isometric figures, each a physical object whose working is the idea: a lock box for the
k-of-n auth witness, keeper lamps lighting a note on the PXE lens for threshold ECDH, a rack of trays for where each
component runs, and the same table re-keyed for a membership rotation. They are drawn with the anatomy isometric kit
(vendored in `apps/web/src/components/iso/`) in its light theme mapped to paper and ink; signal marks the one part to
watch and periwinkle the PXE's lens. Parts only move by transform and opacity, sequences play once (with a replay), and
every figure is still under reduced motion.
