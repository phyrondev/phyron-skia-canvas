//! Renderer contract tests for the `phyron_skia_canvas::native` facade.
//!
//! Tests in this file exercise the surface and pixel IO subset (Chunk 2 of the
//! Studio renderer gap closure plan). Path/filter/shader/font/paragraph tests
//! land alongside their implementation chunks; this file intentionally avoids
//! importing those types so the branch stays compiling and green.
//!
//! Public-leak audit (run from repo root):
//!   rg -n "pub .*skia_safe|pub .*FunctionContext|pub .*JsBox|pub .*Handle<|pub .*RefCell" src/native

use anyhow::Result;
use phyron_skia_canvas::native::{
    LinearColorSpace, NativeBackend, PixelColorSpace, PixelDepth, PixelExportOptions, Rect,
    RgbaLinear, ShapePaint, SurfaceOptions,
};

const ALPHA_HALF_U8: u8 = 128;

fn red_premul(alpha: f32) -> RgbaLinear {
    // Premultiplied linear red at the given alpha (alpha is straight 0..1).
    RgbaLinear::new_premultiplied(alpha, 0.0, 0.0, alpha)
}

#[test]
fn surface_create_clear_draw_snapshot_compose_readback() -> Result<()> {
    let backend = NativeBackend::new();
    let mut surface = backend.create_surface(8, 8, SurfaceOptions::default())?;
    assert_eq!(surface.width(), 8);
    assert_eq!(surface.height(), 8);

    surface.with_canvas(|canvas| {
        canvas.clear(RgbaLinear::new_premultiplied(0.0, 0.0, 0.0, 0.0));
        canvas.draw_rect(
            Rect::from_xywh(2.0, 2.0, 4.0, 4.0),
            &ShapePaint::fill(RgbaLinear::opaque(1.0, 0.0, 0.0)),
        );
    });

    // Snapshot, then composite the snapshot onto a second surface via
    // `draw_image_rect` (default source-over). BlendMode::SourceOver lands
    // with Chunk 3.
    let snapshot = surface.snapshot();
    let mut composed = backend.create_surface(8, 8, SurfaceOptions::default())?;
    composed.with_canvas(|canvas| {
        canvas.clear(RgbaLinear::new_premultiplied(0.0, 0.0, 0.0, 0.0));
        canvas.draw_image_rect(&snapshot, Rect::from_xywh(0.0, 0.0, 8.0, 8.0), 1.0);
    });

    let frame = composed.read_pixels()?;
    assert_eq!(frame.width(), 8);
    assert_eq!(frame.height(), 8);
    assert_eq!(frame.stride(), 8 * 4);
    assert_eq!(frame.depth(), PixelDepth::Uint8);
    assert_eq!(frame.color_space(), PixelColorSpace::Srgb);
    assert!(!frame.premultiplied());
    assert!(frame.pixels().iter().any(|c| *c != 0));
    Ok(())
}

#[test]
fn create_offscreen_inherits_color_space_and_composes() -> Result<()> {
    let backend = NativeBackend::new();
    let mut main = backend.create_surface(
        8,
        8,
        SurfaceOptions {
            color_space: LinearColorSpace::DisplayP3,
            ..SurfaceOptions::default()
        },
    )?;
    let mut offscreen = main.create_offscreen(4, 4)?;
    assert_eq!(offscreen.width(), 4);
    assert_eq!(offscreen.height(), 4);
    assert_eq!(offscreen.color_space(), LinearColorSpace::DisplayP3);

    offscreen.with_canvas(|canvas| {
        canvas.clear(RgbaLinear::opaque(0.0, 1.0, 0.0));
    });
    let off_snapshot = offscreen.snapshot();
    main.with_canvas(|canvas| {
        canvas.clear(RgbaLinear::new_premultiplied(0.0, 0.0, 0.0, 0.0));
        canvas.draw_image_rect(&off_snapshot, Rect::from_xywh(0.0, 0.0, 4.0, 4.0), 1.0);
    });
    let exported = main.read_pixels()?;
    assert!(exported.pixels().iter().any(|c| *c > 32));
    Ok(())
}

#[test]
fn read_pixels_as_supports_required_color_spaces() -> Result<()> {
    let backend = NativeBackend::new();
    for color_space in [
        PixelColorSpace::Srgb,
        PixelColorSpace::SrgbLinear,
        PixelColorSpace::DisplayP3,
        PixelColorSpace::DisplayP3Linear,
        PixelColorSpace::Rec2020,
        PixelColorSpace::Rec2020Linear,
    ] {
        let mut surface = backend.create_surface(2, 2, SurfaceOptions::default())?;
        surface.with_canvas(|canvas| canvas.clear(RgbaLinear::opaque(0.5, 0.5, 0.5)));
        let exported = surface.read_pixels_as(PixelExportOptions {
            color_space,
            depth: PixelDepth::Uint8,
            premultiplied: false,
        })?;
        assert_eq!(exported.color_space(), color_space);
        assert_eq!(exported.width(), 2);
        assert_eq!(exported.height(), 2);
        assert_eq!(exported.stride(), 2 * 4);
    }
    Ok(())
}

