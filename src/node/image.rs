#![allow(unused_imports)]
#![allow(dead_code)]
use crate::{
    context::{BoxedContext2D, Context2D},
    font_library::FontLibrary,
    node::memory::ExternalMemory,
    utils::*,
};
use neon::{prelude::*, types::buffer::TypedArray};
use skia_safe::{
    AlphaType, ColorSpace, ColorType, Data, FontMgr, ISize, Image as SkImage,
    ImageInfo, Picture, PictureRecorder, Rect, Size,
    image::{CachingHint, images},
    svg::{self, Length, LengthUnit},
};
use std::cell::RefCell;

pub type BoxedImage = JsBox<RefCell<Image>>;
impl Finalize for Image {
    fn finalize<'a, C: Context<'a>>(mut self, cx: &mut C) {
        self.memory.clear(cx);
    }
}

pub struct Image {
    src: String,
    pub autosized: bool,
    pub content: Content,
    /// Native bytes reported to V8 for this image (`node::memory`).
    memory: ExternalMemory,
}

impl Default for Image {
    fn default() -> Self {
        Image {
            content: Content::Loading,
            autosized: false,
            src: "".to_string(),
            memory: ExternalMemory::default(),
        }
    }
}

impl Image {
    /// Report the raster this image owns to V8. Decoded files and SVGs are
    /// lazy or vector and own no raster until drawn; raw pixels and canvas
    /// snapshots (`Canvas.toImage`) do.
    pub fn sync_memory<'a>(&mut self, cx: &mut impl Context<'a>) {
        let owned = self.content.raster_bytes();
        self.memory.sync(cx, owned);
    }
}

#[derive(Default)]
pub enum Content {
    Bitmap(SkImage),
    Vector(Picture, Size),
    #[default]
    Loading,
    Broken,
}

impl Clone for Content {
    fn clone(&self) -> Self {
        match self {
            Content::Bitmap(img) => Content::Bitmap(img.clone()),
            Content::Vector(pict, size) => Content::Vector(pict.clone(), *size),
            _ => Content::default(),
        }
    }
}

impl Content {
    /// `cpu_only`: both canvases render on the CPU, so a bitmap source may
    /// be lazy (see `PageRecorder::get_image`).
    pub fn from_context(
        ctx: &mut Context2D,
        use_vector: bool,
        cpu_only: bool,
    ) -> Self {
        match use_vector {
            true => ctx
                .get_picture()
                .map(|p| Content::Vector(p, ctx.bounds.size())),
            false => ctx.get_image(cpu_only).map(Content::Bitmap),
        }
        .unwrap_or_default()
    }

    pub fn from_image_data(image_data: ImageData) -> Self {
        let info = image_data.image_info();
        images::raster_from_data(
            &info,
            &image_data.buffer,
            info.min_row_bytes(),
        )
        .map(Content::Bitmap)
        .unwrap_or_default()
    }

    pub fn size(&self) -> Size {
        match &self {
            Content::Bitmap(img) => img.dimensions().into(),
            Content::Vector(_, size) => *size,
            _ => Size::new_empty(),
        }
    }

    pub fn is_complete(&self) -> bool {
        !matches!(self, Content::Loading)
    }

    /// Bytes of raster this content owns: a materialized bitmap's pixels.
    /// A lazy (deferred decode or picture) image owns nothing until drawn,
    /// and a vector picture is a display list.
    pub fn raster_bytes(&self) -> i64 {
        match self {
            Content::Bitmap(img) if !img.is_lazy_generated() => {
                img.image_info().compute_min_byte_size() as i64
            }
            _ => 0,
        }
    }

    pub fn is_drawable(&self) -> bool {
        !matches!(self, Content::Loading | Content::Broken)
    }

