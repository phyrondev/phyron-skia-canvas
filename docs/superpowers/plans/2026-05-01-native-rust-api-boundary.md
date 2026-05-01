# Native Rust API Boundary Implementation Plan

> **For agentic workers:** REQUIRED: Use `superpowers:subagent-driven-development` (if subagents available) or `superpowers:executing-plans` to implement this plan. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Expose a stable Rust-only API from `phyron-skia-canvas` so `studio-render-native` can stop depending on `skia-safe` directly.

**Architecture:** Add a new `phyron_skia_canvas::native` facade that wraps Skia internally but exposes only Studio-shaped Rust types. Keep the existing Neon/JS API intact. Migrate Studio only after the new facade proves the shape, image, text, color-space, and raw-frame readback contracts.

**Tech Stack:** Rust 2024, `skia-safe` private internals, `phyron-skia-canvas` rlib, Neon existing JS binding, `just`, linked `.blueprints` submodule.

---

## Status And Approval

API changes are approved by Moritz on 2026-05-01 for this specific scope:

- Add `phyron_skia_canvas::native`.
- Add public Rust-only types/methods needed by Studio.
- Keep existing JS/Neon API behavior compatible.
- Do not remove or rename existing public modules in this pass unless Moritz explicitly approves a second breaking cleanup.

### Implementation status (2026-05-01)

- Chunks 1-5 and 7 (Tasks 1-8, 11-12): **complete** in this repo on branch `plan/native-rust-api-boundary`.
- Chunk 6 (Tasks 9-10, Studio consumer proof): **not started** -- happens in `studio/desktop-app` after this PR lands.

Verification on the implementation branch:

- `just fmt-check` passes.
- `just check` passes.
- `just lint-check` passes (zero warnings).
- `just build` produces `lib/skia.node`.
- `just test` -- 146 pass / 1 skip / 0 fail (Node JS suite).
- `cargo test --features "vulkan,window,freetype" --test native_api_contract` -- 5/5 pass.
- Audit `rg -n "pub .*skia_safe|pub .*FunctionContext|pub .*JsBox|pub .*Handle<|pub .*RefCell" src/native` -- no output.
- Audit `rg -n "\.unwrap\(|\.expect\(|panic!|todo!|unimplemented!" src/native tests/native_api_contract.rs` -- no output.

This plan branch includes the blueprint integration locally. Before implementing, make sure `.blueprints` is populated:

```bash
git submodule update --init .blueprints
```

Read these files before editing:

- `AGENTS.md`
- `.blueprints/base/AGENTS.md`
- `.blueprints/base/api-changes.md`
- `.blueprints/base/test-ownership.md`
- `.blueprints/lang/rust/AGENTS.md`
- `.blueprints/lang/rust/testing.md`

Do not edit `.blueprints` as part of this work.

## Current Problem

`phyron-skia-canvas` is now usable as an `rlib`, but the public Rust surface is not yet a Rust API boundary:

- `src/lib.rs` exposes many JS/Neon modules directly (`canvas`, `context`, `utils`, `paragraph`, etc.).
- Public signatures leak `neon` types such as `FunctionContext`, `JsBox`, and `Handle`.
- Public signatures leak `skia_safe` types such as `Canvas`, `ImageInfo`, `ColorSpace`, `ColorType`, `AlphaType`, `Paint`, `FontMgr`, and `Image`.
- `PageRecorder::append` exposes `FnOnce(&skia_safe::Canvas)`, which forces Studio to import `skia_safe`.
- Studio currently imports `skia_safe` directly in `studio-render-native` item renderers and color code.
- Existing `utils::to_color_space` and `utils::to_color_type` silently fall back to sRGB/RGBA8888 on unknown strings. That is acceptable for the historical JS compatibility path, but not for Studio's typed Rust renderer boundary.

The target state is:

- `studio-render-native` depends on `phyron-skia-canvas` only, not `skia-safe`.
- `skia-safe` remains an implementation detail inside `phyron-skia-canvas`.
- Studio can request linear-light sRGB, Display P3, and Rec.2020 working spaces without collapsing to linear sRGB.
- Studio can render into raw buffers without PNG/JPEG/WebP encoding.
- Existing Node consumers keep working.

