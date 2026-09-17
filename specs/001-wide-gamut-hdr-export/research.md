# Research: Wide-Gamut and HDR Pixel Export

Skia references are to Skia m151 source (`~/code/skia`). The fork builds
`skia-safe` 0.97.2 (Skia m148). No 0.97.2 source is on the build machine, so
each Skia fact below is "close to m148, not verified against 0.97.2" until a
test confirms it.

## Measurements (phyron-skia-canvas `3.6.0`, Node 22, CPU raster)

- M1. `toBuffer("raw", { colorType: "RGBAF32", colorSpace: X })` on one
  `RGBAF32` `rec2020-linear` canvas gave identical values for all eight `X`:
  the extended sRGB-encoded values. Cause 1: the readback destination is
  hard-coded to sRGB (`Page::encoded_as`, `"raw"` branch). Cause 2: the page
  cache does not record colour options, so exports 2-8 reused export 1.
- M2. Compositing happens in the export options, not in the canvas settings.
  A fresh `RGBAF32` `srgb-linear` canvas, 50% white over black, read back as:
  `0.7354` for `srgb-linear` (linear blend), `0.5` for `srgb` (encoded blend),
  and `0.2305` for `rec2020-pq`. `[0,0,0,0.5]` over `[2,2,2,1]`: `1.0001`
  (`srgb-linear`) against `0.6767` (`srgb`). Default raw export: `128` and
  `127`, which is an 8-bit sRGB blend with HDR clipped first.
- M3. PNG export with a PQ profile decoded linear `1.0` as 200.9 nits instead
  of 203 (decoder not recorded).
- M4. The GPU engine throws `Could not allocate new WxH bitmap` for `RGBAF32`
  and `R16G16B16A16UNorm`.
- M6. `drawImage(canvas)` between two CPU `RGBAF32` `srgb-linear` canvases:
  source linear 0.002, 0.2001, 0.2003, 1.5 reads back as 7/255, 124/255,
  124/255, 1.0 (sRGB-encoded). `drawCanvas` keeps the values and ignores
  `globalAlpha`. (Measured by the Studio session, handover M5.)
- M5. A float colour array acts as unpremultiplied: `[0.5,0.5,0.5,0.5]` over
  black gave linear `0.25`. `lib/index.d.ts` says premultiplied. Not in scope;
  recorded for a separate fix.

## Decisions

### Decision: Composite in the canvas working space

**Chosen**: The export surface and the `getImageData` surface use the canvas
`colorType` and `colorSpace`. The export options apply once, when pixels are
read (`read_pixels`) or encoded.

**Reason**: Blend results must not depend on the output encoding (M2). The
render service creates its canvases in a linear float working space already.

**Rejected Alternatives**

- Composite in the export options (today): PQ and HLG exports blend in
  PQ-encoded 16-bit; exports without options blend in 8-bit sRGB.
- Composite in the export options, except HDR in linear F32: two models in one
  code path, and the default export still blends in 8-bit sRGB.

### Decision: PQ and HLG transfer in Rust, not in Skia

**Chosen**: For a `rec2020-pq` or `rec2020-hlg` output, Skia reads the working
surface into `RGBAF32`, unpremultiplied, `rec2020-linear`. A Rust function
applies the transfer with the requested reference white. The encoded floats are
then wrapped as a `rec2020-linear` image and read into the output colour type.
Same space on both sides, so Skia does only the type conversion (clamp and
quantization).

**Reason**:

- Skia constructs PQ and HLG only with a 203-nit reference white:
  `SkNamedTransferFn::kPQ = {-5, 203, 0, ...}`,
  `kHLG = {-6, 203, 1000, 1.2, ...}` (`include/core/SkColorSpace.h`).
  `skia-safe` has no `ColorSpace::new_rgb` (`// TODO: makeRGB`), so
  `hdrReferenceWhite` cannot reach Skia through a public API.
- Skia's raster PQ stage (`PQish`, `src/opts/SkRasterPipeline_opts.h`) uses
  `approx_powf`. The PQ `m2` exponent (78.84) multiplies its log error. This is
  the most plausible cause of M3. A 1% error at 203 nits is about 90 PQ code
  values, so the 2-code-value acceptance cannot pass through Skia.
- The exact formulas are short (ST 2084, BT.2100) and testable.

