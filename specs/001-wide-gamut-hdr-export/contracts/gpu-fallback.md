# Contract: GPU Fallback for Colour Types

## Scope

This contract covers export and `getImageData` with the GPU engine when the GPU
cannot allocate the working colour type.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| G1: GPU engine, `RGBAF32` canvas: `toBuffer("raw")` succeeds with the E1 values | Open | None | None | `tests/suite/color.test.js` "GPU engine falls back to CPU raster" (skipped without GPU) and manual QA step 1 |
| G2: after a fallback, `canvas.engine.fallback` names the colour type; absent otherwise | Open | None | None | same test, and manual QA step 1 |

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
