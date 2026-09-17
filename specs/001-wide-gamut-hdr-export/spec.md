# Feature Spec: Wide-Gamut and HDR Pixel Export

## Context

A render service composites frames in linear light at `RGBAF32` in
`srgb-linear`, `display-p3-linear` or `rec2020-linear`. It sends the frames to
a video or image encoder (FFmpeg, citra) in a requested output encoding. The
frames must not go through 8-bit or sRGB on the way.

Two settings control colour. The canvas settings (`new Canvas(w, h, {
colorType, colorSpace })`) name the working space. The export options
(`toBuffer(format, { colorType, colorSpace })`) name the output encoding.
Today the export step mixes the two:

- The export options, not the canvas settings, set the compositing surface.
  Measured on `3.6.0` with a `RGBAF32` `srgb-linear` canvas: 50% white over
  black reads back as `0.735` (linear blend) for `colorSpace: "srgb-linear"`,
  `0.5` (sRGB-encoded blend) for `"srgb"`, and a PQ-encoded blend for
  `"rec2020-pq"`. An export without options composites in 8-bit sRGB and clips
  HDR values before blending.
- The page cache does not record colour space or colour type. A second export
  of an unchanged canvas with other colour options reuses the first result.
- `toBuffer("raw", { colorSpace })` always converts to sRGB at readback.
- `drawImage(canvas)` rasterizes the source canvas as 8-bit sRGB. Measured on
  `3.6.0`, two CPU `RGBAF32` `srgb-linear` canvases, source linear 0.002,
  0.2001, 0.2003, 1.5: after `dst.drawImage(src)` they read back as 7/255,
  124/255, 124/255, 1.0 (sRGB-encoded). Rec.2020 green becomes sRGB green.
  `drawCanvas` keeps the values but ignores `globalAlpha`.
- An unknown or unsupported colour space name silently becomes sRGB.
- A canvas has no `colorSpace` or `colorType` getter, and `ImageData.colorSpace`
  returns the string it was given, alias or not.
- A change of only `colorType` between two exports can reuse a surface of the
  previous colour type.
- The GPU engine cannot allocate float or 16-bit surfaces, and the export
  throws.

## User Stories

### User Story 0: Composite in the working space (P1)

As a render service, I want all compositing to happen in the canvas working
space, so that the export options change only the output encoding and never
the blend result.

**Acceptance Criteria**

- Given a `RGBAF32` `srgb-linear` canvas with `[1,1,1,0.5]` over `[0,0,0,1]`,
  when I export `raw` as `RGBAF32` in `srgb-linear`, `srgb`, `rec2020-linear`
  or `rec2020-pq`, then every result decodes to linear `0.5` (within `1e-3`, or
  2 code values for 16-bit).
- Given a `RGBAF32` `srgb-linear` canvas with `[0,0,0,0.5]` over `[2,2,2,1]`,
  when I export `raw` as `RGBAF32` in `srgb-linear`, then the value is `1.0`.
  HDR values are not clipped before blending.
- Given one unchanged canvas, when I export twice with different `colorSpace`
  or `colorType`, then each result equals the result from a fresh canvas.
- Given a CPU `RGBAF32` `srgb-linear` canvas A with linear 0.002, 0.2001,
  0.2003, 1.5, when I `drawImage(A)` into a `RGBAF32` or `RGBAF16` canvas B in
  the same space, then B reads back the values of A within `1e-4` (`RGBAF16`:
  within its half-float step).
- Given a `rec2020-linear` canvas A with Rec.2020 green, when I `drawImage(A)`
  into a `rec2020-linear` or a `srgb-linear` canvas B, then B holds linear
  BT.709 (-0.5876, 1.1329, -0.1006) in `srgb-linear` terms.
- Given `globalAlpha = 0.5`, when I `drawImage(A)`, then the drawn alpha is
  halved. `globalCompositeOperation`, `filter`, shadows and the transform apply
  as for any other image.
- Given an `RGBA8888` canvas B, when I `drawImage(A)`, then B has 8-bit values.
- The PR reports the time of a 1920x1080 `drawImage(canvas)` before and after.
- Given a `RGBAF32` `srgb-linear` canvas with `[1,1,1,0.5]` over `[0,0,0,1]`,
  when I export `RGBA8888` `srgb` raw or PNG, or draw it into an `RGBA8888`
  `srgb` canvas and call `getImageData`, then the value is 188 (linear blend),
  not 128 (reported by the Studio session).
- Given `globalCompositeOperation` `lighter`, `multiply` or `destination-in`,
  a shadow, or `filter = "blur(2px)"`, when I `drawImage(canvas)` between
  float canvases, then the result matches a premultiplied composite of the
  source pixels.
