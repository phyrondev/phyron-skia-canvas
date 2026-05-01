# Studio Renderer Native API Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED: Use `superpowers:subagent-driven-development` (if subagents available) or `superpowers:executing-plans` to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `phyron_skia_canvas::native` until it can cover the required `@phyron/studio-renderer` backend surface from the Studio TypeScript package, so `studio-render-native` can depend on `phyron-skia-canvas` instead of `skia-safe` without losing renderer parity.

**Architecture:** Promote the mature Skia functionality already present behind the Node/Canvas2D binding into a Rust-only facade. Keep `skia_safe` and Neon private. Add a real surface/canvas/paint/filter/text/image API that mirrors the renderer contract in `packages/renderer`, then migrate Studio against that contract.

**Tech Stack:** Rust 2024, private `skia-safe` internals, `phyron-skia-canvas` native facade, `parking_lot` for shared Rust-facing state when needed, `just`, linked `.blueprints` submodule, downstream Studio `@phyron/studio-renderer` contract.

---

## Status And Approval

API changes are approved by Moritz on 2026-05-01 for this work. This plan is a follow-up to `docs/superpowers/plans/2026-05-01-native-rust-api-boundary.md`.

### Implementation status (2026-05-02)

- **Chunks 1-2 (Tasks 1-4): complete** on branch `plan/studio-renderer-gap-closure`, stacked on `plan/native-rust-api-boundary` (PR #5).
  - `NativeBackend`, `NativeSurface` with `with_canvas`, `snapshot`, `create_offscreen`, `flush`.
  - `PixelColorSpace` (6 variants), `PixelDepth` (Uint8/F16/F32), `PixelExportOptions`, `ExportedPixels`.
  - `read_pixels`, `read_pixels_raw`, `read_pixels_as`, `read_pixels_linear`, `write_pixels`, `write_pixels_linear`.
  - 7 contract tests in `tests/native_studio_renderer_contract.rs` -- all green.
- **Chunks 3-8 (Tasks 5-15): not started** -- paint/blend/path, filters/shaders, raw image creation, SVG, font/paragraph, Studio adapter, docs.
- Per reviewer feedback, tests for Chunks 3-5 were not pre-written -- they will land alongside their implementation chunks to keep every commit green.

The first plan delivered a minimum Rust facade:

- `NativeRecorder`.
- Basic `NativeCanvas`.
- Linear sRGB/Display P3/Rec.2020 working spaces.
- Encoded image decode.
- Basic shape/text drawing.
- Raw frame readback.

That is not enough for the required Studio renderer surface. This plan uses required usage from the TypeScript renderer package, not current `studio-render-native` usage.

Required usage sources audited:

- `/home/moritz/code/typescript/studio/packages/renderer/src/backend/types.ts`.
- `/home/moritz/code/typescript/studio/packages/renderer/src/backend/skia-canvas/*.ts`.
- `/home/moritz/code/typescript/studio/packages/renderer/src/render/frameRenderer.ts`.
- `/home/moritz/code/typescript/studio/packages/renderer/src/render/items/*.ts`.
- `/home/moritz/code/typescript/studio/packages/renderer/src/render/renderHelpers.ts`.
- `/home/moritz/code/typescript/studio/packages/renderer/src/render/pixelExport.ts`.
- `/home/moritz/code/typescript/studio/packages/renderer/src/effects/*.ts`.

Before implementing, read:

- `AGENTS.md`.
- `.blueprints/base/AGENTS.md`.
- `.blueprints/base/api-changes.md`.
- `.blueprints/base/test-ownership.md`.
- `.blueprints/lang/rust/AGENTS.md`.
- `.blueprints/lang/rust/testing.md`.

Do not edit `.blueprints`.

## Current Gap Assessment

### Gap Matrix

| Renderer requirement from `packages/renderer` | Current `native` facade | Required action |
| --- | --- | --- |
| Long-lived offscreen surfaces with `getCanvas()`, `snapshot()`, `createOffscreen()`, `flush()`, `dispose()`. | No. `NativeRecorder` records a page and renders at the end. | Add `NativeSurface` owning a private Skia surface and exposing a borrowed `NativeCanvas`. |
| Read/write pixel data during rendering for Citra corner pin, liquid resize, resample, ID buffer, and export. | Partial. Only final `render_raw()` readback exists. | Add `read_pixels()`, `read_pixels_as()`, `read_pixels_linear()`, `write_pixels()`, and `write_pixels_linear()`. |
| Pixel export spaces: `srgb`, `srgb-linear`, `display-p3`, `display-p3-linear`, `rec2020`, `rec2020-linear`, with `uint8`, `f16`, and `f32`. | Partial. `OutputColorSpace` has only three names and does not distinguish gamma-coded vs linear exports. | Add a strict `PixelColorSpace`/`PixelDepth` export API. Preserve internal linear working space. |
| Studio internal alpha is premultiplied. `putImageData` bridge may request unpremultiplied RGBA8. | Partial. `RgbaLinear` is documented as premultiplied, but the paint helper unpremultiplies before Skia paint. | Add explicit alpha-mode tests and document that only readback/write boundaries may convert alpha mode. |
| Working spaces are linear-light sRGB, linear-light Display P3, and linear-light Rec.2020. Values above `1.0` are valid. | Mostly present. Needs wider tests across surfaces, gradients, read/write, and image creation. | Add HDR/out-of-gamut tests for all three working spaces. Default remains linear sRGB. |
| Canvas state: `save`, `restore`, `saveLayer`, translate, rotate, scale, clipping. | Partial. No `scale`, no `save_layer`, no clipping in Rust facade. | Add `scale`, `concat_transform`, `clip_rect`, `clip_rrect`, `clip_path`, and `save_layer`. |
| Drawing: rect, round rect, oval, line, SVG path with fill rules, image src/dst, surface compositing. | Partial. No line, path, source-rect image draw, or surface draw. | Add `NativePath`, `draw_line`, `draw_path`, `draw_image_src`, `draw_surface`. |
| Paint state: alpha, blend mode, style, stroke width/cap, dash pattern, anti-alias, shader, image filter, color filter. | Partial. `ShapePaint` only covers fill/stroke basics. | Add `NativePaint` and keep `ShapePaint` only as a convenience wrapper if useful. |
| Blend modes: Canvas-compatible Porter-Duff and artistic modes including `plus-lighter`. | No. | Add a strict `BlendMode` enum and conversion to Skia blend modes. |
| Filters: blur, drop shadow, color matrix, luma color filter, gamma filters, compose, color-filter-as-image-filter. | No Rust facade. Existing JS binding has the internals. | Add `NativeImageFilter`, `NativeColorFilter`, and factory methods. |
| Shaders: linear gradients with `srgb`/`oklch` interpolation. | No Rust facade. Existing JS binding has gradient interpolation extensions. | Add `NativeShader::linear_gradient()`. |
| Images: encoded image decode, raw RGBA/F16/F32 image creation, dimensions, draw with nearest/linear/mipmap sampling. | Partial. Encoded decode only. | Add `NativeImage::from_pixels()` and sampling options. |
| Browser/server video frame path equivalent: decoded frame bytes become a drawable image without PNG re-encode. | No. | Use `NativeImage::from_pixels()` for rsmpeg decoded frames. |
| Text: paragraph layout, rich spans, font fallback, font registration, line metrics, rects-for-range, decorations, shadows, letter/word spacing, variable weights. | Very partial. `draw_text_box()` covers only simple text. | Add paragraph/text engine facade and font manager facade. |
| SVG path/image support. | Partial/unclear. `NativeImage::from_encoded()` may not cover SVG the same way the JS backend does. | Add contract tests for SVG XML bytes and `NativePath::from_svg()`. Add explicit API if needed. |
| ID buffer and mask stacks. | Blocked by missing surface, saveLayer, color filters, luma filter, blend modes, and raw readback. | Covered by the surface, filter, blend, and readback chunks. |
| Citra post-processing integration. | Blocked by missing linear F32 read/write and raw image creation. | Covered by pixel IO and image-from-pixels chunks. |
| Server/CLI encoded output. | Raw frame exists, but no PNG/JPEG Rust facade equivalent to `DrawBackend.toBuffer()`. | Add encoded output after raw parity, or document that encoding is a downstream concern. |

### Binding Quality Findings

- The existing Node binding is broad and useful. It already contains Path2D, image filters, color filters, paragraph layout, font library, image data, clipping, sampling, and Canvas2D state internals. The right work is exposing these through a Rust facade, not reimplementing them in Studio.
- The current Rust facade is intentionally minimal. It proves the API boundary, but `NativeRecorder` is the wrong primitive for full renderer parity because Studio needs offscreen surfaces, intermediate snapshots, read/write pixels, mask surfaces, and post-processing during a frame.
- `ShapePaint` is too narrow for Studio. The TS renderer treats paint as a mutable style accumulator with blend/filter/shader/stroke/dash/AA state.
- The text facade is far below Studio requirements. The TS renderer depends on paragraph layout, rich spans, baseline-shift overlays, line metrics, rects-for-range, font registration, variable/static weight resolution, shadows, decorations, letter spacing, word spacing, and wrapping/alignment.
- Global/shared state needs a Rust-facing policy. If the facade exposes shared font/filter/resource registries, use `parking_lot::Mutex` or owned managers. Do not expose `RefCell`-based globals to Rust consumers.
- The public Rust facade must continue to hide `skia_safe` and Neon. Existing JS/Neon modules may continue exposing their historical API.

## Non-Goals

- Do not remove the existing JS/Neon API.
- Do not rewrite Studio's TypeScript renderer in this crate.
- Do not regenerate visual baselines.
- Do not run `cargo clean`.
- Do not add GPU shared-buffer transport here.
- Do not implement FFmpeg decoding here.
- Do not add a SharedArrayBuffer transport dependency.

## Target Public Rust Shape

The final Studio-facing shape should be conceptually close to this:

```rust
use phyron_skia_canvas::native::{
    BlendMode, LinearColorSpace, NativeBackend, NativeImage, NativePaint, PixelColorSpace,
    PixelDepth, PixelExportOptions, Point, Rect, RgbaLinear, SamplingMode, SurfaceOptions,
};

let backend = NativeBackend::new();
let mut surface = backend.create_surface(
    1920,
    1080,
    SurfaceOptions {
        color_space: LinearColorSpace::Srgb,
        ..SurfaceOptions::default()
    },
)?;

let mut paint = NativePaint::default();
paint.set_color(RgbaLinear::opaque(1.0, 0.0, 0.0));
paint.set_blend_mode(BlendMode::SourceOver);

surface.with_canvas(|canvas| {
    canvas.clear(RgbaLinear::new_premultiplied(0.0, 0.0, 0.0, 0.0));
    canvas.save();
    canvas.translate(Point::new(100.0, 100.0));
    canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 200.0, 100.0), &paint);
    canvas.restore();
});

let pixels = surface.read_pixels_as(PixelExportOptions {
    color_space: PixelColorSpace::Srgb,
    depth: PixelDepth::Uint8,
    premultiplied: false,
})?;
```

Names may change during implementation, but the contract must cover the required capabilities above.

## File Structure

Expected new or expanded files:

- `src/native/backend.rs` -- `NativeBackend`, global resource policy, construction helpers.
- `src/native/surface.rs` -- `NativeSurface`, snapshot, offscreen, read/write pixels.
- `src/native/canvas.rs` -- expanded `NativeCanvas`.
- `src/native/paint.rs` -- `NativePaint`, style, stroke, blend, filters, shaders.
- `src/native/filter.rs` -- `NativeImageFilter`, `NativeColorFilter`, filter factories.
- `src/native/shader.rs` -- `NativeShader`, gradients.
- `src/native/path.rs` -- `NativePath`, SVG path parsing, fill rule.
- `src/native/image.rs` -- encoded and raw-pixel image constructors, sampling support.
- `src/native/text.rs` -- text style, paragraph layout, rich spans, paragraph metrics.
- `src/native/font.rs` -- Rust-facing font manager using owned state or `parking_lot::Mutex`.
- `src/native/pixels.rs` -- pixel export/read/write options and strict color-space/depth enums.
- `tests/native_studio_renderer_contract.rs` -- Rust contract tests matching `packages/renderer`.

Expected modified files:

- `src/native/mod.rs` -- public exports.
- `src/native/color.rs` -- stricter working/export color-space split.
- `src/native/error.rs` -- new typed errors.
- `src/lib.rs` -- keep only an export note if needed.
- `README.md` or `docs/api/*.md` -- document the Rust consumer facade.

Do not move or delete the existing Neon modules unless a separate cleanup is approved.

## Chunk 1: Contract Tests And API Inventory

### Task 1: Add renderer-surface contract tests.

**Files:**

- Create: `tests/native_studio_renderer_contract.rs`.

Steps:

- [x] Add a test that creates a `NativeSurface`, clears it, draws a rect, snapshots it, draws the snapshot to another surface, and reads tight RGBA8 pixels.
- [x] Add a test that creates same-config offscreen surfaces and composites them with `BlendMode::SourceOver`.
- [x] Add a test that `read_pixels_as()` supports `srgb`, `srgb-linear`, `display-p3`, `display-p3-linear`, `rec2020`, and `rec2020-linear` where Skia supports them, with typed errors for unsupported combinations.
- [x] Add a test that `read_pixels_linear()` returns F32 linear data and `write_pixels_linear()` writes it back without changing dimensions.
- [x] Add a test that all internal linear working spaces accept HDR values above `1.0` without clamping before export.
- [x] Add a test that premultiplied alpha is preserved internally and only converts when explicitly reading/writing unpremultiplied pixels.

Run:

```bash
cargo test --all-features --test native_studio_renderer_contract
```

Expected result: tests fail to compile until Chunks 2-5 add the API.

### Task 2: Add public leak audits.

**Files:**

- Create or extend: `tests/native_api_contract.rs`.

Steps:

- [x] Add an audit command note in the test comments for `rg -n "pub .*skia_safe|pub .*FunctionContext|pub .*JsBox|pub .*Handle<|pub .*RefCell" src/native`.
- [x] Add a Rust test that imports only `phyron_skia_canvas::native::*` and exercises the new public types.
- [x] Confirm no public `native` signatures expose `skia_safe`, Neon, `RefCell`, or JS types.

## Chunk 2: Surface And Pixel IO

### Task 3: Add `NativeBackend` and `NativeSurface`.

**Files:**

- Create: `src/native/backend.rs`.
- Create: `src/native/surface.rs`.
- Modify: `src/native/mod.rs`.
- Modify: `src/native/error.rs`.

Steps:

- [x] Add `NativeBackend::new()` and `create_surface(width, height, SurfaceOptions) -> Result<NativeSurface, NativeError>`.
- [x] Make `NativeSurface` own the private Skia surface or equivalent p-s-c page/surface object.
- [x] Add `NativeSurface::width()`, `height()`, `flush()`, `snapshot()`, and `create_offscreen()`.
- [x] Add `NativeSurface::with_canvas(|canvas| ...)` to borrow a `NativeCanvas` without exposing `skia_safe`.
- [x] Keep `NativeRecorder` working as an existing convenience API. Do not make the Studio renderer depend on it.

### Task 4: Add pixel export/import parity.

**Files:**

- Modify: `src/native/pixels.rs`.
- Modify: `src/native/surface.rs`.
- Modify: `src/native/color.rs`.

Steps:

- [x] Add `PixelColorSpace::{Srgb, SrgbLinear, DisplayP3, DisplayP3Linear, Rec2020, Rec2020Linear}`.
- [x] Add `PixelDepth::{Uint8, F16, F32}`.
- [x] Add `PixelExportOptions { color_space, depth, premultiplied }`.
- [x] Add `ExportedPixels` or extend `RawFrame` so callers can inspect color space, depth, alpha mode, stride, width, height, and bytes.
- [x] Add `NativeSurface::read_pixels()`, `read_pixels_raw()`, `read_pixels_as()`, `read_pixels_linear()`, `write_pixels()`, and `write_pixels_linear()`.
- [x] Return typed `NativeError` variants for unsupported color-space/depth/alpha combinations. Do not silently fall back to sRGB or RGBA8.

Tests:

- [x] Surface readback returns tight rows unless a caller explicitly asks for padded rows.
- [x] F16 and F32 exports preserve values above `1.0` in linear spaces.
- [x] Unpremultiplied RGBA8 readback round-trips for the Tauri `putImageData` bridge.
- [x] Premultiplied F16/F32 read/write round-trips for Studio internal use.

## Chunk 3: Canvas, Paint, Blend, Path, And Image Drawing

### Task 5: Replace `ShapePaint`-only drawing with `NativePaint`.

**Files:**

- Modify: `src/native/paint.rs`.
- Modify: `src/native/canvas.rs` or `src/native/recorder.rs` if `NativeCanvas` stays there.

Steps:

- [ ] Add `NativePaint` with color, alpha, style, stroke width, stroke cap, dash pattern, anti-alias, blend mode, shader, image filter, and color filter fields.
- [ ] Add `PaintStyle::{Fill, Stroke}`.
- [ ] Add `StrokeCap::{Butt, Round, Square}`.
- [ ] Add `BlendMode` covering the full TypeScript `BlendMode` union from `packages/renderer/src/backend/types.ts`.
- [ ] Keep `ShapePaint` as a convenience wrapper only if it can delegate to `NativePaint`.
- [ ] Add `clone`/copy semantics that do not share mutable internal state unsafely.

### Task 6: Expand `NativeCanvas`.

**Files:**

- Create: `src/native/canvas.rs`, or expand `src/native/recorder.rs` and split later.
- Create: `src/native/path.rs`.
- Modify: `src/native/image.rs`.

Steps:

- [ ] Add `save_layer(paint: Option<&NativePaint>)`.
- [ ] Add `scale(sx, sy)`.
- [ ] Add `concat_transform()` with an affine matrix options type. This covers future transforms without exposing Skia matrices.
- [ ] Add `clip_rect()`, `clip_rrect()`, and `clip_path()`.
- [ ] Add `draw_line()`.
- [ ] Add `NativePath::from_svg(svg_path_data, fill_rule)`.
- [ ] Add `draw_path(path, paint)`.
- [ ] Add `draw_image(image, dst, paint, sampling)`.
- [ ] Add `draw_image_src(image, src, dst, paint, sampling)`.
- [ ] Add `draw_surface(surface, x, y, paint)`.
- [ ] Add `SamplingMode::{Nearest, Linear, Mipmapped}`.

Tests:

- [ ] Path fill rules render different pixels for `nonzero` vs `evenodd`.
- [ ] Rounded clipping masks image corners.
- [ ] `save_layer()` applies opacity/filter/blend only to the isolated layer.
- [ ] `plus-lighter` accumulation does not clamp on F16/F32 surfaces.
- [ ] `draw_image_src()` crops the expected source rect.
- [ ] `draw_image(..., SamplingMode::Nearest)` keeps hard edges for preprocessed Citra output.

## Chunk 4: Filters, Color Filters, And Shaders

### Task 7: Add filter facade.

**Files:**

- Create: `src/native/filter.rs`.
- Create: `src/native/shader.rs`.
- Modify: `src/native/mod.rs`.
- Modify: `src/native/paint.rs`.

Steps:

- [ ] Add `NativeImageFilter`.
- [ ] Add `NativeColorFilter`.
- [ ] Add `NativeImageFilter::blur(sigma_x, sigma_y, input)`.
- [ ] Add `NativeImageFilter::drop_shadow(dx, dy, sigma_x, sigma_y, color, input)`.
- [ ] Add `NativeImageFilter::color_matrix(matrix_4x5, input)`.
- [ ] Add `NativeColorFilter::luma()`.
- [ ] Add `NativeColorFilter::srgb_to_linear_gamma()`.
- [ ] Add `NativeColorFilter::linear_to_srgb_gamma()`.
- [ ] Add `NativeColorFilter::compose(outer, inner)`.
- [ ] Add `NativeImageFilter::from_color_filter(color_filter, input)`.
- [ ] Add `NativeImageFilter::compose(outer, inner)`.
- [ ] Add `NativeShader::linear_gradient(start, end, stops, interpolation)`.

Tests:

- [ ] Blur expands/softens non-transparent pixels.
- [ ] Drop shadow produces offset pixels.
- [ ] Color matrix can replace RGB for ID-buffer rendering while thresholding alpha.
- [ ] Luma color filter works with `destination-in`/`destination-out` mask paths.
- [ ] Linear gradient renders sorted color stops.
- [ ] `oklch` interpolation is either implemented or rejected with a typed error. Do not silently use sRGB if the caller requested `oklch`.

## Chunk 5: Images, SVG, And Raw Decoded Frames

### Task 8: Add raw image creation.

**Files:**

- Modify: `src/native/image.rs`.
- Modify: `src/native/pixels.rs`.
- Modify: `src/native/error.rs`.

Steps:

- [ ] Add `NativeImage::from_pixels(bytes, width, height, stride, pixel_format, color_space)`.
- [ ] Add a borrowed-slice constructor if it is safe; otherwise document the copy and keep owned data.
- [ ] Validate stride, dimensions, byte length, alpha mode, and color space.
- [ ] Support RGBA8 premul/unpremul, RGBA16F premul, and RGBA32F premul.
- [ ] Use this as the intended bridge for rsmpeg decoded video frames and Citra-generated images.

Tests:

- [ ] RGBA8 raw image draws without PNG/JPEG re-encode.
- [ ] F16/F32 raw image preserves HDR values until SDR export.
- [ ] Invalid stride returns a typed error.
- [ ] Alpha mode is explicit and tested.

### Task 9: Pin SVG behavior.

**Files:**

- Modify: `src/native/image.rs`.
- Modify: `src/native/path.rs`.
- Create fixtures if needed under `tests/fixtures/`.

Steps:

- [ ] Add a contract test for `NativePath::from_svg()` using the same path-data form as `ShapeItem.pathData`.
- [ ] Add a contract test for loading SVG XML bytes through `NativeImage::from_encoded()`.
- [ ] If encoded SVG image decode does not match the JS backend, add a named `NativeImage::from_svg_xml(svg, width, height)` API instead of relying on ambiguous codec behavior.

## Chunk 6: Text And Fonts

### Task 10: Add Rust-facing font manager.

**Files:**

- Create: `src/native/font.rs`.
- Modify: `src/native/text.rs`.
- Modify: `src/native/mod.rs`.

Steps:

- [ ] Add `NativeFontManager` or `NativeFontLibrary`.
- [ ] Support `register_font_from_path(family, paths)`.
- [ ] Support `register_font_from_data(family, bytes)`.
- [ ] Support `has_font(family)` and `families()`.
- [ ] Use owned state where possible. If shared global state is required, wrap it with `parking_lot::Mutex`, not `RefCell`.
- [ ] Preserve existing JS `FontLibrary` behavior.

### Task 11: Add paragraph layout facade.

**Files:**

- Modify: `src/native/text.rs`.

Steps:

- [ ] Add `TextStyle` with font family list, font size, font weight, slant, linear color, align, line-height multiplier, letter spacing, word spacing, OpenType features, variation settings, decoration, shadows, and baseline shift.
- [ ] Add `ParagraphStyle`.
- [ ] Add `RichTextSpan`.
- [ ] Add `NativeTextEngine::layout_text(text, style, max_width)`.
- [ ] Add `NativeTextEngine::layout_rich_text(spans, base_style, max_width)`.
- [ ] Add `NativeTextLayout` with width, height, line count, first-line ascent, line metrics, and rects-for-range.
- [ ] Add `NativeCanvas::draw_text_layout(layout, x, y)`.
- [ ] Keep `draw_text_box()` as a convenience API if still useful, but do not make Studio rely on it.

Tests:

- [ ] Simple text draws visible pixels.
- [ ] Center/right alignment uses max width.
- [ ] Wrapping changes line count and height.
- [ ] Rich spans preserve per-span color, weight, letter spacing, and baseline shift.
- [ ] `get_rects_for_range()` supports baseline-shift overlay placement.
- [ ] Registered fonts are used before generic fallbacks.
- [ ] Variable font weight can pass through as a non-integer value when supported.

## Chunk 7: Studio Renderer Contract Adapter Proof

### Task 12: Add a local Rust adapter test in p-s-c.

**Files:**

- Create: `tests/native_studio_renderer_adapter.rs`.

Steps:

- [ ] Write a small Rust-only adapter that mirrors the TypeScript `DrawBackend` contract names locally inside the test.
- [ ] Render a shape, image, text, mask, filter, and offscreen composition through that adapter.
- [ ] Assert the adapter never imports `skia_safe`.
- [ ] Assert no PNG/JPEG/WebP encode is used in the hot path.

This test is intentionally in p-s-c so API gaps are caught before Studio tries to migrate.

### Task 13: Add downstream Studio proof after p-s-c lands.

**Files in Studio desktop worktree:**

- `/home/moritz/.config/superpowers/worktrees/studio/desktop-app/rust/crates/studio-render-native/Cargo.toml`.
- `/home/moritz/.config/superpowers/worktrees/studio/desktop-app/rust/crates/studio-render-native/src/**/*.rs`.

Steps:

- [ ] Replace direct `skia-safe` dependency with `phyron-skia-canvas`.
- [ ] Remove direct `skia_safe` imports from `studio-render-native`.
- [ ] Implement Studio renderer calls using the new native facade.
- [ ] Keep color-space identifiers aligned with the v3 contract: default linear sRGB, plus linear Display P3 and linear Rec.2020.
- [ ] Keep internal alpha premultiplied.
- [ ] Use `NativeImage::from_pixels()` for rsmpeg decoded frames.
- [ ] Run the existing Studio preview parity suite with `native-video`/`native-video-rsmpeg`.

Verification in Studio:

```bash
cargo test -p studio-render-native --features native-video-rsmpeg
cargo clippy -p studio-render-native --all-targets --features native-video-rsmpeg -- -D warnings
```

## Chunk 8: Documentation And Quality Gates

### Task 14: Document the Rust API.

**Files:**

- Modify: `README.md`.
- Modify or add: `docs/api/native-rust.md`.
- Modify: `docs/superpowers/plans/2026-05-01-native-rust-api-boundary.md`.

Steps:

- [ ] Document that `phyron_skia_canvas::native` is the only supported Rust consumer API.
- [ ] Document that `skia_safe` remains a private implementation detail.
- [ ] Document linear working spaces clearly: `Srgb`, `DisplayP3`, and `Rec2020` are all linear-light spaces with their own primaries. They are not aliases for linear sRGB.
- [ ] Document that HDR channel values above `1.0` are valid internally.
- [ ] Document premultiplied alpha as the internal Studio convention.
- [ ] Document which readback modes return unpremultiplied pixels for external APIs like `putImageData`.

### Task 15: Final verification.

Run:

```bash
just fmt-check
just check
just lint-check
cargo test --all-features --test native_api_contract
cargo test --all-features --test native_studio_renderer_contract
cargo test --all-features --test native_studio_renderer_adapter
```

If `lib/skia.node` exists or a build is acceptable in the current environment, also run:

```bash
just test
```

Do not run visual baseline update commands.

## Recommended Execution Order

1. Land Chunks 1-2 first. Without a real `NativeSurface` and pixel IO, the rest cannot prove Studio parity.
2. Land Chunks 3-4 next. This unlocks masks, effects, blend modes, gradients, ID buffer, and shape parity.
3. Land Chunk 5 before rsmpeg integration. Direct decoded-frame upload is the practical reason to avoid shelling out/PNG round-trips.
4. Land Chunk 6 before claiming editor preview parity for text-heavy projects.
5. Land Chunk 7 as the migration go/no-go. If the adapter cannot be written without `skia_safe`, the facade is still incomplete.
6. Only then switch `studio-render-native` from `skia-safe` to `phyron-skia-canvas`.

## Decision

Desktop-app should not switch from `skia-safe` to `phyron-skia-canvas` yet.

The current p-s-c native facade is good as a boundary proof, but it does not cover the required `@phyron/studio-renderer` surface. Switching now would either lose renderer behavior or force Studio to keep private `skia_safe` escape hatches. The correct next work is to close the p-s-c native API gaps above, then migrate Studio in one bounded downstream proof.
