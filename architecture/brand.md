# Brand

Tecpan sits in the Aztec ecosystem and looks like it belongs there: warm paper and olive ink, a light editorial serif,
nature imagery gradient-mapped and partly resolved into pixels (privacy, rendered). The tokens are close siblings of
Aztec's own, not copies.

## Mark

A threshold: an arch in two halves standing on a sill. The halves meet but never join, so the doorway exists only when
both sides are there. Drawn on a 40-unit grid (`apps/desktop/src/renderer/src/components/brand/mark.tsx`):

```text
left half   M7 31V20A13 13 0 0 1 18 7.15V13.82A6.5 6.5 0 0 0 13.5 20V31Z
right half  M33 31V20A13 13 0 0 0 22 7.15V13.82A6.5 6.5 0 0 1 26.5 20V31Z
sill        rect x=4 y=33.5 w=32 h=4
```

Use it in ink on paper, signal on ink, or ink on signal. The app icon (`apps/desktop/resources/icon.svg`) is the mark
in signal on an ink rounded square on the macOS grid; PNG and ICNS are rasterised from it with Chromium.

## Colour

| Token      | Value     | Use                                                                    |
| ---------- | --------- | ---------------------------------------------------------------------- |
| Paper      | `#FBF8F2` | Background                                                             |
| Paper 2    | `#F2EEE5` | Quiet surfaces, segmented controls                                     |
| Ink        | `#15180F` | Text, primary buttons, the sidebar                                     |
| Signal     | `#C8F500` | One thing per screen (the main action on ink, live status); never text |
| Olive      | `#4F6300` | Actions and success on paper (signal fails contrast as text)           |
| Periwinkle | `#A7A9FF` | Viewing and the committee                                              |
| Coral      | `#C4432A` | Errors and danger                                                      |

Light theme only.

## Type

Free stand-ins for Aztec's licensed pairing (ABC Arizona Serif, ABC Oracle), bundled with the app:

- **Newsreader** (light, optical sizes): headings and balances.
- **Geist**: interface and body.
- **Geist Mono**: addresses, keys and hashes only.

## Imagery

`Terrain` generates the landscape: fractal noise, contrast-stretched, gradient-mapped through deep blue, cyan and
lilac to a rare signal-green glint, with clusters of pixels that slowly resolve and dissolve (off for reduced motion).
Mosaic clusters are placed within the visible band of each panel.
