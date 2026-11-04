# Handoff: phyron-skia-canvas 3.7.3 (2026-11-04)

Moving to another machine. Nothing below the "Pending" line has been compiled.

## Where things are

| Ref                            | State                                                                                                                                                                        |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `main` = `dbbd35a8` (pushed)   | 3.7.3 code, Rust CI green. Not tagged, not on npm.                                                                                                                           |
| `a150b9cb` (on `main`)         | Pending regression fixes #32, #33, #34, #35, #36, #38. Moves pixels.                                                                                                         |
| `wip/3.7.3-skia-init` (pushed) | On top of `main`. Dev-built and tested (step 1). `graphics::init()`, rounding tests, raster `drawImage` sources reported to V8, docs, changelog, these scripts.              |
| `skia-m157` (pushed)           | On top of `wip/3.7.3-skia-init`. **Uncompiled.** `skia-safe` 0.97.2 -> 0.157.0 (Skia m148 -> m157), `ganesh` feature added to `vulkan` and `metal`. Nothing else ported yet. |

`dbbd35a8` holds `Canvas.toImage()`, `Canvas.release()`, `Image.release()`, V8 external-memory reporting (`napi_adjust_external_memory`, read back with `externalMemory()`), the `new Canvas(w, h, { gpu: false })` fix, and Skia's raster cache at 256 MB (`SKIA_CANVAS_CACHE_MB`; the default is back to Skia's 32 MB on `wip/3.7.3-skia-init`, see Decisions).

## Decisions (Moritz)

- Scope and order of 3.7.3 approved; tag and publish only with Moritz's go-ahead.
- `skia_safe::graphics::init()` always on at module load (option A, no opt-out variable). It is on `wip/3.7.3-skia-init`.
- Raster cache: default back to Skia's 32 MB; `SKIA_CANVAS_CACHE_MB` stays as an opt-in library option, Studio will not set it. 440 MB is too much of a pod's 18 GiB headroom for 5%; speed comes from the motion-blur fast path, smaller surfaces (PP-1633) and `graphics::init()`.
- Skia rebase: do it; it goes into 3.7.3 only if pixels are byte-identical to 3.7.2. Otherwise report which cases differ, how many pixels and the max per-channel difference, and Moritz decides.
- 3.7.3 is not byte-identical to 3.7.2, by decision: `a150b9cb` ("high" smoothing follows Chrome: trilinear on minification, Mitchell on upscale) and `graphics::init()` both ship. Studio draws with "high"; the new minification costs the same as "low" (full-HD F16 at 0.37x: 5.5 ms, 3.7.2's Mitchell was about 50% over "low") and stops aliasing. Antialiasing and shape differences are expected anyway (Studio's Rust pipeline on parley differs there too).
- Skia m157 goes to 3.8.0, not 3.7.3: its brotli `BUILD.gn` bug blocks the link (see step 4 on `skia-m157`). No CI patch for now; upstream may be fixed by the time it matters.

## Verified (on `dbbd35a8`, dev build unless noted)

- 213 JS tests and 117 Rust tests pass; clippy and fmt clean; memory tests (incl. 6 worker threads with live objects at exit) pass on Node 22.23.4 and 24.22.2.
- 25 scenes raw-export byte-identical to the 3.7.2 npm prebuild (`identity.js`).
- `deltas.js` (adds text, gradient, blur, image smoothing): 3.7.3 vs 3.7.2 differs only through `imageSmoothingQuality = "high"` minification (Mitchell -> trilinear, from `a150b9cb`): max 13 codes premultiplied in 8-bit sRGB, 22 through the F16 path. Raw exports show 255 on nearly transparent pixels (unpremultiplied); compare premultiplied.
- 188 linear-blend probe: 188 on 3.7.2 and 3.7.3.
- Orphaned canvases (500 x 1024x1024 with a `getImageData` surface, one `await` each): peak 167 MB RSS vs 860 MB on 3.7.2.