- Given a GPU `RGBAF32` source canvas, when I `drawImage` it into a CPU
  `RGBAF32` canvas, then extended values and alpha survive.
- Given a `RGBAF32` canvas, when I call `getImageData` with `colorType:
  "RGBAF32"` and `colorSpace` `srgb-linear` or `rec2020-linear`, and then
  `putImageData` with the result, then the values round-trip unclamped within
  `1e-5`.
- Given an export without `colorType` and `colorSpace`, then the output is
  `RGBA8888` `srgb`, as today.

### User Story 1: Raw export in the requested colour space (P1)

As a render service, I want `toBuffer("raw")` to return pixels in the colour
space and colour type that I request, so that I can encode wide-gamut and HDR
output without a detour through sRGB.

**Acceptance Criteria**

Fixture: a `3x1` canvas, `colorType: "RGBAF32"`, `colorSpace:
"rec2020-linear"`, CPU engine. The three pixels are filled with the linear
arrays `[0,1,0,1]` (Rec.2020 green), `[1,1,1,1]` and `[2,2,2,1]`.

- Given the fixture, when I export `raw` with `colorType: "RGBAF32"`, then the
  values match this table within `1e-3`:

  | `colorSpace` | green (r, g, b) | `[2,2,2,1]` pixel |
  | --- | --- | --- |
  | `rec2020-linear` | 0, 1, 0 | 2, 2, 2 |
  | `srgb-linear` | -0.5876, 1.1329, -0.1006 | 2, 2, 2 |
  | `display-p3-linear` | -0.2822, 1.0758, -0.0196 | 2, 2, 2 |
  | `srgb` | -0.790, 1.056, -0.350 | 1.353 |

- Given the fixture, when I export `raw` with `colorType: "R16G16B16A16UNorm"`
  and `colorSpace: "rec2020-pq"` or `"rec2020-hlg"`, then each code value
  matches ST 2084 (PQ) or BT.2100 (HLG) within 2 code values. Linear `1.0` is
  the SDR reference white (see R6).
- Expected values come from an independent calculation in the test (primaries
  matrix and transfer function written out). They do not come from a second
  call into Skia.
- Given any export, when I pass `premultiplied: true`, then colour channels are
  multiplied by alpha. When I omit it or pass `false`, then colour channels are
  not multiplied by alpha (the current behaviour).

### User Story 2: Colour space names fail loudly and read back truthfully (P1)

As a caller, I want an invalid colour space name to throw and a valid name to
read back unchanged, so that a typo cannot produce sRGB output without notice.

**Acceptance Criteria**

- Given `colorSpace: "rec2020-pqq"`, when I create a canvas or export, then a
  `TypeError` is thrown. The message names the value and the accepted names.
- Given every documented colour space name, when I create a canvas and read
  `colorSpace` back, then I get the canonical name of that space (`srgb`,
  `srgb-linear`, `display-p3`, `display-p3-linear`, `rec2020`,
  `rec2020-linear`, `rec2020-pq`, `rec2020-hlg`).
- Given an alias (`linear`, `p3`, `p3-linear`, `bt2020`, `bt2020-linear`,
  `hdr10`, `hlg`), when I create a canvas, then it is accepted and `colorSpace`
  reads back the canonical name.
- Given an unknown `colorType` name, when I create a canvas or export, then a
  `TypeError` is thrown. The message names the value and the accepted names.

### User Story 3: Repeated exports with different colour types (P1)

As a caller, I want each export to use the colour type I request, even when the
canvas has not changed since the previous export.

**Acceptance Criteria**

- Given an unchanged canvas, when I export `raw` as `RGBA8888` and then as
  `RGBAF32`, then the second buffer has 16 bytes per pixel and float values.

### User Story 4: Float and 16-bit export with the GPU engine (P2)

As a render service on a host with a real GPU, I want float and 16-bit exports
to succeed, so that the output does not depend on the host.

**Acceptance Criteria**

- Given the GPU engine, when I export `raw` as `RGBAF32` or
  `R16G16B16A16UNorm`, then the export succeeds and the values match User
  Story 1.
- Given the GPU engine cannot allocate the requested colour type, when I
  export, then the export renders with CPU raster, and `canvas.engine` reports
  that the last export fell back to CPU raster.
- Where CI has no GPU, the test is skipped with a reason, and a manual QA step
  on a GPU host is the evidence.

## Non-Goals

- Video encoding, and EXR or TIFF writing.
- Tone mapping from HDR to SDR.
- Any change to how the working colour space of a canvas is chosen.
- A `cICP` chunk in PNG export for PQ and HLG. The render service writes files
  with citra (see Clarifications).