    pub fn snap_rects_to_bounds(
        &self,
        mut src: Rect,
        mut dst: Rect,
    ) -> (Rect, Rect) {
        // Handle 'overdraw' of the src image where the crop coordinates are
        // outside of its bounds Snap the src rect to its actual bounds
        // and shift/pad the dst rect to account for the whitespace
        // included in the crop.
        let scale_x = dst.width() / src.width();
        let scale_y = dst.height() / src.height();
        let size = self.size();

        if src.left < 0.0 {
            dst.left += -src.left * scale_x;
            src.left = 0.0;
        }

        if src.top < 0.0 {
            dst.top += -src.top * scale_y;
            src.top = 0.0;
        }

        if src.right > size.width {
            dst.right -= (src.right - size.width) * scale_x;
            src.right = size.width;
        }

        if src.bottom > size.height {
            dst.bottom -= (src.bottom - size.height) * scale_y;
            src.bottom = size.height;
        }

        (src, dst)
    }
}

#[derive(Debug)]
pub struct ImageData {
    pub width: f32,
    pub height: f32,
    pub buffer: Data,
    color_type: ColorType,
    color_space: ColorSpace,
}

impl ImageData {
    pub fn new(
        buffer: Data,
        width: f32,
        height: f32,
        color_type: ColorType,
        color_space: ColorSpace,
    ) -> Self {
        Self {
            buffer,
            width,
            height,
            color_type,
            color_space,
        }
    }

    pub fn image_info(&self) -> ImageInfo {
        ImageInfo::new(
            (self.width as _, self.height as _),
            self.color_type,
            AlphaType::Unpremul,
            self.color_space.clone(),
        )
    }
}

//
// -- Javascript Methods
// --------------------------------------------------------------------------
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedImage> {
    let this = RefCell::new(Image::default());
    Ok(cx.boxed(this))
}

/// `Canvas.toImage()`: a raster snapshot of a canvas, taken now, on the CPU,
/// at the canvas working colour type and space. The image owns its pixels,
/// so drawing it pins neither the canvas's recording nor a picture of it,
/// whatever the colour type or engine.
pub fn from_canvas(mut cx: FunctionContext) -> JsResult<BoxedImage> {
    let ctx = cx.argument::<BoxedContext2D>(1)?;
    // Take the lazy image where one exists and rasterize it through Skia's
    // own generator, so the pixels are the ones a CPU `drawImage(canvas)`
    // draws, byte for byte (an eager raster of the same picture differs by
    // a code value here and there at 8 bits). Other colour types are
    // rasterized eagerly, as `drawImage` rasterizes them.
    let content =
        match Content::from_context(&mut ctx.borrow_mut(), false, true) {
            Content::Bitmap(image) if image.is_lazy_generated() => image
                .make_raster_image(None, CachingHint::Disallow)
                .map(Content::Bitmap)
                .unwrap_or(Content::Broken),
            content @ Content::Bitmap(_) => content,
            // a canvas that cannot be snapshotted (0x0) is a broken image,
            // not one that never finishes loading
            _ => Content::Broken,
        };
    let mut image = Image {
        content,
        ..Image::default()
    };
    image.sync_memory(&mut cx);
    Ok(cx.boxed(RefCell::new(image)))
}

/// `Image.release()`: drop the pixels now. The image reads as broken
/// afterwards; the JS side refuses to draw it.
pub fn release(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let this = cx.argument::<BoxedImage>(0)?;
    let mut this = this.borrow_mut();
    this.content = Content::Broken;
    this.sync_memory(&mut cx);
    Ok(cx.undefined())
}

pub fn get_src(mut cx: FunctionContext) -> JsResult<JsString> {
    let this = cx.argument::<BoxedImage>(0)?;
    let this = this.borrow();

    Ok(cx.string(&this.src))
}

pub fn set_src(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let this = cx.argument::<BoxedImage>(0)?;
    let mut this = this.borrow_mut();

    let src = cx.argument::<JsString>(1)?.value(&mut cx);
    this.src = src;
    Ok(cx.undefined())
}