## Non-Goals

- Do not port Studio's whole renderer into `phyron-skia-canvas`.
- Do not remove the existing Neon API.
- Do not regenerate visual baselines.
- Do not run `cargo clean`.
- Do not change Skia version.
- Do not add GPU shared-buffer transport here.
- Do not implement Citra integration here.

## File Structure

Create these files:

- `src/native/mod.rs` -- public facade exports and module-level docs.
- `src/native/error.rs` -- `NativeError`.
- `src/native/geometry.rs` -- `Point`, `Size`, `Rect`.
- `src/native/color.rs` -- `LinearColorSpace`, `OutputColorSpace`, `RgbaLinear`, strict Skia conversion helpers.
- `src/native/pixels.rs` -- `PixelFormat`, `AlphaMode`, `SurfaceOptions`, `RawFrameOptions`, `RawFrame`.
- `src/native/paint.rs` -- `ShapePaint`, stroke/fill options.
- `src/native/image.rs` -- `NativeImage`, encoded image decode, raw RGBA image creation if needed.
- `src/native/text.rs` -- simple text box options, alignment, typeface lookup wrapper.
- `src/native/recorder.rs` -- `NativeRecorder`, `NativeCanvas`, raw readback.
- `tests/native_api_contract.rs` -- Rust consumer contract tests.

Modify these files:

- `src/lib.rs` -- add `pub mod native;` and an `AIDEV-NOTE` stating `native` is the Rust consumer API.
- `src/context/page.rs` -- add any minimal internal helper needed by `native::recorder`, but do not widen Skia leaks.
- `README.md` -- add a short "Rust library consumers" section pointing at `phyron_skia_canvas::native`.

Later Studio proof modifies the separate repo:

- `/home/moritz/.config/superpowers/worktrees/studio/desktop-app/rust/crates/studio-render-native/Cargo.toml`
- `/home/moritz/.config/superpowers/worktrees/studio/desktop-app/rust/crates/studio-render-native/src/color.rs`
- `/home/moritz/.config/superpowers/worktrees/studio/desktop-app/rust/crates/studio-render-native/src/preview/mod.rs`
- `/home/moritz/.config/superpowers/worktrees/studio/desktop-app/rust/crates/studio-render-native/src/preview/items/*.rs`
- `/home/moritz/.config/superpowers/worktrees/studio/desktop-app/rust/crates/studio-render-native/src/preview/space.rs`

## API Shape

The new API must look like this to consumers:

```rust
use phyron_skia_canvas::native::{
    LinearColorSpace, NativeImage, NativeRecorder, PixelFormat, Point, RawFrameOptions, Rect,
    RgbaLinear, ShapePaint, SurfaceOptions,
};

let bounds = Rect::from_xywh(0.0, 0.0, 1920.0, 1080.0);
let mut recorder = NativeRecorder::new(bounds)?;
recorder.record(|canvas| {
    canvas.clear(RgbaLinear::opaque(0.0, 0.0, 0.0));
    canvas.save();
    canvas.translate(Point::new(100.0, 100.0));
    canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 200.0, 100.0), &ShapePaint::fill(RgbaLinear::opaque(1.0, 0.0, 0.0)));
    canvas.restore();
});

let frame = recorder.render_raw(
    SurfaceOptions {
        color_space: LinearColorSpace::DisplayP3,
        ..SurfaceOptions::default()
    },
    RawFrameOptions {
        pixel_format: PixelFormat::Rgba8UnormUnpremul,
        ..RawFrameOptions::default()
    },
)?;

assert_eq!(frame.stride(), frame.width() * 4);
```

Public `native` signatures must not contain:

- `skia_safe`
- `neon`
- `FunctionContext`
- `JsBox`
- `Handle<...>`
- `RefCell`

Internal implementation may use these where appropriate.

## Public Types