#[test]
fn read_write_pixels_linear_round_trips_dimensions() -> Result<()> {
    let backend = NativeBackend::new();
    let mut surface = backend.create_surface(4, 4, SurfaceOptions::default())?;
    surface.with_canvas(|canvas| canvas.clear(RgbaLinear::opaque(0.25, 0.5, 0.75)));
    let exported = surface.read_pixels_linear()?;
    assert_eq!(exported.depth(), PixelDepth::F32);
    assert_eq!(exported.color_space(), PixelColorSpace::SrgbLinear);
    assert!(exported.premultiplied());
    assert_eq!(exported.stride(), 4 * 16);
    assert_eq!(exported.pixels().len(), 4 * 16 * 4);

    // Round-trip back into a fresh surface and re-read; dimensions and depth
    // must be preserved.
    let mut destination = backend.create_surface(4, 4, SurfaceOptions::default())?;
    destination.write_pixels_linear(exported.pixels())?;
    let round_tripped = destination.read_pixels_linear()?;
    assert_eq!(round_tripped.width(), 4);
    assert_eq!(round_tripped.height(), 4);
    assert_eq!(round_tripped.depth(), PixelDepth::F32);
    assert_eq!(round_tripped.pixels().len(), exported.pixels().len());
    Ok(())
}

#[test]
fn linear_working_spaces_accept_hdr_values_above_one() -> Result<()> {
    let backend = NativeBackend::new();
    for color_space in [
        LinearColorSpace::Srgb,
        LinearColorSpace::DisplayP3,
        LinearColorSpace::Rec2020,
    ] {
        let mut surface = backend.create_surface(
            2,
            2,
            SurfaceOptions {
                color_space,
                ..SurfaceOptions::default()
            },
        )?;
        surface.with_canvas(|canvas| canvas.clear(RgbaLinear::opaque(2.0, 0.0, 0.0)));
        let exported = surface.read_pixels_linear()?;
        assert_eq!(exported.depth(), PixelDepth::F32);
        // First pixel's red channel as a little-endian f32.
        let bytes = exported.pixels();
        let r = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert!(r > 1.5, "expected HDR red >1.5 in {color_space:?}, got {r}");
    }
    Ok(())
}

#[test]
fn premultiplied_alpha_preserved_across_read_modes() -> Result<()> {
    let backend = NativeBackend::new();
    let mut surface = backend.create_surface(4, 4, SurfaceOptions::default())?;
    surface.with_canvas(|canvas| {
        canvas.clear(RgbaLinear::new_premultiplied(0.0, 0.0, 0.0, 0.0));
        canvas.draw_rect(
            Rect::from_xywh(0.0, 0.0, 4.0, 4.0),
            &ShapePaint::fill(red_premul(0.5)),
        );
    });

    // Premultiplied Uint8 in the surface's own linear space: r ≈ 0.5 * 255.
    let premul = surface.read_pixels_as(PixelExportOptions {
        color_space: PixelColorSpace::SrgbLinear,
        depth: PixelDepth::Uint8,
        premultiplied: true,
    })?;
    let p_r = premul.pixels()[0];
    let p_a = premul.pixels()[3];
    assert!(
        p_r.abs_diff(ALPHA_HALF_U8) <= 4,
        "premul red ≈ 128, got {p_r}"
    );
    assert!(
        p_a.abs_diff(ALPHA_HALF_U8) <= 4,
        "premul alpha ≈ 128, got {p_a}"
    );

    // Unpremultiplied Uint8 in the same color space: r = 1.0 * 255, alpha = 128.
    let unpremul = surface.read_pixels_as(PixelExportOptions {
        color_space: PixelColorSpace::SrgbLinear,
        depth: PixelDepth::Uint8,
        premultiplied: false,
    })?;
    let u_r = unpremul.pixels()[0];
    let u_a = unpremul.pixels()[3];
    assert!(u_r > 240, "unpremul red ≈ 255, got {u_r}");
    assert!(
        u_a.abs_diff(ALPHA_HALF_U8) <= 4,
        "unpremul alpha ≈ 128, got {u_a}"
    );
    Ok(())
}

/// Compile-time leak audit: importing only `phyron_skia_canvas::native::*`
/// must be sufficient for surface + pixel IO contract use. This test
/// references the new public types at run time so any accidental private
/// scoping breaks the build.
#[test]
fn public_types_are_reachable_from_native_namespace_only() -> Result<()> {
    let backend = NativeBackend::new();
    let surface = backend.create_surface(1, 1, SurfaceOptions::default())?;
    let _ = (
        surface.color_space(),
        PixelColorSpace::Srgb,
        PixelDepth::F16,
        PixelExportOptions::default(),
    );
    Ok(())
}
