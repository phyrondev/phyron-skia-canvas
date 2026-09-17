# Contract: GPU Fallback for Colour Types

## Scope

This contract covers export and `getImageData` with the GPU engine when the GPU
cannot allocate the working colour type.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| G1: GPU engine, `RGBAF32` canvas: `toBuffer("raw")` succeeds with the E1 values | Covered | `RenderingEngine::make_surface` (`src/gpu/mod.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24 (ran, not skipped, Quadro RTX 5000); 3.6.0 threw `Could not allocate new 3x1 bitmap` on the same host; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251): ran and passed on macOS (Metal), skipped on Ubuntu and Windows (no GPU) | None |
| G2: after a fallback, `canvas.engine.fallback` names the colour type; absent otherwise | Covered | `FallbackCell` (`src/gpu/mod.rs`), `get_engine_status` (`src/node/canvas.rs`) | local release build `bfc2d36`, Linux + Vulkan GPU: `node --test tests/suite/color.test.js` 24/24 (ran, not skipped, Quadro RTX 5000); 3.6.0 threw `Could not allocate new 3x1 bitmap` on the same host; CI `test.yml` [35228478251](https://github.com/phyrondev/phyron-skia-canvas/actions/runs/35228478251): ran and passed on macOS (Metal), skipped on Ubuntu and Windows (no GPU) | None |

## Invariants

- A fallback never changes pixel values relative to the CPU engine beyond
  `1e-3`.

## Failure Modes

- CPU raster also fails: the export rejects with the allocation error.

## Required Evidence Before Marking Complete

- Source evidence: `RenderingEngine::make_surface` in `src/gpu/mod.rs`;
  `get_engine_status` in `src/node/canvas.rs`.
- Executable evidence: the named test; on CI without a GPU it reports skipped
  with a reason.
- Manual QA evidence: `quickstart.md` step 1 on a host with a real GPU, output
  pasted into the row.
