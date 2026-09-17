# Implementation Plan: Wide-Gamut and HDR Pixel Export

**Branch**: `feat/wide-gamut-hdr-export` | **Date**: `2026-09-17` |
**Spec**: `specs/001-wide-gamut-hdr-export/spec.md`.

## Summary

Separate the working space from the output encoding. Canvases composite in
their own `colorType` and `colorSpace`; export options convert once at
readback. `toBuffer("raw")` and `getImageData` honour `colorSpace`,
`colorType`, `premultiplied` and `hdrReferenceWhite`. Colour names fail loudly
and read back canonical. Caches key on the working space. The GPU engine falls
back to CPU raster for colour types it cannot allocate and reports it.

## Technical Context

**Language/Version**: Rust 2024 (Neon), JavaScript (CommonJS), TypeScript
declarations.

**Primary Dependencies**: `skia-safe` 0.97.2, `neon`, `rayon`, `dashmap`.

**Storage**: None. Wire formats: JS option objects, raw pixel buffers (see
`data-model.md`).

**Testing**: `node --test` (`tests/suite/*.test.js` through `tests/runner`),
`cargo test` for the transfer module.

**Target Platform**: Node 18 and 24 on Linux, macOS, Windows. CPU raster and
GPU (Vulkan, Metal).

**Performance Goals**: One extra `read_pixels` plus one per-pixel pass only for
PQ and HLG outputs. No extra pass for other outputs.

**Constraints**: Evidence runs locally and in CI on the
pushed branch. No `unwrap`/`expect` without `// SAFETY:`. Nightly rustfmt.

## Constitution Check

- Source-of-truth: `.specify/feature.json` -> `specs/001-wide-gamut-hdr-export`.
- Required artifacts: all present.
- Evidence policy: each contract has `Required Evidence Before Marking
  Complete`.
- Scope: one user story per commit (see Implementation Slices).
- Shared logic owner: named in `data-model.md` (Ownership Matrix).
- Review gates: `just ci` equivalent runs in CI (`rust-ci.yml`, `test.yml`).
  Visual baselines are not regenerated.

## Project Structure

```text
specs/001-wide-gamut-hdr-export/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── tasks.md
├── checklists/
│   └── requirements.md
└── contracts/
    ├── color-names.md
    ├── working-space.md
    ├── raw-export.md
    └── gpu-fallback.md

src/context/transfer/mod.rs     new: PQ and HLG encode, reference white
src/context/transfer/tests.rs   new: unit tests
src/context/page.rs             ExportOptions, Page::encoded_as, PageRecorder, PageCache
src/node/utils.rs               name parsing, export option parsing
src/node/canvas.rs              working space into ExportOptions, engine status
src/node/image.rs               ImageData names
src/context/api.rs              getImageData options
src/gpu/mod.rs                  CPU raster fallback
lib/classes/imagery.js          ImageData colour space check
lib/index.d.ts, README.md       docs
tests/suite/color.test.js       new: contract tests
```

## Design

1. `ExportOptions` gets `working_color_type`, `working_color_space`,
   `premultiplied`, `hdr_reference_white`. `toBuffer`, `toFile` and
   `getImageData` fill the working fields from the `Canvas`.
2. `Page::encoded_as` and `PageRecorder::update` allocate surfaces with the
   working fields. `PageCache` and `PageRecorder::is_config_stale` compare them.
3. One function `read_output(surface_or_image, crop, &ExportOptions)` in
   `page.rs` produces output pixels:
   - SDR spaces: `read_pixels` into `ImageInfo(output type, alpha, output
     space)`.
   - PQ and HLG: `read_pixels` into `RGBAF32` unpremul `rec2020-linear`, run
     `transfer::encode_pq` or `transfer::encode_hlg` with
     `hdr_reference_white`, premultiply if asked, wrap as a `rec2020-linear`
     raster image, `read_pixels` into the output type in `rec2020-linear`.
   - `raw` returns these bytes. PNG, JPEG and WebP encode a `Pixmap` of them
     (unpremul), tagged with the output `ColorSpace`, so the PNG `iCCP`
     behaviour stays.
4. `PageRecorder::get_image` rasterizes the source picture into a raster
   surface of the source working colour type and space, and returns the
   snapshot. Float `putImageData` writes unclamped values.
5. `to_color_space` and `to_color_type` return `Result<_, String>`; callers
   throw `TypeError`. `from_color_space` compares with the eight canonical
   spaces.
6. `RenderingEngine::make_surface` retries with `surfaces::raster` on GPU
   failure and writes the colour type name to a shared `Arc<Mutex<Option<
   String>>>` owned by `Canvas`; `get_engine_status` adds `fallback`.

## Execution Rules

1. Resolve ambiguous behavior before implementation.
2. Work one user story or one contract row at a time.
3. Add or update tests from the contract invariants.
4. Mark rows `Covered` only after evidence is present.

## Artifact Checklist

- [x] Active feature pointer is updated.
- [x] Required artifact set exists.
- [x] Each contract file has `Required Evidence Before Marking Complete`.
- [ ] Each `Covered` row cites source evidence.
- [ ] Each `Covered` row cites executable or manual QA evidence.

## Implementation Slices

One commit each, test first in the same commit.

1. Spec and blueprints (this directory, `.blueprints` bump, SDD seeding).
2. US2 colour names -> `color-names.md` N1-N4.
3. PQ and HLG transfer module -> `raw-export.md` E8.
4. US0 and US3 working space compositing and caches -> `working-space.md`
   W1-W10.
5. US1 output encoding, `premultiplied`, `hdrReferenceWhite`, `getImageData`
   -> `raw-export.md` E1-E7.
6. US4 GPU fallback -> `gpu-fallback.md` G1-G2.
7. R8 and R9 types and README.
8. Evidence: CI run URLs into the contract rows; manual GPU QA row.
