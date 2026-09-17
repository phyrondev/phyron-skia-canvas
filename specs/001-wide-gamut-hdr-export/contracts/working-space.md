# Contract: Working Space Compositing

## Scope

This contract covers where compositing happens for `toBuffer`, `toFile`,
`getImageData` and `drawImage(canvas)`, and the validity of the page cache and
the recorder surface. It does not cover the output encoding (see
`raw-export.md`).

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| W1: 50% white over black on a `RGBAF32` `srgb-linear` canvas decodes to linear 0.5 for outputs `srgb-linear`, `srgb`, `rec2020-linear`, `rec2020-pq` | Open | None | None | `tests/suite/color.test.js` "composites in the working space" |
| W2: `[0,0,0,0.5]` over `[2,2,2,1]` exports `1.0` in `srgb-linear` (no clip before blend) | Open | None | None | `tests/suite/color.test.js` "does not clip HDR before blending" |
| W3: two exports of one unchanged canvas with different `colorSpace` or `colorType` equal the fresh-canvas results | Open | None | None | `tests/suite/color.test.js` "repeated exports ignore stale caches" |
| W4: `getImageData` uses the working space for compositing | Open | None | None | `tests/suite/color.test.js` "getImageData composites in the working space" |
| W5: `drawImage(canvasA)` into `RGBAF32` and `RGBAF16` canvas B keeps linear 0.002, 0.2001, 0.2003, 1.5 within `1e-4` (F16: its step) | Open | None | None | `tests/suite/color.test.js` "drawImage of a canvas keeps precision and range" |
| W7: `drawImage(canvasA)` keeps Rec.2020 green into `rec2020-linear` and `srgb-linear` B | Open | None | None | `tests/suite/color.test.js` "drawImage of a canvas keeps gamut" |
| W8: `drawImage(canvasA)` with `globalAlpha = 0.5` halves alpha; `RGBA8888` B has 8-bit values | Open | None | None | `tests/suite/color.test.js` "drawImage of a canvas applies globalAlpha" |
| W9: float `getImageData` then `putImageData` round-trips within `1e-5` in `srgb-linear` and `rec2020-linear` | Open | None | None | `tests/suite/color.test.js` "float ImageData round trip" |
| W10: 1920x1080 `drawImage(canvas)` time before and after is in the PR | Open | None | None | timing script output in the PR description |
| W11: 8-bit `srgb` output of a `RGBAF32` `srgb-linear` canvas blends linearly (188, not 128) for raw `RGBA8888`, PNG, and `getImageData` after `drawImage` into an `RGBA8888` canvas | Open | None | None | `tests/suite/color.test.js` "8-bit sRGB output of a float linear canvas blends linearly" |
| W12: `drawImage(canvas)` with `lighter`, `multiply`, `destination-in` and a shadow equals the same drawing from a float `ImageData` within `1e-3`; with `blur(2px)` the unpremultiplied colour stays the source colour within `1e-5` | Open | None | None | `tests/suite/color.test.js` "drawImage of a canvas composites like a plain image source" |
| W13: `drawImage` from a GPU `RGBAF32` canvas into a CPU `RGBAF32` canvas keeps Rec.2020 green and alpha 0.5 | Open | None | None | `tests/suite/color.test.js` "drawImage from a GPU canvas into a CPU float canvas keeps extended values" (skipped without GPU) |
| W6: export without colour options returns `RGBA8888` `srgb` | Open | None | Existing `tests/suite/canvas.test.js` raw export tests | `just test` green in CI |

## Invariants

- The output encoding never changes the blend result.
- `PageCache` and the `PageRecorder` surface are reused only for identical
  density, matte, msaa, working colour type and working colour space.

## Failure Modes

- The working surface cannot be allocated: the export rejects with
  `Could not allocate new WxH bitmap (color type: T)`.

## Required Evidence Before Marking Complete

- Source evidence: `Page::encoded_as`, `PageRecorder::get_pixels`,
  `PageRecorder::get_image`, `PageCache::is_valid`,
  `PageRecorder::is_config_stale` in `src/context/page.rs`.
- Executable evidence: `tests/suite/color.test.js` tests named in the matrix,
  green in the `test.yml` CI run on the branch (run URL in the row).
