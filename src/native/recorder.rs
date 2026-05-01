use skia_safe::{
    Canvas as SkCanvas, Color4f, ColorType, ImageInfo, Paint, PaintStyle, Point as SkPoint, RRect,
    Rect as SkRect,
};

use crate::context::page::{ExportOptions, PageRecorder};
use crate::gpu::RenderingEngine;
use crate::native::color::RgbaLinear;
use crate::native::error::NativeError;
use crate::native::geometry::{Point, Rect};
use crate::native::image::NativeImage;
use crate::native::paint::ShapePaint;
use crate::native::pixels::{RawFrame, RawFrameOptions, SurfaceOptions};
use crate::native::text::{TextAlign, TextBoxOptions, VerticalAlign};

pub struct NativeRecorder {
    recorder: PageRecorder,
    bounds: Rect,
}

pub struct NativeCanvas<'a> {
    canvas: &'a SkCanvas,
}

impl NativeRecorder {
    pub fn new(bounds: Rect) -> Result<Self, NativeError> {
        if bounds.is_empty() || !bounds.width().is_finite() || !bounds.height().is_finite() {
            return Err(NativeError::InvalidDimensions {
                width: bounds.width(),
                height: bounds.height(),
            });
        }
        let sk_bounds = to_sk_rect(bounds);
        let recorder = PageRecorder::new(sk_bounds);
        Ok(Self { recorder, bounds })
    }

