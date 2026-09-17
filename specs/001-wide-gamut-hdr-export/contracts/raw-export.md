# Contract: Raw Export Encoding

## Scope

This contract covers the output encoding of `toBuffer("raw")` and
`getImageData`: colour space, colour type, `premultiplied` and
`hdrReferenceWhite`. It does not cover compositing (see `working-space.md`) or
PNG `cICP` chunks (non-goal).

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| E1: `RGBAF32` output in `rec2020-linear`, `srgb-linear`, `display-p3-linear`, `srgb` matches the spec table within `1e-3` | Open | None | None | `tests/suite/color.test.js` "raw export converts to the requested space" |
| E2: `R16G16B16A16UNorm` `rec2020-pq` matches ST 2084 at 203 nits within 2 code values | Open | None | None | `tests/suite/color.test.js` "raw export encodes PQ" |
| E3: `R16G16B16A16UNorm` `rec2020-hlg` matches BT.2100 HLG (1000 nits, gamma 1.2) within 2 code values; white is 75% | Open | None | None | `tests/suite/color.test.js` "raw export encodes HLG" |
| E4: `hdrReferenceWhite: 100` maps linear 1.0 to the PQ code for 100 nits | Open | None | None | `tests/suite/color.test.js` "hdrReferenceWhite scales PQ and HLG" |
| E5: `hdrReferenceWhite` not finite or <= 0 throws `RangeError` | Open | None | None | `tests/suite/color.test.js` "hdrReferenceWhite validates" |
| E6: `premultiplied: true` multiplies colour by alpha; default does not | Open | None | None | `tests/suite/color.test.js` "premultiplied option" |
| E7: `getImageData` honours `colorSpace`; `ImageData` accepts non-`srgb` spaces | Open | None | None | `tests/suite/color.test.js` "getImageData converts to the requested space" |
| E8: Rust PQ and HLG functions match reference values | Open | None | None | `cargo test` unit tests in `src/context/transfer/tests.rs` |

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
