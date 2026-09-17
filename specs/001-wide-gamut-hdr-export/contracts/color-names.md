# Contract: Colour Space and Colour Type Names

## Scope

This contract covers parsing and reporting of `colorSpace` and `colorType`
strings at every JS entry point: `new Canvas`, export options, `ImageData`,
`getImageData`, and the `colorSpace` getters.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| N1: unknown `colorSpace` (`rec2020-pqq`) throws `TypeError` naming the value and the accepted names, at canvas creation and at export | Covered | `to_color_space`, `opt_color_space_for_key`, `color_space_name` (`src/node/utils.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| N2: every canonical name round-trips through a canvas `colorSpace` getter | Covered | `from_color_space` (`src/node/utils.rs`), `get_color_space` (`src/node/canvas.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| N3: every alias is accepted and reads back canonical | Covered | `to_color_space`, `from_color_space` (`src/node/utils.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| N4: unknown `colorType` throws `TypeError` at canvas creation and at export | Covered | `to_color_type`, `opt_color_type_for_key` (`src/node/utils.rs`), `get_color_type` (`src/node/canvas.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |

## Invariants

- No name maps to a fallback space or type.
- The canonical set has exactly eight names.

## Failure Modes

- `new_cicp` returns `None` for a valid name: `TypeError` naming the value.

## Required Evidence Before Marking Complete

- Source evidence: `to_color_space`, `from_color_space`, `to_color_type` and
  their callers in `src/node/utils.rs`, `src/node/canvas.rs`,
  `src/node/image.rs`, `src/context/api.rs`.
- Executable evidence: the named tests, green in the `test.yml` CI run.
