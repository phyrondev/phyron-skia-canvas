# Contract: Working Space Compositing

## Scope

This contract covers where compositing happens for `toBuffer`, `toFile`,
`getImageData` and `drawImage(canvas)`, and the validity of the page cache and
the recorder surface. It does not cover the output encoding (see
`raw-export.md`).

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| W1: 50% white over black on a `RGBAF32` `srgb-linear` canvas decodes to linear 0.5 for outputs `srgb-linear`, `srgb`, `rec2020-linear`, `rec2020-pq` | Covered | `Page::encoded_as`, `RecordingSurface::update` (`src/context/page.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| W2: `[0,0,0,0.5]` over `[2,2,2,1]` exports `1.0` in `srgb-linear` (no clip before blend) | Covered | `Page::encoded_as` (`src/context/page.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| W3: two exports of one unchanged canvas with different `colorSpace` or `colorType` equal the fresh-canvas results | Covered | `PageCache::is_valid`, `RecordingSurface::is_config_stale`, `is_surface_stale` (`src/context/page.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| W4: `getImageData` uses the working space for compositing | Covered | `PageRecorder::get_pixels`, `RecordingSurface::copy_pixels` (`src/context/page.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| W5: `drawImage(canvasA)` into `RGBAF32` and `RGBAF16` canvas B keeps linear 0.002, 0.2001, 0.2003, 1.5 within `1e-4` (F16: its step) | Covered | `PageRecorder::get_image` (`src/context/page.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| W7: `drawImage(canvasA)` keeps Rec.2020 green into `rec2020-linear` and `srgb-linear` B | Covered | `PageRecorder::get_image` (`src/context/page.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| W8: `drawImage(canvasA)` with `globalAlpha = 0.5` halves alpha; `RGBA8888` B has 8-bit values | Covered | `PageRecorder::get_image` (`src/context/page.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| W9: float `getImageData` then `putImageData` round-trips within `1e-5` in `srgb-linear` and `rec2020-linear` | Covered | `PageRecorder::get_pixels` (`src/context/page.rs`), `Context2D::blit_pixels` (`src/context/mod.rs`), `ImageData` (`lib/classes/imagery.js`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| W10: 1920x1080 `drawImage(canvas)` time before and after is in the PR | Covered | `PageRecorder::get_image` (`src/context/page.rs`) | release builds, same host, median of 7, three batches: branch 81.0/89.5/84.0 ms, `3.6.0` 88.1/88.4/93.1 ms (research M10); in the PR description | None |
| W11: 8-bit `srgb` output of a `RGBAF32` `srgb-linear` canvas blends linearly (188, not 128) for raw `RGBA8888`, PNG, and `getImageData` after `drawImage` into an `RGBA8888` canvas | Covered | `read_output`, `Page::encoded_as` (`src/context/page.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24 | None |
| W12: `drawImage(canvas)` with `lighter`, `multiply`, `destination-in` and a shadow equals the same drawing from a float `ImageData` within `1e-3`; with `blur(2px)` the unpremultiplied colour stays the source colour within `1e-5` | Covered | `PageRecorder::get_image` (`src/context/page.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24 | None |
| W13: `drawImage` from a GPU `RGBAF32` canvas into a CPU `RGBAF32` canvas keeps Rec.2020 green and alpha 0.5 | Covered | `PageRecorder::get_image` (`src/context/page.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24 (ran, not skipped); added after the CI run | None |
| W6: export without colour options returns `RGBA8888` `srgb` | Covered | `Canvas::export_options` (`src/node/canvas.rs`), `ExportOptions::default` (`src/context/page.rs`) | local: `node --test tests/suite/canvas.test.js` 28/28 (run alone; parallel runs hit pre-existing crash M9); CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 (canvas raw tests pass, same 25 decode failures as `main`) | None |

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
