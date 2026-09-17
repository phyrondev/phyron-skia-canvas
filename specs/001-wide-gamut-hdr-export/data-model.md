# Data Model: Wide-Gamut and HDR Pixel Export

## Entities

### Working space

- **Owner**: `Canvas` (`src/node/canvas.rs`), fields `color_type`,
  `color_space`.
- **Identity**: One per canvas, set by `new Canvas(w, h, { colorType,
  colorSpace })`. Defaults: `rgba` (`RGBA8888`), `srgb`.
- **Fields**: Skia `ColorType`, Skia `ColorSpace`.
- **Invariants**: All compositing for export and `getImageData` happens in
  this space. `drawImage(canvas)` rasterizes the source in its own working
  space.
- **Persistence**: None.

### Output encoding

- **Owner**: `ExportOptions` (`src/context/page.rs`).
- **Fields**: `color_type` and `color_space` (output), new `premultiplied`
  (bool, default `false`), new `hdr_reference_white` (f32 nits, default 203),
  new `working_color_type` and `working_color_space` (copied from the canvas).
- **Invariants**: The output encoding never changes the blend result.
  `hdr_reference_white` is finite and greater than 0, else `RangeError`.
- **Persistence**: None.

### Page cache entry

- **Owner**: `PageCache` (`src/context/page.rs`).
- **Fields**: Adds `color_type` and `color_space` of the working space.
- **Invariants**: A cached image is used only for the same density, matte,
  msaa, working colour type and working colour space. The cached image is in
  the working space, before output conversion.

### Engine details

- **Owner**: `get_engine_status` (`src/node/canvas.rs`), type `EngineDetails`
  (`lib/index.d.ts`).
- **Fields**: New optional `fallback: string`, the colour type name of the last
  export that fell back from GPU to CPU raster.

## Wire Formats

| Name | Type | Values |
| --- | --- | --- |
| `colorSpace` (in) | string | `srgb`, `srgb-linear`, `display-p3`, `display-p3-linear`, `rec2020`, `rec2020-linear`, `rec2020-pq`, `rec2020-hlg`; aliases `linear`, `p3`, `p3-linear`, `bt2020`, `bt2020-linear`, `hdr10`, `hlg`. Else `TypeError`. |
| `colorSpace` (out, getters) | string | Canonical names only. |
| `colorType` (in) | string | Names in `to_color_type`, plus aliases `rgba`, `rgb`, `bgra`. Else `TypeError`. |
| `premultiplied` | boolean | Default `false`. |
| `hdrReferenceWhite` | number | Nits, default 203, finite and > 0. |
| `raw` buffer | bytes | Native endianness, row-major, RGBA channel order of the colour type. |

## Ownership Matrix

| Concept | Owner | Consumers | Status | Evidence |
| --- | --- | --- | --- | --- |
| JS colour space and type names | `src/node/utils.rs` | canvas, export, `ImageData`, `getImageData` | Open | `contracts/color-names.md` |
| PQ and HLG transfer | new `src/context/transfer/mod.rs` | `Page::encoded_as`, `PageRecorder::get_pixels` | Open | `contracts/raw-export.md` |
| Working space compositing | `src/context/page.rs` | `toBuffer`, `toFile`, `getImageData`, `drawImage(canvas)` | Open | `contracts/working-space.md` |
| GPU fallback | `src/gpu/mod.rs` | export surfaces, `canvas.engine` | Open | `contracts/gpu-fallback.md` |

## Migration Rules

- Invalid colour space or colour type names now throw. No silent fallback.
- An 8-bit canvas exported as float has 8-bit precision. A `srgb` canvas
  exported in a wider gamut is clipped to sRGB first. Callers set precision and
  gamut on the canvas.