pub fn set_data<'a>(
    mut cx: FunctionContext<'a>,
) -> NeonResult<Handle<'a, JsBoolean>> {
    let this = cx.argument::<BoxedImage>(0)?;
    let mut this = this.borrow_mut();
    let buffer = cx.argument::<JsBuffer>(1)?;
    let data = Data::new_copy(buffer.as_slice(&cx));

    if let Some(raw_info) = opt_image_info_arg(&mut cx, 2)? {
        // First, check for an optional dims argument and interpret the buffer
        // as raw rgba if present
        this.content = match images::raster_from_data(
            &raw_info,
            data,
            raw_info.min_row_bytes(),
        ) {
            Some(image) => Content::Bitmap(image),
            None => Content::Broken,
        }
    } else if let Some(image) = images::deferred_from_encoded_data(&data, None)
    {
        // Next, try interpreting the data as an encoded bitmap
        this.content = Content::Bitmap(image);
    } else if let Ok(mut dom) = svg::Dom::from_bytes(
        &data,
        FontLibrary::with_shared(|lib| lib.font_mgr()),
    ) {
        // Finally, try parsing as SVG
        let root = dom.root();

        let mut size = root.intrinsic_size();
        if size.is_empty() {
            // flag that image lacks an intrinsic size so it will be drawn to
            // match the canvas size if dimensions aren't provided
            // in the drawImage() call
            this.autosized = true;

            // If width or height attributes aren't defined on the root `<svg>`
            // element, they will be reported as "100%". If only one
            // is defined, use it for both dimensions, and if both are missing
            // use the aspect ratio to scale the width vs a fixed
            // height of 150 (i.e., Chrome's behavior)
            let Length {
                value: width,
                unit: w_unit,
            } = root.width();
            let Length {
                value: height,
                unit: h_unit,
            } = root.height();
            size = match ((width, w_unit), (height, h_unit)) {
                // NB: only unitless numeric lengths are currently being
                // handled; values in em, cm, in, etc. are ignored,
                // but perhaps they should be converted to px?
                (
                    (100.0, LengthUnit::Percentage),
                    (height, LengthUnit::Number),
                ) => (*height, *height).into(),
                (
                    (width, LengthUnit::Number),
                    (100.0, LengthUnit::Percentage),
                ) => (*width, *width).into(),
                _ => {
                    let aspect = root
                        .view_box()
                        .map(|vb| vb.width() / vb.height())
                        .unwrap_or(1.0);
                    (150.0 * aspect, 150.0).into()
                }
            };
        };

        // Save the SVG contents as a Picture (to be drawn later)
        let bounds = Rect::from_size(size);
        let mut compositor = PictureRecorder::new();
        dom.set_container_size(bounds.size());
        dom.render(compositor.begin_recording(bounds, true));
        this.content = match compositor.finish_recording_as_picture(None) {
            Some(picture) => Content::Vector(picture, size),
            None => Content::Broken,
        };
    } else {
        this.content = Content::Broken
    }

    this.sync_memory(&mut cx);
    Ok(cx.boolean(this.content.is_drawable()))
}

pub fn get_width(mut cx: FunctionContext) -> JsResult<JsValue> {
    let this = cx.argument::<BoxedImage>(0)?;
    let this = this.borrow();
    Ok(cx.number(this.content.size().width).upcast())
}

pub fn get_height(mut cx: FunctionContext) -> JsResult<JsValue> {
    let this = cx.argument::<BoxedImage>(0)?;
    let this = this.borrow();
    Ok(cx.number(this.content.size().height).upcast())
}

pub fn get_complete(mut cx: FunctionContext) -> JsResult<JsBoolean> {
    let this = cx.argument::<BoxedImage>(0)?;
    let this = this.borrow();
    Ok(cx.boolean(this.content.is_complete()))
}

pub fn pixels(mut cx: FunctionContext) -> JsResult<JsValue> {
    let this = cx.argument::<BoxedImage>(0)?;
    let this = this.borrow_mut();
    let (color_type, color_space) = image_data_settings_arg(&mut cx, 1)?;

    let info = ImageInfo::new(
        this.content.size().to_floor(),
        color_type,
        AlphaType::Unpremul,
        color_space,
    );
    let mut pixels = cx.buffer(
        info.bytes_per_pixel() * (info.width() * info.height()) as usize,
    )?;

    match &this.content {
        Content::Bitmap(image) => {
            match image.read_pixels(
                &info,
                pixels.as_mut_slice(&mut cx),
                info.min_row_bytes(),
                (0, 0),
                skia_safe::image::CachingHint::Allow,
            ) {
                true => Ok(pixels.upcast()),
                false => Ok(cx.undefined().upcast()),
            }
        }
        _ => Ok(cx.undefined().upcast()),
    }
}