Implement the following public types. Derives must follow `.blueprints/lang/rust/AGENTS.md`: public types implement `Debug`, `Clone`, `Hash`, `PartialEq`, `Eq` where possible, and `Copy` where trivially derivable.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinearColorSpace {
    Srgb,
    DisplayP3,
    Rec2020,
}
```

`LinearColorSpace::DisplayP3` means linear-light Display P3 primaries. It does not mean gamma-coded Display P3.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputColorSpace {
    Srgb,
    DisplayP3,
    Rec2020,
}
```

`OutputColorSpace` is for wire/readback conversion. `RawFrameOptions::default()` should use `OutputColorSpace::Srgb` because `HTMLCanvasElement.putImageData` expects sRGB-encoded 8-bit pixels.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    Rgba8UnormPremul,
    Rgba8UnormUnpremul,
    Rgba16fPremul,
    Rgba32fPremul,
}
```

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbaLinear {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
```

`RgbaLinear` is always premultiplied linear-light RGBA. Values above `1.0` are allowed for HDR.

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
```

Do not use `fn_params_excessive_bools` patterns. Prefer enums/options structs.

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum NativeError {
    InvalidDimensions { width: f32, height: f32 },
    InvalidRect { rect: Rect },
    UnsupportedColorSpace { color_space: LinearColorSpace },
    UnsupportedOutputColorSpace { color_space: OutputColorSpace },
    UnsupportedPixelFormat { pixel_format: PixelFormat },
    DecodeImage { reason: String },
    Render { reason: String },
}
```

Implement `Display` and `std::error::Error` for `NativeError`.

## Chunk 1: Native Module Skeleton And Contract Tests

### Task 1: Add the compile-contract test.

**Files:**

- Create: `tests/native_api_contract.rs`

- [x] **Step 1: Write the failing compile/runtime contract.**

```rust
use phyron_skia_canvas::native::{
    LinearColorSpace, NativeRecorder, PixelFormat, RawFrameOptions, Rect, RgbaLinear,
    ShapePaint, SurfaceOptions,
};

#[test]
fn native_facade_renders_tight_rgba8_without_importing_skia_safe() {
    let mut recorder = NativeRecorder::new(Rect::from_xywh(0.0, 0.0, 8.0, 8.0))
        .expect("recorder dimensions are valid");

    recorder.record(|canvas| {
        canvas.clear(RgbaLinear::opaque(0.0, 0.0, 0.0));
        canvas.draw_rect(
            Rect::from_xywh(2.0, 2.0, 4.0, 4.0),
            &ShapePaint::fill(RgbaLinear::opaque(1.0, 0.0, 0.0)),
        );
    });

    let frame = recorder
        .render_raw(
            SurfaceOptions {
                color_space: LinearColorSpace::Srgb,
                ..SurfaceOptions::default()
            },
            RawFrameOptions {
                pixel_format: PixelFormat::Rgba8UnormUnpremul,
                ..RawFrameOptions::default()
            },
        )
        .expect("raw readback succeeds");

    assert_eq!(frame.width(), 8);
    assert_eq!(frame.height(), 8);
    assert_eq!(frame.stride(), 32);
    assert_eq!(frame.pixels().len(), 8 * 32);
    assert!(frame.pixels().iter().any(|channel| *channel != 0));
}
```

The test intentionally does not import `skia_safe`.

- [x] **Step 2: Run the test and verify it fails.**

```bash
cargo test --no-default-features native_facade_renders_tight_rgba8_without_importing_skia_safe
```

Expected: FAIL to compile because `phyron_skia_canvas::native` does not exist.

### Task 2: Add module skeleton.

**Files:**

- Create: `src/native/mod.rs`
- Create: `src/native/error.rs`
- Create: `src/native/geometry.rs`
- Create: `src/native/color.rs`
- Create: `src/native/pixels.rs`
- Create: `src/native/paint.rs`
- Create: `src/native/recorder.rs`
- Modify: `src/lib.rs`

- [x] **Step 1: Add public module export.**

In `src/lib.rs`, add:

```rust
// AIDEV-NOTE: Rust library consumers should use `native`.
// The older public modules below are the JS/Neon compatibility surface.
pub mod native;
```

Keep existing module exports unchanged.

- [x] **Step 2: Add public type shells.**

Implement the types from "Public Types" with methods:

