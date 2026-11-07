# Brand

## Purpose

How the Clave site (`apps/web`) looks. Clave sits in the Aztec ecosystem and looks like it belongs there: warm paper
and olive ink, a light editorial serif, nature imagery gradient-mapped and partly resolved into pixels (privacy,
rendered). The tokens are close siblings of Aztec's own, not copies.

## Mark

A threshold: an arch in two halves standing on a sill. The halves meet but never join, so the doorway exists only when
both sides are there. Drawn on a 40-unit grid (`apps/web/src/components/brand/mark.tsx`):

```text
left half   M7 31V20A13 13 0 0 1 18 7.15V13.82A6.5 6.5 0 0 0 13.5 20V31Z
right half  M33 31V20A13 13 0 0 0 22 7.15V13.82A6.5 6.5 0 0 1 26.5 20V31Z
sill        rect x=4 y=33.5 w=32 h=4
```

Use it in ink on paper, signal on ink, or ink on signal. The site icon (`apps/web/public/icon.svg`) is the mark in
signal on a dark rounded square.

## Colour

Tokens are defined in `apps/web/src/styles.css`.

| Token      | Value     | Use                                                                    |
| ---------- | --------- | ---------------------------------------------------------------------- |
| Paper      | `#FBF8F2` | Background                                                             |
| Paper 2    | `#F2EEE5` | Quiet surfaces, segmented controls                                     |
| Ink        | `#15180F` | Text, primary buttons                                                  |
| Navy       | `#0E1530` | Dark chapters: the home hero, the navigation bar, dark bands           |
| Signal     | `#C8F500` | One thing per screen (the main action on ink, live status); never text |
| Olive      | `#4F6300` | Actions and success on paper (signal fails contrast as text)           |
| Periwinkle | `#A7A9FF` | Viewing, shares and the committee                                      |
| Coral      | `#C4432A` | Errors and danger                                                      |

Light pages only; there is no dark theme. Dark sections are navy chapters inside light pages.

## Type

Free stand-ins for Aztec's licensed pairing (ABC Arizona Serif, ABC Oracle), bundled with the site:

- **Newsreader** (light, optical sizes): headings; its italic carries the key phrase of a heading.
- **Geist**: interface and body.
- **Geist Mono**: addresses, keys and hashes only.

## Imagery

`Terrain` (`apps/web/src/components/brand/terrain.tsx`) generates the landscape behind the home hero and the footer:
fractal noise, contrast-stretched, gradient-mapped through deep blue, cyan and lilac to a rare signal-green glint, with
clusters of pixels that slowly resolve and dissolve (off for reduced motion). Mosaic clusters are placed within the
visible band of each panel.

## Figures

Mechanisms are explained with flat illustrations (`apps/web/src/figures/simple/`): single-idea drawings in ink, paper,
signal and periwinkle (signal marks the moment that matters, periwinkle the shares) for spending, approval, viewing,
combining partials and membership. Each plays its sequence once when it scrolls into view and rests on the final frame;
replay runs it again, and under reduced motion the final frame shows at once.

One isometric figure remains: the rack of trays in the home page's system tour (`apps/web/src/figures/rack/`), showing
where each component runs. It is drawn with the anatomy isometric kit (vendored in `apps/web/src/components/iso/`) in
its light theme mapped to paper and ink. The earlier isometric lock box and keeper-lamp figures are no longer used.
Parts only move by transform and opacity.