- Publishing a release. Release `3.7.0` with prebuilds is a separate step after
  merge.

## Requirements

- R0: Compositing uses the canvas `colorType` and `colorSpace`. Export options
  set only the output encoding, applied once when pixels are read or encoded.
  The page cache and the export surface cache are valid only for the same
  working space and colour type. `drawImage(canvas)` keeps the source canvas
  working space and colour type (F32 included). `getImageData` follows the same
  rule, and `putImageData` writes float `ImageData` unclamped.
- R1: `toBuffer("raw")` converts to the requested `colorSpace` and
  `colorType`.
- R2: `premultiplied` export option, default `false`.
- R3: An unknown colour space name, or a name that Skia cannot construct,
  throws a `TypeError`. There is no silent sRGB fallback. The documented
  aliases stay accepted. An unknown `colorType` name also throws a
  `TypeError`.
- R4: New read-only `canvas.colorSpace` and `canvas.colorType` getters, and
  `ImageData.colorSpace`, return the canonical name of the actual space.
- R5: A change of the output `colorType` or `colorSpace` alone does not
  reuse a cached result that was encoded for other output options.
- R6: In PQ and HLG, linear `1.0` maps to the SDR reference white of 203 nits
  (BT.2408), within 1 nit. Measured today: 200.9 nits through the PNG path.
  The export option `hdrReferenceWhite` (nits, default 203) sets another value.
  If Skia cannot construct a PQ or HLG space with another reference white,
  stop and update this spec before implementation.
- R7: The GPU engine does not throw for colour types that only CPU raster can
  allocate. It falls back to CPU raster and reports this in `canvas.engine`.
- R8: `getImageData` honours `colorSpace` if the export path supports it. The
  type docs must say what is true.
- R9: `lib/index.d.ts` and the README document `colorSpace`, `colorType` and
  `premultiplied` for `raw` and `png`, and name the colour types each format
  supports.

## Risks

- R0 is a behaviour change. A default `rgba` `srgb` canvas exported as
  `display-p3` is clipped to the sRGB gamut, and a `RGBAF32` export of an 8-bit
  canvas has 8-bit precision. Mitigation: document that the canvas settings
  set precision and gamut; the release notes say so.
- R0 changes Studio server renders: today they blend in gamma space (50%
  white over black is 128), and after R0 a linear working canvas blends
  linearly (188), like the CanvasKit editor. Studio's byte-identical parity
  baselines need a human-approved regeneration. Mitigation: the PR description
  says so (reported by the Studio session).
- R3 is a behaviour change: a caller that passes an invalid colour space or
  colour type name gets an exception instead of sRGB or `RGBA8888`. Mitigation:
  the error names the accepted values; the release notes say so.
- Skia may not allow a PQ or HLG reference white other than its built-in
  value. Mitigation: research before R6 is planned.
- The GPU path cannot be tested in CI. Mitigation: manual QA on a GPU host.
- A cold native build is slow. Mitigation: keep one `target/`; CI covers the
  OS matrix (see Clarifications).

## Clarifications

Session 2026-09-17:

- Q: Where does compositing happen? A: In the canvas `colorType` and
  `colorSpace`. Export options set only the output encoding. Found while
  planning: the handover assumed this already, but the export options set the
  compositing surface.
- Q (from the Studio session, handover C0 and C9): Must `drawImage(canvas)`
  and float `ImageData` keep precision, gamut and range? A: Yes. Added to User
  Story 0.
- Q: What is the output encoding when the export gives no colour options?
  A: `RGBA8888` `srgb`, unchanged.

- Q: Is the `cICP` chunk for PQ and HLG PNG in scope? A: No. The render
  service writes files with citra. This fork renders and reads back pixels.
- Q: How does the GPU engine report a CPU raster fallback? A: A field in
  `canvas.engine` (`EngineDetails`).
- Q: Fixed SDR reference white or an option? A: The export option
  `hdrReferenceWhite`, nits, default 203.
- Q: Do colour space aliases stay accepted? A: Yes. They read back as the
  canonical name.
- Q: No getter reads a canvas colour space back. What reads back the canonical
  name? A: New read-only `canvas.colorSpace` and `canvas.colorType` getters,
  and `ImageData.colorSpace`.
- Q: Does an unknown `colorType` name throw? A: Yes, a `TypeError`.
- Q: Where do evidence commands run? A: In CI on the pushed branch
  (`test.yml` by `workflow_dispatch`, and `rust-ci.yml`). Correction: local
  builds are fine; the development host is always on mains power and has a
  GPU, so G1 and G2 run locally too.
- Q: Is the `3.7.0` release part of done? A: No. It is a separate step after
  merge.