```rust
impl Rect {
    pub fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self;
    pub fn width(&self) -> f32;
    pub fn height(&self) -> f32;
    pub fn is_empty(&self) -> bool;
}

impl RgbaLinear {
    pub fn new_premultiplied(r: f32, g: f32, b: f32, a: f32) -> Self;
    pub fn opaque(r: f32, g: f32, b: f32) -> Self;
    pub fn with_opacity(self, opacity: f32) -> Self;
}
```

No `get_` prefixes.

- [x] **Step 3: Add minimal `NativeRecorder` stubs.**

```rust
pub struct NativeRecorder {
    // private fields only
}

pub struct NativeCanvas<'a> {
    // private fields only
}
```

The stubs can return `NativeError::Render { reason: "not implemented".into() }` until Chunk 2.

- [x] **Step 4: Run the test and verify the failure moved.**

```bash
cargo test --no-default-features native_facade_renders_tight_rgba8_without_importing_skia_safe
```

Expected: compile succeeds, runtime fails with the temporary "not implemented" error.

- [x] **Step 5: Commit.**

```bash
git add src/lib.rs src/native tests/native_api_contract.rs
git commit -m "Add native Rust facade skeleton."
```

## Chunk 2: Strict Color And Pixel Conversion

### Task 3: Implement strict Skia conversion helpers.

**Files:**

- Modify: `src/native/color.rs`
- Modify: `src/native/pixels.rs`
- Test: `tests/native_api_contract.rs`

- [x] **Step 1: Add color-space tests.**

Add:

```rust
#[test]
fn native_facade_constructs_required_linear_working_spaces() {
    for color_space in [
        LinearColorSpace::Srgb,
        LinearColorSpace::DisplayP3,
        LinearColorSpace::Rec2020,
    ] {
        let mut recorder = NativeRecorder::new(Rect::from_xywh(0.0, 0.0, 4.0, 4.0))
            .expect("recorder dimensions are valid");
        recorder.record(|canvas| canvas.clear(RgbaLinear::opaque(0.25, 0.5, 1.5)));
        let frame = recorder
            .render_raw(
                SurfaceOptions {
                    color_space,
                    ..SurfaceOptions::default()
                },
                RawFrameOptions::default(),
            )
            .expect("required linear color space renders");
        assert_eq!(frame.width(), 4);
        assert_eq!(frame.height(), 4);
    }
}
```

- [x] **Step 2: Implement private conversion helpers.**

In `src/native/color.rs`, implement private methods:

```rust
impl LinearColorSpace {
    pub(crate) fn to_skia_color_space(self) -> Result<skia_safe::ColorSpace, NativeError>;
}

impl OutputColorSpace {
    pub(crate) fn to_skia_color_space(self) -> Result<skia_safe::ColorSpace, NativeError>;
}
```

Implementation requirements:

- `LinearColorSpace::Srgb` -> `ColorSpace::new_srgb_linear()`.
- `LinearColorSpace::DisplayP3` -> `ColorSpace::new_cicp(DisplayP3 primaries, Linear transfer)`.
- `LinearColorSpace::Rec2020` -> `ColorSpace::new_cicp(Rec2020 primaries, Linear transfer)`.
- Return `NativeError::UnsupportedColorSpace` if `new_cicp` returns `None`.
- Do not silently fallback to sRGB in the new native API.
- Existing `utils::to_color_space` may remain fallback-based for JS compatibility.

- [x] **Step 3: Implement pixel format conversions.**

In `src/native/pixels.rs`, implement private helpers:

```rust
impl PixelFormat {
    pub(crate) fn to_skia_color_type(self) -> Result<skia_safe::ColorType, NativeError>;
    pub(crate) fn to_skia_alpha_type(self) -> skia_safe::AlphaType;
    pub fn bytes_per_pixel(self) -> usize;
}
```

Map:

- `Rgba8UnormPremul` -> `RGBA8888`, `Premul`, 4.
- `Rgba8UnormUnpremul` -> `RGBA8888`, `Unpremul`, 4.
- `Rgba16fPremul` -> `RGBAF16`, `Premul`, 8.
- `Rgba32fPremul` -> `RGBAF32`, `Premul`, 16.