    pub fn record(&mut self, f: impl FnOnce(&mut NativeCanvas<'_>)) {
        self.recorder.append(|skia_canvas| {
            let mut canvas = NativeCanvas::new(skia_canvas);
            f(&mut canvas);
        });
    }

    pub fn render_raw(
        &mut self,
        surface_options: SurfaceOptions,
        frame_options: RawFrameOptions,
    ) -> Result<RawFrame, NativeError> {
        let surface_color_space = surface_options.color_space.to_skia_color_space()?;
        let dst_color_type = frame_options.pixel_format.to_skia_color_type()?;
        let dst_alpha_type = frame_options.pixel_format.to_skia_alpha_type();
        let dst_color_space = frame_options.color_space.to_skia_color_space()?;

        let density = if surface_options.density.is_finite() && surface_options.density > 0.0 {
            surface_options.density
        } else {
            1.0
        };
        let scaled_w = (self.bounds.width() * density).floor().max(0.0) as i32;
        let scaled_h = (self.bounds.height() * density).floor().max(0.0) as i32;
        if scaled_w <= 0 || scaled_h <= 0 {
            return Err(NativeError::InvalidDimensions {
                width: self.bounds.width(),
                height: self.bounds.height(),
            });
        }

        let dst_info = ImageInfo::new(
            (scaled_w, scaled_h),
            dst_color_type,
            dst_alpha_type,
            dst_color_space,
        );

        let export_options = ExportOptions {
            density,
            color_type: ColorType::RGBAF16,
            color_space: surface_color_space,
            msaa: surface_options.msaa,
            ..ExportOptions::default()
        };

        let page = self.recorder.get_page();
        let pixels = page
            .render_raw(export_options, dst_info, RenderingEngine::default())
            .map_err(|reason| NativeError::Render { reason })?;

        let stride = (scaled_w as usize) * frame_options.pixel_format.bytes_per_pixel();
        Ok(RawFrame::new(
            scaled_w as u32,
            scaled_h as u32,
            stride,
            frame_options.pixel_format,
            frame_options.color_space,
            pixels,
        ))
    }

    pub fn bounds(&self) -> Rect {
        self.bounds
    }
}

impl NativeCanvas<'_> {
    pub(crate) fn new(canvas: &SkCanvas) -> NativeCanvas<'_> {
        NativeCanvas { canvas }
    }

    pub fn clear(&mut self, color: RgbaLinear) {
        self.canvas.clear(to_unpremul_color4f(color));
    }

    pub fn save(&mut self) {
        self.canvas.save();
    }

    pub fn restore(&mut self) {
        self.canvas.restore();
    }

    pub fn translate(&mut self, point: Point) {
        self.canvas.translate(SkPoint::new(point.x, point.y));
    }

    pub fn rotate_degrees(&mut self, degrees: f32, pivot: Option<Point>) {
        let pivot = pivot.map(|p| SkPoint::new(p.x, p.y));
        self.canvas.rotate(degrees, pivot);
    }

    pub fn draw_rect(&mut self, rect: Rect, paint: &ShapePaint) {
        let sk_rect = to_sk_rect(rect);
        if let Some(fill) = paint.fill {
            let p = make_paint(fill, PaintStyle::Fill, 0.0, paint.anti_alias);
            self.canvas.draw_rect(sk_rect, &p);
        }
        if let Some(stroke) = paint.stroke {
            let p = make_paint(
                stroke,
                PaintStyle::Stroke,
                paint.stroke_width,
                paint.anti_alias,
            );
            self.canvas.draw_rect(sk_rect, &p);
        }
    }

    pub fn draw_rounded_rect(&mut self, rect: Rect, radius: f32, paint: &ShapePaint) {
        let sk_rect = to_sk_rect(rect);
        let rrect = RRect::new_rect_xy(sk_rect, radius, radius);
        if let Some(fill) = paint.fill {
            let p = make_paint(fill, PaintStyle::Fill, 0.0, paint.anti_alias);
            self.canvas.draw_rrect(rrect, &p);
        }
        if let Some(stroke) = paint.stroke {
            let p = make_paint(
                stroke,
                PaintStyle::Stroke,
                paint.stroke_width,
                paint.anti_alias,
            );
            self.canvas.draw_rrect(rrect, &p);
        }
    }

    pub fn draw_oval(&mut self, rect: Rect, paint: &ShapePaint) {
        let sk_rect = to_sk_rect(rect);
        if let Some(fill) = paint.fill {
            let p = make_paint(fill, PaintStyle::Fill, 0.0, paint.anti_alias);
            self.canvas.draw_oval(sk_rect, &p);
        }
        if let Some(stroke) = paint.stroke {
            let p = make_paint(
                stroke,
                PaintStyle::Stroke,
                paint.stroke_width,
                paint.anti_alias,
            );
            self.canvas.draw_oval(sk_rect, &p);
        }
    }

    pub fn draw_image_rect(&mut self, image: &NativeImage, dst: Rect, opacity: f32) {
        let dst_rect = to_sk_rect(dst);
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_alpha_f(opacity.clamp(0.0, 1.0));
        self.canvas
            .draw_image_rect(&image.inner, None, dst_rect, &paint);
    }

    pub fn draw_text_box(&mut self, text: &str, rect: Rect, options: &TextBoxOptions) {
        use skia_safe::{
            FontMgr, FontStyle,
            font_style::{Slant, Weight, Width},
            textlayout::{
                FontCollection, ParagraphBuilder, ParagraphStyle, TextAlign as SkTextAlign,
                TextStyle,
            },
        };

        let mut paint = Paint::default();
        let modulated = options.color.with_opacity(options.opacity);
        paint.set_color4f(to_unpremul_color4f(modulated), None);
        paint.set_anti_alias(true);

        let font_mgr = FontMgr::new();
        let mut font_collection = FontCollection::new();
        font_collection.set_default_font_manager(font_mgr, None);

        let mut text_style = TextStyle::new();
        text_style.set_foreground_paint(&paint);
        text_style.set_font_size(options.font_size);
        if let Some(family) = &options.font_family {
            text_style.set_font_families(&[family.as_str()]);
        }
        text_style.set_font_style(FontStyle::new(
            Weight::from(options.font_weight),
            Width::NORMAL,
            Slant::Upright,
        ));

        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.set_text_align(match options.horizontal_align {
            TextAlign::Left => SkTextAlign::Left,
            TextAlign::Center => SkTextAlign::Center,
            TextAlign::Right => SkTextAlign::Right,
        });
        paragraph_style.set_text_style(&text_style);

        let mut builder = ParagraphBuilder::new(&paragraph_style, font_collection);
        builder.add_text(text);
        let mut paragraph = builder.build();
        paragraph.layout(rect.width());

        let y_offset = match options.vertical_align {
            VerticalAlign::Top => 0.0,
            VerticalAlign::Center => (rect.height() - paragraph.height()).max(0.0) / 2.0,
            VerticalAlign::Bottom => (rect.height() - paragraph.height()).max(0.0),
        };

        paragraph.paint(self.canvas, (rect.left, rect.top + y_offset));
    }
}

fn to_sk_rect(rect: Rect) -> SkRect {
    SkRect::from_ltrb(rect.left, rect.top, rect.right, rect.bottom)
}

fn to_unpremul_color4f(c: RgbaLinear) -> Color4f {
    if c.a > 0.0 {
        Color4f {
            r: c.r / c.a,
            g: c.g / c.a,
            b: c.b / c.a,
            a: c.a,
        }
    } else {
        Color4f::new(0.0, 0.0, 0.0, 0.0)
    }
}

fn make_paint(color: RgbaLinear, style: PaintStyle, stroke_width: f32, anti_alias: bool) -> Paint {
    let mut paint = Paint::default();
    paint.set_color4f(to_unpremul_color4f(color), None);
    paint.set_style(style);
    paint.set_stroke_width(stroke_width);
    paint.set_anti_alias(anti_alias);
    paint
}