## Benchmarks so far (release build of `dbbd35a8`, this host: Xeon E-2276M, AVX2, no `graphics::init()`)

Studio's loop, `studioLoopRepro.mjs <lib> 30 64` (30 blurred items x 64 motion-blur samples):

| Build     | Loop                           | Time  | Peak RSS            |
| --------- | ------------------------------ | ----- | ------------------- |
| 3.6.0 npm | 8-bit                          | 63 s  | 435 MB              |
| 3.7.0 npm | F16 linear                     | 224 s | 25.7 GB (the OOM)   |
| 3.7.2 npm | F16 linear                     | 281 s | 479 MB              |
| 3.7.2 npm | 8-bit                          | 87 s  | 443 MB              |
| 3.7.3     | F16 linear                     | 167 s | 937 MB              |
| 3.7.3     | 8-bit                          | 71 s  | 890 MB              |
| 3.7.3     | F16, `toImage` per sample only | 175 s | 14.5 GB (see below) |

The `toImage` variant pinned 30 x 64 x 7.7 MB because each accumulator's recording kept its sample rasters and the accumulators were drawn lazily onward. `studioLoopToImage.mjs` now also cuts at the accumulator; rerun it. Docs (`docs/api/canvas.md`, `toImage()`) explain the pattern.

## Benchmarks on the new machine (release builds, Ryzen 7 6800H, AVX2, no AVX-512, Node 22.21.1)

About 3x faster than the Xeon; do not mix the two tables. `studioLoop*.mjs <lib> 30 64`, two runs where given:

| Build | Loop | Time | Peak RSS |
| --- | --- | --- | --- |
| 3.6.0 npm | 8-bit | 28.7 s | 430 MB |
| 3.7.2 npm | F16 linear | 90.9 s, 93.5 s | 473 MB, 487 MB |
| 3.7.2 npm | 8-bit | 18.4 s | 443 MB |
| `dbbd35a8` (no init) | F16 linear | 118.7 s, 102.6 s | 863 MB, 861 MB |
| `dbbd35a8` (no init) | 8-bit | 18.8 s | 870 MB |
| `wip/3.7.3-skia-init` | F16 linear | 42.4 s, 42.5 s | 913 MB, 862 MB |
| `wip/3.7.3-skia-init` | 8-bit | 17.6 s | 835 MB |
| `wip/3.7.3-skia-init` | F16, `toImage` + `release` | 55.1 s | 1098 MB |
| `wip/3.7.3-skia-init` | F16, `SKIA_CANVAS_CACHE_MB=32` | 44.5 s | 476 MB |

- On this host the whole F16 speedup is `graphics::init()` (2.2x over 3.7.2); `dbbd35a8` alone is not faster than 3.7.2 here, unlike on the Xeon.
- The 256 MB cache buys 5% (2 s) for about +440 MB peak; a rerun at the new 32 MB default (no variable set, `wip/3.7.3-skia-init` after the change) took 40.0 s at 467 MB peak, so the gain is within run-to-run noise. Decided: back to 32 MB (see Decisions).
- Orphaned canvases: 159 MB peak RSS. 188 probe: 188. 8-bit rounding probe: no difference at 0.1 code or more from a boundary.
- `f16store.js` (now fed through float `ImageData`): with init 400/400 round to nearest, denormals kept; `dbbd35a8` and 3.7.2 truncate 400/400 and flush denormals to 0.
- `deltas.js` 3.7.2 -> `dbbd35a8`: only `imageSmoothingQuality = "high"` (20/20 identical without it), max 9 codes on an 8-bit canvas and 37 codes premultiplied in 8-bit output of float canvases (the old machine reported 13 and 22; not reproduced). `dbbd35a8` -> init: 8-bit canvases identical; F32 canvases 1e-5 linear; F16 canvases one half step (2e-3 linear at HDR values), in 8-bit output at most 1 code on opaque and 2 on alpha >= 128 pixels.

## Findings to report