**Rejected Alternatives**

- Patch the reference white in a serialized `ColorSpace` and deserialize it:
  depends on an undocumented byte layout, and keeps the `approx_powf` error.
- Call the raw binding `SkColorSpace_MakeRGB`: returns `sk_sp` by value
  without a `C_` wrapper, so the ABI is suspect. Also keeps the error.
- Add `ColorSpace::new_rgb` to the rust-skia fork: a dependency change for one
  option, and it keeps the error.

### Decision: HLG definition

**Chosen**: Display-referred BT.2100 HLG for a 1000-nit display, system gamma
1.2, black level 0. Per pixel: `Fd = linear * W` nits, `Yd` from BT.2020 luma
coefficients (0.2627, 0.6780, 0.0593) of `Fd / 1000`, scene light
`Es = (Fd / 1000) * Yd^((1 - 1.2) / 1.2)`, signal `E' = OETF(Es)`.
Check: `W = 203` gives `E' = 0.750` for white (BT.2408: 75%).

**Reason**: Matches Skia's `kHLG` parameters (1000 nits, 1.2) and BT.2408.

### Decision: `drawImage(canvas)` precision

**Chosen**: `PageRecorder::get_image` rasterizes the source picture into a
raster surface of the source canvas working colour type and colour space, and
returns its snapshot. The destination then draws it as any other image, so
`globalAlpha`, compositing, filters, shadows and sampling are unchanged.

**Reason**: M6 below needs `1e-4` at 0.2001 against 0.2003. A half float has a
step of about 2.4e-4 near 0.2, so F16 fails. `images::deferred_from_picture`
supports only `BitDepth::U8` and `BitDepth::F16`, so it cannot produce F32.

**Rejected Alternatives**

- `deferred_from_picture` with `BitDepth::F16`: fails the precision criterion.
- Draw the picture directly (as `drawCanvas` does): keeps values, but
  `globalAlpha` and the image paths (sampling, shadows) would need a second
  implementation.
- Widest of source and destination colour type (handover C0): the source
  cannot hold more than its own type, so the source type is enough.

### Decision: Name parsing owner

**Chosen**: `src/node/utils.rs` keeps the JS name tables (`to_color_space`,
`from_color_space`, `to_color_type`) and returns `Result`. The crate-root Rust
API (`src/pixels.rs`, `src/color.rs`) keeps its typed enums, which have no
string names.

**Reason**: The names are a JS API surface. The Rust API has no string form to
share. No logic is duplicated.

### Decision: Reading back a colour space name

**Chosen**: Build the eight canonical `ColorSpace` values once and compare with
`==` (`PartialEq` through `SkColorSpace_Equals`). No match is an internal error.

**Reason**: Simpler than matching `transfer_fn()` and `to_xyzd50_hash()`.

### Decision: GPU fallback report

**Chosen**: `RenderingEngine::make_surface` retries with `surfaces::raster`
when the GPU surface fails. The canvas keeps a shared cell that the export
writes. `canvas.engine` gets the field `fallback` (string, the colour type
that fell back), absent when the last export did not fall back.

**Reason**: Clarification (field in `EngineDetails`). A shared cell is
needed because the export runs on a `rayon` thread after `toBuffer` returns.

### Decision: `premultiplied` scope

**Chosen**: Applies to `raw` and `getImageData`. PNG, JPEG and WebP are
unpremultiplied by definition, so the encoders ignore it.

## References

- ST 2084:2014 (PQ). ITU-R BT.2100-2 (PQ, HLG). ITU-R BT.2408-7 (203 nits).
- `include/core/SkColorSpace.h` (Skia m151): `SkNamedTransferFn::kPQ`, `kHLG`.
- `src/core/SkColorSpaceXformSteps.cpp` (Skia m151): PQ `scaleFactor`.
- `skia-safe` `src/core/color_space.rs`: `PartialEq`, no `new_rgb`.
- `src/context/page.rs`: `Page::encoded_as`, `PageRecorder::get_pixels`,
  `PageRecorder::get_image`, `PageCache`.
- `src/node/utils.rs`: `to_color_space`, `from_color_space`, `to_color_type`,
  `export_options_arg`, `image_data_export_arg`.
- `src/gpu/mod.rs`: `RenderingEngine::make_surface`.