- [x] **Step 4: Run tests.**

```bash
cargo test --no-default-features native_facade_constructs_required_linear_working_spaces
```

Expected: pass once Chunk 3 render path is implemented; if Chunk 3 is not implemented yet, this may still fail at render-time. Do not skip it.

- [x] **Step 5: Commit.**

```bash
git add src/native/color.rs src/native/pixels.rs tests/native_api_contract.rs
git commit -m "Add strict native color and pixel contracts."
```

## Chunk 3: Recorder And Raw Readback

### Task 4: Wrap `PageRecorder` without leaking Skia.

**Files:**

- Modify: `src/native/recorder.rs`
- Modify: `src/context/page.rs` only if needed
- Test: `tests/native_api_contract.rs`

- [x] **Step 1: Implement `NativeRecorder::new`.**

Requirements:

- Reject empty or negative bounds with `NativeError::InvalidDimensions`.
- Internally create `context::page::PageRecorder`.
- Keep `skia_safe::Rect` private.

- [x] **Step 2: Implement `NativeRecorder::record`.**

The public closure receives `&mut NativeCanvas`, never `&skia_safe::Canvas`.

Suggested internal implementation:

```rust
pub fn record(&mut self, f: impl FnOnce(&mut NativeCanvas<'_>)) {
    self.recorder.append(|skia_canvas| {
        let mut canvas = NativeCanvas::new(skia_canvas);
        f(&mut canvas);
    });
}
```

`NativeCanvas::new` must be private.

- [x] **Step 3: Implement `NativeCanvas::clear`.**

Use `skia_safe::Color4f` internally. Preserve premultiplied alpha from `RgbaLinear`.

- [x] **Step 4: Implement `NativeRecorder::render_raw`.**

Requirements:

- Build surface options from `SurfaceOptions`.
- Build destination `ImageInfo` from `RawFrameOptions`.
- Call the existing raw page readback seam.
- Return `RawFrame { width, height, stride, pixel_format, color_space, pixels }`.
- `RawFrame::pixels(&self) -> &[u8]`.
- `RawFrame::into_pixels(self) -> Vec<u8>`.
- Do not encode PNG/JPEG/WebP.

If `Page::render_raw` cannot be used without exposing `skia_safe::ImageInfo` publicly, keep that type inside `native::recorder` and pass it internally.

- [x] **Step 5: Run the first contract test.**

```bash
cargo test --no-default-features native_facade_renders_tight_rgba8_without_importing_skia_safe
```

Expected: pass.

- [x] **Step 6: Commit.**

```bash
git add src/native/recorder.rs src/native/pixels.rs src/native/color.rs src/context/page.rs tests/native_api_contract.rs
git commit -m "Render raw frames through native Rust facade."
```

## Chunk 4: Shape, Image, And Text Surface

### Task 5: Add shape drawing needed by Studio.

**Files:**

- Modify: `src/native/paint.rs`
- Modify: `src/native/recorder.rs`
- Test: `tests/native_api_contract.rs`

- [x] **Step 1: Add shape paint tests.**

Add a test that draws a fill-only rectangle, a stroked rounded rect, and an oval, then asserts non-background pixels exist in expected rough regions.

Do not create/update visual baselines.

