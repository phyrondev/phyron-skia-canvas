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
| W5: `drawImage(canvasA)` into `RGBAF32` canvas B keeps HDR `2.0` | Open | None | None | `tests/suite/color.test.js` "drawImage of a canvas keeps HDR values" |
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
