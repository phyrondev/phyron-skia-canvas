# Tasks: Wide-Gamut and HDR Pixel Export

**Input**: `spec.md`, `plan.md`, `research.md`, `data-model.md`, `contracts/`.

Order tasks by dependency. Mark a task `[P]` when it can run in parallel with
other `[P]` tasks in the same group -- it touches different files and has no
ordering constraint. Pair each source task with the test or manual-QA task that
produces its contract evidence. Write the test before the implementation it
covers. No local native builds: a test is written first in the same commit, and
it runs in CI.

## Phase 1: Contract Setup

- [x] T001 Update `.blueprints` to `origin/main`; seed `specs/README.md`,
  `.specify/`, `.claude/commands/`; allowlist them in `.gitignore`.
- [x] T002 Point `.specify/feature.json` at this directory; add the SDD block
  to `AGENTS.md`.
- [x] T003 Write `spec.md`, record clarifications.
- [x] T004 Write `plan.md`, `research.md`, `data-model.md`, `quickstart.md`,
  `contracts/*.md`, `checklists/requirements.md`.

## Phase 2: User Story 2 -- Colour Names (`contracts/color-names.md`)

- [ ] T005 [P] Add `tests/suite/color.test.js` with N1-N4 tests.
- [ ] T006 `src/node/utils.rs`: `to_color_space`, `to_color_type` return
  `Result<_, String>` with the accepted-name list; `from_color_space` compares
  against the eight canonical spaces; add explicit `rgba` alias.
- [ ] T007 Throw `TypeError` at the callers: `src/node/canvas.rs` (`new`),
  `src/node/utils.rs` (`export_options_arg`, `image_data_arg`,
  `image_data_export_arg`), `src/node/image.rs`, `src/context/api.rs`.

## Phase 3: Transfer Module (`contracts/raw-export.md` E8)

- [ ] T008 [P] `src/context/transfer/tests.rs`: PQ at 0, 100, 203, 1000,
  10000 nits; HLG white 203 -> 0.750; reference white scaling; negative input
  clamps to 0.
- [ ] T009 `src/context/transfer/mod.rs`: `encode_pq(rgba: &mut [f32],
  reference_white: f32)`, `encode_hlg(rgba: &mut [f32], reference_white: f32)`
  on unpremultiplied RGBA F32 rows. Register in `src/context/mod.rs`.

## Phase 4: User Story 0 and 3 -- Working Space (`contracts/working-space.md`)

- [ ] T010 [P] Add W1-W5 tests to `tests/suite/color.test.js`.
- [ ] T011 `src/context/page.rs`: `ExportOptions.working_color_type`,
  `working_color_space`; `Page::encoded_as` and `PageRecorder::update`
  allocate with them; `PageCache` stores and compares them;
  `PageRecorder::is_config_stale` compares them.
- [ ] T012 `src/node/canvas.rs` (`toBuffer`, `toBufferSync`, `save`,
  `saveSync`) and `src/context/api.rs` (`getImageData`): fill the working
  fields from `Canvas`.
- [ ] T013 `PageRecorder::get_image`: working space, `BitDepth::F16` for non
  8-bit working types.

## Phase 5: User Story 1 -- Output Encoding (`contracts/raw-export.md` E1-E7)

- [ ] T014 [P] Add E1-E7 tests to `tests/suite/color.test.js`, expected values
  from formulas in the test.
- [ ] T015 `src/node/utils.rs`: parse `premultiplied` and `hdrReferenceWhite`
  (`RangeError` when not finite or <= 0) for export and `getImageData`.
- [ ] T016 `src/context/page.rs`: `read_output` (plan Design 3); use it in the
  `raw` branch, the PNG, JPEG and WebP branches, and `PageRecorder::get_pixels`.
- [ ] T017 `lib/classes/canvas.js`, `lib/classes/context.js`,
  `lib/classes/imagery.js`: pass the new options; `ImageData` accepts every
  canonical colour space.

## Phase 6: User Story 4 -- GPU Fallback (`contracts/gpu-fallback.md`)

- [ ] T018 [P] Add G1-G2 test to `tests/suite/color.test.js`, skipped with a
  reason when `backend().gpuAvailable` is false.
- [ ] T019 `src/gpu/mod.rs`: `make_surface` retries with `surfaces::raster`
  and records the colour type in the shared cell; `src/node/canvas.rs`: cell
  on `Canvas`, `fallback` in `get_engine_status`.

## Phase 7: Docs (R8, R9)

- [ ] T020 [P] `lib/index.d.ts`: `ExportOptions` and `ImageDataExportSettings`
  (`premultiplied`, `hdrReferenceWhite`, remove "must be srgb"),
  `EngineDetails.fallback`.
- [ ] T021 [P] `README.md`: working space against output encoding; colour
  types per format.

## Completion Gate

- [ ] T022 Push; `rust-ci.yml` green (fmt, clippy, `cargo test`) -- run URL.
- [ ] T023 `test.yml` by `workflow_dispatch` green on all 6 jobs -- run URL.
- [ ] T024 Manual QA step 1 on a GPU host (needs the developer).
- [ ] T025 Update contract rows with evidence; tick tasks; delete
  `plans/wide-gamut-hdr-export.md`.