- [x] **Step 2: Implement `ShapePaint`.**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ShapePaint {
    pub fill: Option<RgbaLinear>,
    pub stroke: Option<RgbaLinear>,
    pub stroke_width: f32,
    pub anti_alias: bool,
}
```

Methods:

```rust
impl ShapePaint {
    pub fn fill(color: RgbaLinear) -> Self;
    pub fn stroke(color: RgbaLinear, width: f32) -> Self;
    pub fn fill_and_stroke(fill: RgbaLinear, stroke: RgbaLinear, width: f32) -> Self;
}
```

- [x] **Step 3: Implement canvas methods.**

```rust
impl NativeCanvas<'_> {
    pub fn save(&mut self);
    pub fn restore(&mut self);
    pub fn translate(&mut self, point: Point);
    pub fn rotate_degrees(&mut self, degrees: f32, pivot: Option<Point>);
    pub fn draw_rect(&mut self, rect: Rect, paint: &ShapePaint);
    pub fn draw_rounded_rect(&mut self, rect: Rect, radius: f32, paint: &ShapePaint);
    pub fn draw_oval(&mut self, rect: Rect, paint: &ShapePaint);
}
```

Do not expose `skia_safe::Paint`, `skia_safe::Rect`, or `skia_safe::Point`.

- [x] **Step 4: Run shape tests.**

```bash
cargo test --no-default-features native_facade_draws_shapes
```

Expected: pass.

- [x] **Step 5: Commit.**

```bash
git add src/native/paint.rs src/native/recorder.rs tests/native_api_contract.rs
git commit -m "Add shape drawing to native Rust facade."
```

### Task 6: Add encoded image drawing.

**Files:**

- Modify: `src/native/image.rs`
- Modify: `src/native/recorder.rs`
- Test: `tests/native_api_contract.rs`

- [x] **Step 1: Add encoded image test.**

Use one existing small image from `tests/assets`, for example `tests/assets/pentagon.png`.

```rust
#[test]
fn native_facade_decodes_and_draws_encoded_image() {
    let bytes = std::fs::read("tests/assets/pentagon.png").expect("fixture exists");
    let image = NativeImage::from_encoded(&bytes).expect("fixture decodes");
    assert!(image.width() > 0);
    assert!(image.height() > 0);

    let mut recorder = NativeRecorder::new(Rect::from_xywh(0.0, 0.0, 32.0, 32.0))
        .expect("recorder dimensions are valid");
    recorder.record(|canvas| {
        canvas.clear(RgbaLinear::opaque(0.0, 0.0, 0.0));
        canvas.draw_image_rect(&image, Rect::from_xywh(0.0, 0.0, 32.0, 32.0), 1.0);
    });

    let frame = recorder
        .render_raw(SurfaceOptions::default(), RawFrameOptions::default())
        .expect("raw readback succeeds");
    assert!(frame.pixels().iter().any(|channel| *channel != 0));
}
```

- [x] **Step 2: Implement `NativeImage`.**

`NativeImage` wraps `skia_safe::Image` privately.

Public methods:

```rust
impl NativeImage {
    pub fn from_encoded(bytes: &[u8]) -> Result<Self, NativeError>;
    pub fn width(&self) -> u32;
    pub fn height(&self) -> u32;
}
```

- [x] **Step 3: Implement `draw_image_rect`.**

```rust
impl NativeCanvas<'_> {
    pub fn draw_image_rect(&mut self, image: &NativeImage, dst: Rect, opacity: f32);
}
```

Opacity must modulate alpha only. Preserve Studio's premultiplied alpha assumptions.

- [x] **Step 4: Run image test.**

```bash
cargo test --no-default-features native_facade_decodes_and_draws_encoded_image
```

Expected: pass.

- [x] **Step 5: Commit.**

```bash
git add src/native/image.rs src/native/recorder.rs tests/native_api_contract.rs
git commit -m "Add encoded image drawing to native Rust facade."
```

### Task 7: Add simple text drawing.

**Files:**

- Modify: `src/native/text.rs`
- Modify: `src/native/recorder.rs`
- Test: `tests/native_api_contract.rs`

- [x] **Step 1: Add text test.**

```rust
#[test]
fn native_facade_draws_visible_text_pixels() {
    let mut recorder = NativeRecorder::new(Rect::from_xywh(0.0, 0.0, 128.0, 64.0))
        .expect("recorder dimensions are valid");
    recorder.record(|canvas| {
        canvas.clear(RgbaLinear::opaque(0.0, 0.0, 0.0));
        canvas.draw_text_box(
            "Studio",
            Rect::from_xywh(4.0, 4.0, 120.0, 56.0),
            &TextBoxOptions {
                color: RgbaLinear::opaque(1.0, 1.0, 1.0),
                font_size: 32.0,
                ..TextBoxOptions::default()
            },
        );
    });
    let frame = recorder
        .render_raw(SurfaceOptions::default(), RawFrameOptions::default())
        .expect("raw readback succeeds");
    assert!(frame.pixels().iter().any(|channel| *channel > 32));
}
```

- [x] **Step 2: Implement text options.**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TextBoxOptions {
    pub color: RgbaLinear,
    pub font_family: Option<String>,
    pub font_size: f32,
    pub font_weight: i32,
    pub horizontal_align: TextAlign,
    pub vertical_align: VerticalAlign,
    pub opacity: f32,
}
```

