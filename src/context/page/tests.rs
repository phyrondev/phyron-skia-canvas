use super::*;
use skia_safe::{Color4f, Paint};

const SIZE: i32 = 4;

fn linear() -> ColorSpace {
    ColorSpace::new_srgb_linear()
}

/// A recorder whose page is opaque black under a fill of `color`, drawn in
/// `color_space`.
fn recorder(color: Color4f, color_space: &ColorSpace) -> PageRecorder {
    let mut recorder =
        PageRecorder::new(Rect::from_wh(SIZE as f32, SIZE as f32));
    recorder.append(|canvas| {
        canvas.clear(Color::BLACK);
        let paint = Paint::new(color, color_space);
        canvas.draw_rect(Rect::from_wh(SIZE as f32, SIZE as f32), &paint);
    });
    recorder
}

/// The first pixel of `image`, read as unpremultiplied linear-light floats.
fn first_pixel(image: &SkImage) -> anyhow::Result<[f32; 4]> {
    let info = ImageInfo::new(
        (SIZE, SIZE),
        ColorType::RGBAF32,
        AlphaType::Unpremul,
        linear(),
    );
    let mut pixels = vec![0f32; (SIZE * SIZE * 4) as usize];
    let row_bytes = info.min_row_bytes();
    anyhow::ensure!(
        image.read_pixels(
            &info,
            &mut pixels,
            row_bytes,
            (0, 0),
            CachingHint::Disallow
        ),
        "read_pixels failed"
    );
    Ok([pixels[0], pixels[1], pixels[2], pixels[3]])
}

fn near(got: f32, want: f32, tolerance: f32, what: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        (got - want).abs() <= tolerance,
        "{what}: {got} is not within {tolerance} of {want}"
    );
    Ok(())
}

/// `drawImage(canvas)` must not rasterize an 8-bit or half-float source on
/// the calling thread: 3.7.0 did, which put every layer composite of a
/// recorded scene on the JS thread instead of the export pool.
#[test]
fn get_image_is_lazy_for_8888_and_f16() -> anyhow::Result<()> {
    for (color_type, color_space, lazy_type) in [
        (ColorType::RGBA8888, ColorSpace::new_srgb(), ColorType::N32),
        (ColorType::BGRA8888, ColorSpace::new_srgb(), ColorType::N32),
        (ColorType::RGBA8888, linear(), ColorType::N32),
        (ColorType::RGBAF16, linear(), ColorType::RGBAF16),
        (
            ColorType::RGBAF16,
            ColorSpace::new_srgb(),
            ColorType::RGBAF16,
        ),
    ] {
        let image = recorder(Color4f::new(0.25, 0.5, 0.75, 1.0), &color_space)
            .get_image(color_type, &color_space)
            .ok_or_else(|| anyhow::anyhow!("no image for {color_type:?}"))?;
        anyhow::ensure!(
            image.is_lazy_generated(),
            "{color_type:?} source was rasterized eagerly"
        );
        anyhow::ensure!(
            image.color_type() == lazy_type,
            "{color_type:?} source is {:?}, not {lazy_type:?}",
            image.color_type()
        );
        anyhow::ensure!(
            image.color_space().as_ref() == Some(&color_space),
            "{color_type:?} source lost its colour space"
        );
    }
    Ok(())
}

/// Types Skia has no picture image for stay eager, at their own type.
#[test]
fn get_image_is_eager_at_other_types() -> anyhow::Result<()> {
    for color_type in [ColorType::RGBAF32, ColorType::RGB888x] {
        let image = recorder(Color4f::new(0.25, 0.5, 0.75, 1.0), &linear())
            .get_image(color_type, &linear())
            .ok_or_else(|| anyhow::anyhow!("no image for {color_type:?}"))?;
        anyhow::ensure!(
            !image.is_lazy_generated(),
            "{color_type:?} source is lazy"
        );
        anyhow::ensure!(
            image.color_type() == color_type,
            "{color_type:?} source is {:?}",
            image.color_type()
        );
    }
    Ok(())
}

/// The lazy half-float source keeps the colour behaviour of 3.7.0
/// (upstream skia-canvas #1083 as adopted in Studio): it blends in the
/// linear working space (50% white over black is 0.5 linear, 188 in sRGB,
/// not the gamma blend's 128), and it keeps extended values above 1.
#[test]
fn lazy_f16_source_keeps_linear_blend_and_range() -> anyhow::Result<()> {
    let half_white = recorder(Color4f::new(1.0, 1.0, 1.0, 0.5), &linear())
        .get_image(ColorType::RGBAF16, &linear())
        .ok_or_else(|| anyhow::anyhow!("no image"))?;
    let [r, g, b, a] = first_pixel(&half_white)?;
    for (value, channel) in [(r, "r"), (g, "g"), (b, "b")] {
        near(value, 0.5, 1e-3, channel)?;
    }
    near(a, 1.0, 1e-3, "a")?;

    let extended = recorder(Color4f::new(2.0, 0.5, 0.001, 1.0), &linear())
        .get_image(ColorType::RGBAF16, &linear())
        .ok_or_else(|| anyhow::anyhow!("no image"))?;
    let [r, g, b, _] = first_pixel(&extended)?;
    near(r, 2.0, 2e-3, "extended r")?;
    near(g, 0.5, 1e-3, "g")?;
    // 0.001 linear is below the first 8-bit sRGB step (0.0003 and 0.0006
    // round to codes 0 and 1); half floats resolve it.
    near(b, 0.001, 1e-5, "deep shadow b")?;
    Ok(())
}

/// The source is a snapshot: drawing on the source canvas after
/// `drawImage` does not reach the image already taken.
#[test]
fn lazy_source_is_a_snapshot() -> anyhow::Result<()> {
    let mut recorder = recorder(Color4f::new(1.0, 0.0, 0.0, 1.0), &linear());
    let image = recorder
        .get_image(ColorType::RGBAF16, &linear())
        .ok_or_else(|| anyhow::anyhow!("no image"))?;
    recorder.append(|canvas| {
        canvas.clear(Color::BLUE);
    });
    let [r, g, b, _] = first_pixel(&image)?;
    near(r, 1.0, 1e-3, "r")?;
    near(g, 0.0, 1e-3, "g")?;
    near(b, 0.0, 1e-3, "b")?;
    Ok(())
}
