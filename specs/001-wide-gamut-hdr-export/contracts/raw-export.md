# Contract: Raw Export Encoding

## Scope

This contract covers the output encoding of `toBuffer("raw")` and
`getImageData`: colour space, colour type, `premultiplied` and
`hdrReferenceWhite`. It does not cover compositing (see `working-space.md`) or
PNG `cICP` chunks (non-goal).

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| E1: `RGBAF32` output in `rec2020-linear`, `srgb-linear`, `display-p3-linear`, `srgb` matches the spec table within `1e-3` | Covered | `read_output` (`src/context/page.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| E2: `R16G16B16A16UNorm` `rec2020-pq` matches ST 2084 at 203 nits within 2 code values | Covered | `read_output` (`src/context/page.rs`), `encode_pq` (`src/context/transfer/mod.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| E3: `R16G16B16A16UNorm` `rec2020-hlg` matches BT.2100 HLG (1000 nits, gamma 1.2) within 2 code values; white is 75% | Covered | `read_output` (`src/context/page.rs`), `encode_hlg` (`src/context/transfer/mod.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| E4: `hdrReferenceWhite: 100` maps linear 1.0 to the PQ code for 100 nits | Covered | `output_alpha_and_white_args` (`src/node/utils.rs`), `HdrTransfer::encode` (`src/context/transfer/mod.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| E5: `hdrReferenceWhite` not finite or <= 0 throws `RangeError` | Covered | `output_alpha_and_white_args` (`src/node/utils.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| E6: `premultiplied: true` multiplies colour by alpha; default does not | Covered | `ExportOptions::raw_alpha_type`, `read_output` (`src/context/page.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| E7: `getImageData` honours `colorSpace`; `ImageData` accepts non-`srgb` spaces | Covered | `image_data_export_arg` (`src/node/utils.rs`), `getImageData` (`src/context/api.rs`), `ImageData` (`lib/classes/imagery.js`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251) at `ee89242`: pass on Ubuntu, macOS, Windows x Node 18, 24 | None |
| E8: Rust PQ and HLG functions match reference values | Covered | `pq_from_nits`, `hlg_from_scene`, `encode_pq`, `encode_hlg` (`src/context/transfer/mod.rs`) | CI `rust-ci.yml` [35226347056](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35226347056) at `7fb83d4`: `cargo test --lib` green | None |

## Invariants

- Expected values in tests come from formulas written in the test, never from a
  second Skia call.
- For PQ and HLG, the transfer runs once, in `src/context/transfer/mod.rs`.

## Failure Modes

- Unsupported output colour type for a colour space: `TypeError` naming both.
- `hdrReferenceWhite` invalid: `RangeError`.

## Required Evidence Before Marking Complete

- Source evidence: `Page::encoded_as` and `PageRecorder::get_pixels` in
  `src/context/page.rs`; `src/context/transfer/mod.rs`; `export_options_arg` and
  `image_data_export_arg` in `src/node/utils.rs`; `lib/classes/imagery.js`.
- Executable evidence: the named tests, green in the `test.yml` CI run and the
  `rust-ci.yml` run on the branch (run URLs in the rows).