Enums:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerticalAlign {
    Top,
    Center,
    Bottom,
}
```

- [x] **Step 3: Implement `draw_text_box`.**

Use Skia internally. This can start with the same primitive font API Studio uses today. Do not expose `FontMgr`, `Typeface`, `Font`, or paragraph internals.

Future paragraph/rich text support can be added in a later API expansion.

- [x] **Step 4: Run text test.**

```bash
cargo test --no-default-features native_facade_draws_visible_text_pixels
```

Expected: pass.

- [x] **Step 5: Commit.**

```bash
git add src/native/text.rs src/native/recorder.rs tests/native_api_contract.rs
git commit -m "Add simple text drawing to native Rust facade."
```

## Chunk 5: Binding Hygiene And Documentation

### Task 8: Document the public Rust boundary and quality rules.

**Files:**

- Modify: `README.md`
- Modify: `src/lib.rs`
- Optional create: `docs/rust-native-api.md`

- [x] **Step 1: Add README section.**

Add a concise section:

```markdown
## Rust Library Consumers

Rust consumers should use `phyron_skia_canvas::native`. That facade is the stable Rust API and intentionally hides Neon and `skia-safe` types. The older public modules exist for Node/Neon compatibility and are not the preferred API for new Rust consumers.
```

- [x] **Step 2: Add boundary note to `src/lib.rs`.**

Use `AIDEV-NOTE`, per blueprints:

```rust
// AIDEV-NOTE: `native` is the Rust consumer API. Keep Neon and raw
// `skia_safe` types out of its public signatures so downstream Rust
// crates do not inherit the binding internals.
```

- [x] **Step 3: Audit new public signatures.**

Run:

```bash
rg -n "pub .*skia_safe|pub .*FunctionContext|pub .*JsBox|pub .*Handle<|pub .*RefCell" src/native
```

Expected: no output.

This command is not sufficient by itself because multiline signatures can evade it. Also inspect `src/native/*.rs` manually.

- [x] **Step 4: Audit new panics/unwraps.**

Run:

```bash
rg -n "\\.unwrap\\(|\\.expect\\(|panic!|todo!|unimplemented!" src/native tests/native_api_contract.rs
```

Expected: no output, except test `expect(...)` calls may remain if they are ordinary test setup assertions. Production `src/native` must have no undocumented panic path.

- [x] **Step 5: Commit.**

```bash
git add README.md src/lib.rs docs/rust-native-api.md
git commit -m "Document native Rust facade boundary."
```

## Chunk 6: Studio Consumer Proof

This chunk happens in `/home/moritz/.config/superpowers/worktrees/studio/desktop-app` after `phyron-skia-canvas` has the new facade.

### Task 9: Point Studio at the local `phyron-skia-canvas` facade.

**Files:**

- Modify: `/home/moritz/.config/superpowers/worktrees/studio/desktop-app/rust/crates/studio-render-native/Cargo.toml`
- Modify: `/home/moritz/.config/superpowers/worktrees/studio/desktop-app/Cargo.toml` only if a `[patch]` is needed for local verification

- [ ] **Step 1: Replace direct `skia-safe` dependency.**

In `studio-render-native/Cargo.toml`, remove:

```toml
"dep:skia-safe",
skia-safe = { version = "0.93.1", features = ["textlayout", "webp", "svg"], optional = true }
```

Keep `phyron-skia-canvas` optional.

- [ ] **Step 2: Use local path for verification.**

Use a temporary local dependency while verifying:

```toml
phyron-skia-canvas = { path = "/home/moritz/code/crates/phyron-skia-canvas-rust-api-plan", default-features = false, optional = true }
```

Do not commit this path if the final Studio branch should consume a git revision instead. If committing a git revision, point to the branch/commit that contains the facade.

### Task 10: Migrate Studio imports.

**Files:**

- Modify: `rust/crates/studio-render-native/src/color.rs`
- Modify: `rust/crates/studio-render-native/src/preview/mod.rs`
- Modify: `rust/crates/studio-render-native/src/preview/items/shape.rs`
- Modify: `rust/crates/studio-render-native/src/preview/items/image.rs`
- Modify: `rust/crates/studio-render-native/src/preview/items/video.rs`
- Modify: `rust/crates/studio-render-native/src/preview/items/text.rs`
- Modify: `rust/crates/studio-render-native/src/preview/items/mod.rs`
- Modify: `rust/crates/studio-render-native/src/preview/space.rs`

- [ ] **Step 1: Replace `skia_safe` imports with `phyron_skia_canvas::native`.**

Target command:

```bash
rg -n "skia_safe|skia-safe" rust/crates/studio-render-native/src rust/crates/studio-render-native/Cargo.toml
```

Expected after migration: no production hits. Test helpers may use image fixtures, but avoid `skia_safe` there too if practical.

- [ ] **Step 2: Keep Studio's renderer semantics unchanged.**

Maintain these contracts:

- Internal compositing is linear-light.
- Supported working spaces are linear sRGB, linear Display P3, linear Rec.2020.
- Alpha inside Studio is premultiplied.
- HDR values above `1.0` survive float compositing.
- The desktop preview transport still requests tight `Rgba8UnormUnpremul` for `putImageData`.
- No PNG/JPEG/WebP encoding on the preview hot path.

- [ ] **Step 3: Run Studio checks.**

```bash
cargo test -p studio-render-native --features native-skia
cargo test -p studio-render-native --features native-video
cargo clippy -p studio-render-native --all-targets --features native-video -- -D warnings
```

Expected: pass.

If vcpkg/rsmpeg work is in-flight and blocks `native-video`, run `native-skia` first and document the `native-video` blocker.

## Chunk 7: Final Verification

### Task 11: Verify `phyron-skia-canvas`.

**Files:** none, unless fixes are needed.

- [x] **Step 1: Run formatting.**

```bash
just fmt-check
```

Expected: pass.

- [x] **Step 2: Run Rust checks.**

```bash
just check
just lint-check
```

Expected: pass with zero warnings.

- [x] **Step 3: Run tests.**

```bash
just test
```

Expected: pass.

- [x] **Step 4: Run build.**

```bash
just build
```

Expected: pass.

- [x] **Step 5: Run aggregate if local environment supports it.**

```bash
just ci
```

Expected: pass.

If `just ci` fails due environment-specific native build dependencies, record the exact failing command and run every possible subset. Do not claim full verification if this happens.

### Task 12: Commit final plan/implementation state.

- [x] **Step 1: Check status.**

```bash
git status --short
```

Expected: only intentional files changed.

- [x] **Step 2: Commit.**

```bash
git add src/native src/lib.rs README.md tests/native_api_contract.rs docs/rust-native-api.md
git commit -m "Expose native Rust rendering facade."
```

## Acceptance Criteria

The work is complete only when all are true:

- `phyron_skia_canvas::native` exists and is documented.
- Public `native` signatures do not expose `skia_safe` or Neon types.
- Required linear working spaces render: sRGB, Display P3, Rec.2020.
- Raw readback supports tight RGBA8 unpremultiplied.
- Shape, image, and simple text drawing work through `native`.
- Existing JS/Neon tests still pass.
- Studio `studio-render-native` no longer depends on `skia-safe` directly.
- Studio native preview tests still pass with `native-skia`.
- No new undocumented `unwrap()`, `expect()`, `panic!`, `todo!`, or `unimplemented!` in production code.
- No edits to `.blueprints`.

## Known Follow-Ups

- Rich paragraph shaping API. Start simple text first; do not block this boundary on the full paragraph surface.
- GPU shared buffer transport. This belongs in Studio transport plans, not here.
- Old public module cleanup. Once Studio is migrated, a later breaking release can hide or deprecate raw modules if desired.
- Full visual regression baselines. Agents must not regenerate expected images; humans decide when to approve visual baselines.