- Rounding (Studio asked; answered to studio-9b): no float -> 8-bit path truncates (raster pipeline `to_unorm`, skcms `to_fixed`, `Sk4f_toL32`, `SkConvertPixels`; PNG writes floats as 16-bit). x86 SIMD rounds ties to even, scalar half up. Skia's sRGB encode uses `approx_powf` (about 0.02 code of error), so the tests keep 0.1 code from boundaries.
- F16 stores truncated and flushed denormals because the module never called `SkGraphics::Init()`: Skia ran its SSE2 baseline on every host, whose `to_half` drops 13 mantissa bits. m157's baseline `to_half` is unchanged, so the rebase does not fix it; `graphics::init()` does (AVX2 F16C path).
- Array `fillStyle`/`strokeStyle` colors are not exact on float canvases, on 3.7.2 as well as 3.7.3: `SkPaint::setColor(SkColor4f, SkColorSpace*)` converts the color to sRGB for storage and the draw converts it back, both through skcms's approximate `powf`. A linear fill on an `RGBAF32` `srgb-linear` canvas lands up to 4.6e-4 relative off (0.92 of a half-float step); `putImageData` of the same floats is exact. Below a tenth of an 8-bit code and under one half-float step. A color shader does not avoid it (`SkShaders::Color(SkColor4f, space)` also stores sRGB, `SkColorShader.cpp`). The loss is skcms's `powf_` in `skcms_TransferFunction_eval`; options are a 1x1 float image shader per solid color (loses the constant-color and memset fast paths) or patching skcms in a vendored Skia. Not worth it so far; float `ImageData` is exact for anyone who needs exact values. The rounding test feeds F16 through float `ImageData` because of this.
- `SKIA_CANVAS_THREADS` already bounds the async export pool (Studio's production OOM suspect is pool size vs core count); Studio now sets it.

## Pending, in order

1. ~~**Build `wip/3.7.3-skia-init`** (dev)~~ Done on the new machine (Ryzen 7 6800H, AVX2, Node 22.21.1): dev build, `node --test` 220 pass, 4 skipped, rustfmt clean on `nightly-2026-05-25`. `rounding.test.js` 11/11 after switching its F16 inputs from fills to float `ImageData` (see the `SkPaint` finding); the 3.7.2 prebuild still fails its 3 F16 tests. W20 `Covered`.
2. ~~**Release build** and benchmarks~~ Done, see "Benchmarks on the new machine". `CHANGELOG.md` numbers filled in. Open for Moritz: the byte-identical question under Decisions, and the cache default.
3. ~~**AVX2 vs AVX-512**~~ Dropped (Moritz): the difference between instruction sets does not matter.
4. **Skia rebase on `skia-m157`** (already on top of `wip/3.7.3-skia-init`; rebase again if that branch moves): compile and port. Breaking changes per rust-skia notes: Vulkan context builder (0.99; `src/gpu/vulkan/{engine,renderer}.rs` use `vk::BackendContext::new` + `direct_contexts::make_vulkan`), Ganesh/Graphite feature split (0.153, done in `Cargo.toml`), third-party libs incl. freetype/harfbuzz updated (0.156; text pixels will likely move), FFI aliasing fixes (0.157: PDF `Document` borrows its writer -- `pdf::new_document` in `src/context/page.rs`; `TypefaceFontProvider`, `RefHandle::native_mut()` gone). **Regenerate `containers/use-static-fontconfig.patch`** against the rust-skia 0.157.0 tag or the Linux node prebuild breaks with a misleading error (`.github/workflows/build.yml` applies it with `-p1` to the vendored rust-skia checkout; verify `patch -p1 --dry-run` exits 0 with no fuzz). Then `deltas.js` vs 3.7.2 and the full suite; report to Moritz.
5. Merge to `main`, CI green on all platforms (the new `src/node/memory.rs` has a Windows-only branch that has never been compiled), then ask Moritz for the tag. Release flow: `just release patch` (interactive), prebuilds via the draft release, un-draft, `npm run snapshot`, publish. At least linux-x64-gnu must be built.
6. Tell studio-7b the final rev; it runs the five Cars golden cases (pixel verdict and timing) before publish. Known Studio impacts: `"high"` smoothing on every non-aligned draw (downscales change), `drawParagraph` now honours `globalAlpha`/blend (faded text may double-apply opacity), `graphics::init()` (pixels within about 1 code).

## Gotchas

- Benchmark only release builds: the dev build renders about 6x slower (1649 s vs 281 s on the F16 loop).
- `process.memoryUsage().external` does not include the Node-API external-memory counter on Node 22/24; use `externalMemory()`.
- Compare raw exports premultiplied; unpremultiplied near-transparent pixels swing by 255.
- `new Canvas(w, h, { gpu: false })` only works from `dbbd35a8` on; for 3.7.2 comparisons set `canvas.gpu = false`.
- Studio sessions on the old machine: studio-7b (dep, golden suite), studio-9b (production OOM). They will not be reachable from the new machine.

## Standing rules from the old machine's agent memory

That memory stayed on the old machine; these are the parts that apply anywhere.

- **Prose (commits, PRs, comments, docs):** no spaces around `/`; `--` instead of em/en dashes; ASCII only. Never add a Claude/Anthropic co-author trailer or a "Generated with Claude Code" footer -- Moritz authors the commits.
- **No PRs on weekends:** commit and push branches, but do not open, merge or push PRs on Saturday/Sunday unless Moritz asks.
- **Rust API naming:** accessors have no `get_` prefix (`rects_for_range()`, not `get_rects_for_range()`); boolean properties read `is_*`/`has_*`. Applies to the crate-root Rust API; the Neon binding functions in `src/node/` keep their JS-shaped names.
- **Build locally** before pushing; CI is for the OS matrix, not the only compiler.
- **One `target/` directory:** a cold Skia build is 10-25 min and about 50 GB. Switch branches in place (`git switch`) instead of `git worktree add`; if a second worktree is unavoidable, point `CARGO_TARGET_DIR` at the main checkout's `target/`. Never run two cargo builds on it at once.
- **`Cargo.lock` is not tracked** (library crate). Leave it on disk, never re-add it.
- **`.gitignore` is a denylist:** `/*.*` ignores every root file with an extension except an allowlist. A new root config file (`clippy.toml`, `rust-toolchain.toml`, ...) is silently dropped unless you add `!/<file>`.
- **rustfmt is pinned to `nightly-2026-05-25`** in CI (`rustfmt.toml` uses nightly-only options). Format with `cargo +nightly-2026-05-25 fmt`; a newer nightly can drift. The old machine formatted with a newer nightly and CI still passed on `dbbd35a8`, but check. A newer clippy also flagged four lints CI's does not (fixed on `main`).
- **CI clippy matrix** covers only features that gate Rust code: features off, `vulkan,window,node-addon` (Ubuntu), `metal,window,node-addon` (macOS). `freetype` gates no Rust.
- **Skia bump checklist:** regenerate `containers/use-static-fontconfig.patch` against the new rust-skia tag (see Pending step 4).
- **npm release flow:** prebuilds upload to a draft GitHub release `v<version>`; un-draft it (`gh release edit v<version> --draft=false`) before `npm publish` so the download URLs resolve, then `npm run snapshot` writes the asset sha256 digests into `package.json`. The npm package and the `skia-canvas` crate version independently (`rust-v<X.Y.Z>` tags for the crate).
- **Search and rewrite tools (blueprints `9c33b02`, `.blueprints/base/AGENTS.md`):** `ast-grep` for searching and rewriting source code, `rg` for other text, `fd` for files. `grep`, `find` and `ls -R` are forbidden, piped output included. Mechanical code edits are `ast-grep` rewrites (preview without `-U` first, then `cargo fmt`), never `sed`, `perl -pi` or a regex script. The old machine's agent worked before this bump and used `grep`/`sed`/Python edits; do not copy that.
