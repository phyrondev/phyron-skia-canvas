use crc::{CRC_32_ISO_HDLC, Crc};
use dashmap::DashMap;
use little_exif::{
    exif_tag::ExifTag, filetype::FileExtension, metadata::Metadata,
};
use neon::prelude::*;
use rayon::prelude::*;
use skia_safe::{
    AlphaType, Canvas as SkCanvas, ClipOp, Color, ColorSpace, ColorType, Data,
    Document, IRect, ISize, Image as SkImage, ImageInfo, Matrix, Path, Picture,
    PictureRecorder, PixelGeometry, Rect, Size, Surface, SurfaceProps,
    SurfacePropsFlags,
    image::CachingHint,
    images, jpeg_encoder, pdf, png_encoder, surfaces,
    svg::{self, canvas::Flags},
    webp_encoder,
};
use std::{
    fs,
    path::Path as FilePath,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};
const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

use super::transfer::{self, HdrTransfer};
use crate::{
    canvas::BoxedCanvas, context::BoxedContext2D, gpu::RenderingEngine,
};

static CACHE: OnceLock<Arc<DashMap<usize, PageCache>>> = OnceLock::new();

//
// Deferred canvas (records drawing commands for later replay on an output
// surface)
//

pub struct PageRecorder {
    current: PictureRecorder,
    layers: Vec<Picture>,
    bounds: Rect,
    matrix: Matrix,
    clip: Option<Path>,
    surface: RecordingSurface,
    changed: bool,
    id: usize,
}

impl PageRecorder {
    pub fn new(bounds: Rect) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(1);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        PageCache::add(id);

        let mut rec = PictureRecorder::new();
        rec.begin_recording(bounds, true).save(); // start at depth 2

        PageRecorder {
            current: rec,
            layers: vec![],
            changed: false,
            matrix: Matrix::default(),
            clip: None,
            bounds,
            id,
            surface: RecordingSurface::default(),
        }
    }

    pub fn append<F>(&mut self, f: F)
    where
        F: FnOnce(&SkCanvas),
    {
        if let Some(canvas) = self.current.recording_canvas() {
            f(canvas);
            self.changed = true;
        }
    }

    pub fn set_bounds(&mut self, bounds: Rect) {
        *self = PageRecorder::new(bounds);
    }

    pub fn update_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds; // non-destructively update the size
    }

    pub fn set_matrix(&mut self, matrix: Matrix) {
        self.matrix = matrix;
        self.restore();
    }

    pub fn set_clip(&mut self, clip: &Option<Path>) {
        self.clip = clip.clone();
        self.restore();
    }

    pub fn restore(&mut self) {
        if let Some(canvas) = self.current.recording_canvas() {
            canvas.restore_to_count(1);
            canvas.save();
            if let Some(clip) = &self.clip {
                canvas.clip_path(
                    clip,
                    ClipOp::Intersect,
                    true, /* antialias */
                );
            }
            canvas.set_matrix(&self.matrix.into());
        }
    }

    pub fn get_pixels(
        &mut self,
        crop: IRect,
        opts: ExportOptions,
        engine: RenderingEngine,
    ) -> Result<Vec<u8>, String> {
        // return an empty buffer if the requested rect is entirely outside the
        // canvas
        if !self.bounds.intersects(Rect::from_irect(crop)) {
            return Ok(vec![
                0;
                opts.output_info(
                    crop.size(),
                    opts.raw_alpha_type()
                )
                .compute_min_byte_size()
            ]);
        }

        let page = self.get_page();
        self.surface.update(&page, &opts, &engine);
        self.surface.copy_pixels(crop, &opts)
    }

    pub fn get_page(&mut self) -> Page {
        if self.changed {
            // store layer as a drawable (so copies are deduplicated) wrapped in
            // a picture (so it can be sent to other threads)
            if let Some(pict) = self
                .current
                .finish_recording_as_drawable()
                .and_then(|mut drawable| {
                    let mut wrapper = PictureRecorder::new();
                    wrapper
                        .begin_recording(self.bounds, true)
                        .draw_drawable(&mut drawable, None);
                    wrapper.finish_recording_as_picture(None)
                })
            {
                self.layers.push(pict)
            }

            // resume recording
            self.current.begin_recording(self.bounds, true);
            self.changed = false;
            self.restore();
        }

        Page {
            layers: self.layers.clone(),
            bounds: self.bounds,
            id: self.id,
        }
    }

    pub fn get_page_for_export(
        &mut self,
        opts: &ExportOptions,
        engine: &RenderingEngine,
    ) -> Page {
        // update the PageCache with the surface bitmap (if it's valid for this
        // export)
        let page = self.get_page();
        if opts.is_raster()
            && let Some(image) =
                self.surface.snapshot_if_valid(&page, opts, engine)
        {
            PageCache::set(self.id, image, opts, self.surface.depth);
        }
        page
    }

    /// Rasterize the page for use as a `drawImage` source, in the canvas
    /// working colour type and space so precision, gamut and range survive.
    pub fn get_image(
        &mut self,
        color_type: ColorType,
        color_space: &ColorSpace,
    ) -> Option<SkImage> {
        let info = ImageInfo::new(
            self.bounds.size().to_floor(),
            color_type,
            AlphaType::Premul,
            color_space.clone(),
        );
        let picture = self.get_page().get_picture(None)?;
        let mut surface = surfaces::raster(&info, None, None)?;
        surface.canvas().draw_picture(&picture, None, None);
        Some(surface.image_snapshot())
    }
}

impl Drop for PageRecorder {
    fn drop(&mut self) {
        PageCache::drop(self.id);
    }
}

//
// Persistent GPU/CPU surface for caching intermediate results of getImageData()
//

pub struct RecordingSurface {
    surface: Option<Surface>,
    depth: usize,
    matte: Option<Color>,
    msaa: Option<usize>,
    gpu: Option<bool>,
    color_type: ColorType,
    color_space: ColorSpace,
    density: f32,
}

impl Default for RecordingSurface {
    fn default() -> Self {
        Self {
            surface: None,
            depth: 0,
            matte: None,
            msaa: None,
            gpu: None,
            color_type: ColorType::RGBA8888,
            color_space: ColorSpace::new_srgb(),
            density: 0.0,
        }
    }
}

impl RecordingSurface {
    fn is_surface_stale(
        &mut self,
        page: &Page,
        opts: &ExportOptions,
        engine: &RenderingEngine,
    ) -> bool {
        let gpu_toggled =
            self.gpu != Some(matches!(engine, RenderingEngine::GPU));
        let page_size = page.scaled_dimensions(opts.density);
        let resized = self
            .surface
            .as_mut()
            .map(|surface| {
                let info = surface.image_info();
                info.dimensions() != page_size
                    || info.color_type() != opts.working_color_type
                    || info.color_space().as_ref()
                        != Some(&opts.working_color_space)
            })
            .unwrap_or(true);

        gpu_toggled || resized
    }

    fn is_config_stale(&self, opts: &ExportOptions) -> bool {
        self.density != opts.density
            || self.matte != opts.matte
            || self.msaa != opts.msaa
            || self.color_type != opts.working_color_type
            || self.color_space != opts.working_color_space
    }

    pub fn update(
        &mut self,
        page: &Page,
        opts: &ExportOptions,
        engine: &RenderingEngine,
    ) {
        // check for anything that would invalidate the previous contents
        let reconfigure = self.is_config_stale(opts);
        let recreate = self.is_surface_stale(page, opts, engine);

        // start from scratch if invalidated
        if reconfigure || recreate {
            self.gpu = Some(matches!(engine, RenderingEngine::GPU));
            self.color_type = opts.working_color_type;
            self.color_space = opts.working_color_space.clone();
            self.density = opts.density;
            self.matte = opts.matte;
            self.msaa = opts.msaa;
            self.depth = 0;

            // only allocate a new surface if the dimensions (size * density)
            // have changed or engine switched
            if recreate {
                let page_size = page.scaled_dimensions(opts.density);
                let img_info = ImageInfo::new(
                    page_size,
                    opts.working_color_type,
                    AlphaType::Premul,
                    opts.working_color_space.clone(),
                );
                self.surface = engine.make_surface(&img_info, opts).ok();
            }
        }

        if let Some(surface) = self.surface.as_mut() {
            let canvas = surface.canvas();
            let (cache_image, cache_depth) =
                PageCache::get(page.id, opts, page.depth());

            if let Some(image) = cache_image {
                // use the cached bitmap as the background (if present)
                canvas.draw_image(image, (0, 0), None);
                self.depth = cache_depth;
            } else if self.depth == 0 {
                // otherwise, fill the canvas if requested
                canvas.clear(self.matte.unwrap_or(Color::TRANSPARENT));
            }

            // only add new layers to surface
            canvas.scale((self.density, self.density));

            // draw newly added layers
            for pict in page.layers.iter().skip(self.depth) {
                pict.playback(canvas);
            }
            self.depth = page.layers.len();
        }
    }

    pub fn snapshot_if_valid(
        &mut self,
        page: &Page,
        opts: &ExportOptions,
        engine: &RenderingEngine,
    ) -> Option<SkImage> {
        match !(self.is_config_stale(opts)
            || self.is_surface_stale(page, opts, engine)
            || self.depth == 0)
        {
            true => self
                .surface
                .as_mut()
                .map(|surface| surface.image_snapshot()),
            false => None,
        }
    }

    /// Read `crop` from the working surface in the output encoding of
    /// `opts`.
    pub fn copy_pixels(
        &mut self,
        crop: IRect,
        opts: &ExportOptions,
    ) -> Result<Vec<u8>, String> {
        self.surface
            .as_mut()
            .ok_or_else(|| {
                "Could not allocate a surface for getImageData".to_string()
            })
            .and_then(|surface| {
                read_output(surface, crop, opts, opts.raw_alpha_type())
            })
    }
}

//
// Image generator for a single drawing context
//

#[derive(Debug, Clone)]
pub struct Page {
    pub id: usize,
    pub bounds: Rect,
    pub layers: Vec<Picture>,
}

impl PartialEq for Page {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.depth() == other.depth()
    }
}

impl Default for Page {
    fn default() -> Self {
        Self {
            id: 0,
            bounds: skia_safe::Rect::new_empty(),
            layers: vec![],
        }
    }
}

impl Page {
    pub fn depth(&self) -> usize {
        self.layers.len()
    }

    pub fn scaled_dimensions(&self, density: f32) -> ISize {
        Size::new(
            self.bounds.width() * density,
            self.bounds.height() * density,
        )
        .to_floor()
    }

    pub fn get_picture(&self, matte: Option<Color>) -> Option<Picture> {
        let mut compositor = PictureRecorder::new();
        let output = compositor.begin_recording(self.bounds, true);
        matte.map(|c| output.clear(c));
        self.layers.iter().for_each(|pict| pict.playback(output));
        compositor.finish_recording_as_picture(None)
    }

    pub fn encoded_as(
        &self,
        options: ExportOptions,
        engine: RenderingEngine,
    ) -> Result<Vec<u8>, String> {
        if self.bounds.is_empty() {
            return Err(
                "Width and height must be non-zero to generate an image"
                    .to_string(),
            );
        }

        let ExportOptions {
            ref format,
            quality,
            density,
            matte,
            ..
        } = options;
        let size = self.bounds.size();
        let img_dims = self.scaled_dimensions(density);
        // composite in the canvas working space; the output encoding is
        // applied once, when pixels are read or encoded
        let img_info = ImageInfo::new(
            img_dims,
            options.working_color_type,
            AlphaType::Premul,
            options.working_color_space.clone(),
        );
        let img_quality = ((quality * 100.0) as u32).clamp(0, 100);
        let img_scale = Matrix::scale((density, density)).into();

        match format.as_str() {
            "pdf" => {
                let mut pdf_bytes = Vec::new();
                let metadata = pdf::Metadata {
                    producer: "Skia Canvas <https://skia-canvas.org>"
                        .to_string(),
                    encoding_quality: Some((quality * 100.0) as i32),
                    raster_dpi: Some(density * 72.0),
                    ..Default::default()
                };
                let mut document = pdf_document(&mut pdf_bytes, &metadata)
                    .begin_page(size, None);
                let canvas = document.canvas();
                let picture = self
                    .get_picture(matte)
                    .ok_or("Could not generate an image")?;
                canvas.draw_picture(&picture, None, None);
                document.end_page().close();
                Ok(pdf_bytes)
            }

            "svg" => {
                let canvas = svg::Canvas::new(
                    Rect::from_size(size),
                    options.svg_flags(),
                );
                let picture = self
                    .get_picture(matte)
                    .ok_or("Could not generate an image")?;
                canvas.draw_picture(&picture, None, None);
                Ok(canvas.end().as_bytes().to_vec())
            }

            // handle bitmap formats using (potentially gpu-backed) rasterizer
            _ => {
                let mut surface = engine.make_surface(&img_info, &options)?;
                let canvas = surface.canvas();

                let (cache_image, cache_depth) =
                    PageCache::get(self.id, &options, self.depth());
                if let Some(image) = cache_image {
                    // use the cached bitmap as the background
                    canvas.draw_image(image, (0, 0), None);
                } else if let Some(color) = options.matte {
                    // otherwise, fill the canvas if requested
                    canvas.clear(color);
                }

                // draw newly added layers and cache the full-canvas bitmap
                canvas.set_matrix(&img_scale);
                for pict in self.layers.iter().skip(cache_depth) {
                    pict.playback(canvas);
                }

                // extract the results
                let context = &mut surface.direct_context();
                let image = surface
                    .make_temporary_image()
                    .unwrap_or_else(|| surface.image_snapshot());

                // update cache
                if self.depth() > cache_depth {
                    if rayon::current_thread_index().is_some() {
                        // move bitmap off GPU if we're in a background thread
                        // and need to share
                        if let Some(raster) = image.make_non_texture_image(
                            &mut surface.direct_context(),
                        ) {
                            PageCache::set(
                                self.id,
                                raster,
                                &options,
                                self.depth(),
                            )
                        }
                    } else {
                        PageCache::set(
                            self.id,
                            image.clone(),
                            &options,
                            self.depth(),
                        );
                    }
                }

                let full = IRect::from_size(img_dims);
                if format == "raw" {
                    return read_output(
                        &mut surface,
                        full,
                        &options,
                        options.raw_alpha_type(),
                    );
                }

                // the encoders take the image in the output encoding
                let image = match options.is_working_encoding() {
                    true => image,
                    false => {
                        let info =
                            options.output_info(img_dims, AlphaType::Unpremul);
                        let pixels = read_output(
                            &mut surface,
                            full,
                            &options,
                            AlphaType::Unpremul,
                        )?;
                        images::raster_from_data(
                            &info,
                            Data::new_copy(&pixels),
                            info.min_row_bytes(),
                        )
                        .ok_or(format!(
                            "Could not convert to {:?} for {}",
                            options.color_type, format
                        ))?
                    }
                };

                // handle image encoding
                match format.as_str() {
                    "jpg" | "jpeg" => {
                        let jpg_opts = jpeg_encoder::Options {
                            quality: img_quality,
                            downsample: match options.jpeg_downsample {
                                true => {
                                    jpeg_encoder::Downsample::BothDirections
                                }
                                false => jpeg_encoder::Downsample::No,
                            },
                            ..jpeg_encoder::Options::default()
                        };

                        jpeg_encoder::encode_image(context, &image, &jpg_opts)
                            .map(|data| {
                                let mut bytes = data.as_bytes().to_vec();
                                let [l, r] =
                                    (72 * density as u16).to_be_bytes();
                                bytes.splice(
                                    13..18,
                                    [1, l, r, l, r].iter().cloned(),
                                );
                                bytes
                            })
                    }

                    "png" => {
                        let png_opts = png_encoder::Options::default();

                        png_encoder::encode_image(context, &image, &png_opts)
                            .map(|data| {
                                let mut bytes = data.as_bytes().to_vec();
                                let mut digest = CRC32.digest();
                                let [a, b, c, d] = ((72.0 * density * 39.3701)
                                    as u32)
                                    .to_be_bytes();
                                let phys = vec![
                                    b'p', b'H', b'Y', b's', a, b, c,
                                    d, // x-dpi
                                    a, b, c, d, // y-dpi
                                    1, // dots per meter
                                ];
                                digest.update(&phys);

                                let length = 9u32.to_be_bytes().to_vec();
                                let checksum =
                                    digest.finalize().to_be_bytes().to_vec();
                                bytes.splice(
                                    33..33,
                                    [length, phys, checksum].concat(),
                                );
                                bytes
                            })
                    }

                    "webp" => {
                        let mut webp_opts = webp_encoder::Options::default();
                        if img_quality == 100 {
                            webp_opts.compression =
                                webp_encoder::Compression::Lossless;
                            webp_opts.quality = 75.0;
                        } else {
                            webp_opts.compression =
                                webp_encoder::Compression::Lossy;
                            webp_opts.quality = img_quality as _;
                        }

                        webp_encoder::encode_image(context, &image, &webp_opts)
                            .map(|data| {
                                let mut bytes = data.as_bytes().to_vec();

                                // toggle EXIF flag in VP8X chunk
                                bytes[20] |= 1 << 3;

                                // append EXIF chunk with DPI
                                let dpi = (72.0 * density) as f64;
                                let mut exif = Metadata::new();
                                exif.set_tag(ExifTag::XResolution(vec![
                                    dpi.into(),
                                ]));
                                exif.set_tag(ExifTag::YResolution(vec![
                                    dpi.into(),
                                ]));
                                if let Ok(mut exif_bytes) =
                                    exif.as_u8_vec(FileExtension::WEBP)
                                {
                                    bytes.append(&mut exif_bytes);
                                }

                                // update file-length field in RIFF header
                                let file_size =
                                    ((bytes.len() - 8) as u32).to_le_bytes();
                                bytes.splice(4..8, file_size.iter().cloned());

                                bytes
                            })
                    }
                    _ => {
                        return Err(format!(
                            "Unsupported file format {}",
                            format
                        ));
                    }
                }
                .ok_or(format!("Could not encode as {}", format))
            }
        }
    }

    pub fn write(
        &self,
        filename: &str,
        options: ExportOptions,
        engine: RenderingEngine,
    ) -> Result<(), String> {
        let path = FilePath::new(&filename);
        let data = self.encoded_as(options, engine)?;
        fs::write(path, data)
            .map_err(|why| format!("{}: \"{}\"", why, path.display()))
    }

    /// Render this page into a raster surface configured from
    /// `surface_options` and read the resulting pixels into the
    /// caller-supplied `dst_info`.
    ///
    /// Splits the two color configurations `encoded_as("raw", ...)`
    /// conflates: `surface_options` decides the compositing pixel
    /// format + color space (e.g. linear F32 for HDR-capable
    /// blending), while `dst_info` decides the wire format
    /// Skia converts the snapshot into (e.g. 8-bit sRGB
    /// premultiplied or unpremultiplied for a canvas paint path).
    ///
    /// Returns the packed pixel buffer sized to
    /// `dst_info.compute_min_byte_size()`.
    pub fn render_raw(
        &self,
        surface_options: ExportOptions,
        dst_info: ImageInfo,
        engine: RenderingEngine,
    ) -> Result<Vec<u8>, String> {
        if self.bounds.is_empty() {
            return Err(
                "Width and height must be non-zero to generate an image"
                    .to_string(),
            );
        }
        let ExportOptions {
            density,
            matte,
            working_color_type,
            ref working_color_space,
            ..
        } = surface_options;
        let img_dims = self.scaled_dimensions(density);
        let img_info = ImageInfo::new(
            img_dims,
            working_color_type,
            AlphaType::Premul,
            working_color_space.clone(),
        );
        let img_scale = Matrix::scale((density, density)).into();

        let mut surface = engine.make_surface(&img_info, &surface_options)?;
        let canvas = surface.canvas();
        if let Some(color) = matte {
            canvas.clear(color);
        }
        canvas.set_matrix(&img_scale);
        for pict in self.layers.iter() {
            pict.playback(canvas);
        }

        let stride = dst_info.min_row_bytes();
        let mut buffer: Vec<u8> = vec![0; dst_info.compute_min_byte_size()];
        if surface.read_pixels(&dst_info, &mut buffer, stride, (0, 0)) {
            Ok(buffer)
        } else {
            Err(format!(
                "Could not read pixels into destination format ({:?} / {:?})",
                dst_info.color_type(),
                dst_info.alpha_type(),
            ))
        }
    }

    fn append_to<'a>(
        &self,
        doc: Document<'a>,
        matte: Option<Color>,
    ) -> Result<Document<'a>, String> {
        if !self.bounds.is_empty() {
            let mut doc = doc.begin_page(self.bounds.size(), None);
            let canvas = doc.canvas();
            if let Some(picture) = self.get_picture(matte) {
                canvas.draw_picture(&picture, None, None);
            }
            Ok(doc.end_page())
        } else {
            Err("Width and height must be non-zero to generate a PDF page"
                .to_string())
        }
    }
}

//
// Container for a canvas's entire stack of page contexts
//

pub struct PageSequence {
    pub pages: Vec<Page>,
    pub engine: RenderingEngine,
}

impl PageSequence {
    pub fn from(pages: Vec<Page>, engine: RenderingEngine) -> Self {
        PageSequence { pages, engine }
    }

    pub fn first(&self) -> &Page {
        &self.pages[0]
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    pub fn materialize(
        &mut self,
        engine: &RenderingEngine,
        options: &ExportOptions,
    ) {
        if !options.is_raster() {
            return;
        }
        for page in self.pages.iter_mut() {
            PageCache::materialize(page.id, engine, options);
        }
    }

    pub fn as_pdf(&self, options: ExportOptions) -> Result<Vec<u8>, String> {
        let ExportOptions {
            quality,
            density,
            matte,
            ..
        } = options;
        let mut pdf_bytes = Vec::new();
        let metadata = pdf::Metadata {
            producer: "Skia Canvas <https://skia-canvas.org>".to_string(),
            encoding_quality: Some((quality * 100.0) as i32),
            raster_dpi: Some(density * 72.0),
            ..Default::default()
        };
        self.pages
            .iter()
            .try_fold(pdf_document(&mut pdf_bytes, &metadata), |doc, page| {
                page.append_to(doc, matte)
            })
            .map(|doc| doc.close())?;
        Ok(pdf_bytes)
    }

    pub fn write_image(
        &self,
        pattern: &str,
        options: ExportOptions,
    ) -> Result<(), String> {
        self.first().write(pattern, options, self.engine)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn write_sequence(
        &self,
        pattern: &str,
        padding: f32,
        options: ExportOptions,
    ) -> Result<(), String> {
        let padding = match padding as i32 {
            -1 => (1.0 + (self.pages.len() as f32).log10().floor()) as usize,
            pad => pad as usize,
        };

        self.pages
            .par_iter()
            .enumerate()
            .try_for_each(|(pp, page)| {
                let folio = format!("{:0width$}", pp + 1, width = padding);
                let filename = pattern.replace("{}", folio.as_str());
                page.write(&filename, options.clone(), self.engine)
            })
    }

    pub fn write_pdf(
        &self,
        path: &str,
        options: ExportOptions,
    ) -> Result<(), String> {
        let path = FilePath::new(&path);
        match self.as_pdf(options) {
            Ok(document) => fs::write(path, document)
                .map_err(|why| format!("{}: \"{}\"", why, path.display())),
            Err(msg) => Err(msg),
        }
    }
}

//
// Cache for the last bitmap generated by a given Page
//

#[derive(Debug, Clone)]
struct PageCache {
    image: Option<SkImage>,
    color_type: ColorType,
    color_space: ColorSpace,
    density: f32,
    matte: Option<Color>,
    msaa: Option<usize>,
    depth: usize,
}

impl Default for PageCache {
    fn default() -> Self {
        Self {
            image: None,
            color_type: ColorType::RGBA8888,
            color_space: ColorSpace::new_srgb(),
            depth: 0,
            density: 1.0,
            matte: None,
            msaa: None,
        }
    }
}

impl PageCache {
    pub fn shared<'a>() -> &'a Arc<DashMap<usize, PageCache>> {
        CACHE.get_or_init(|| Arc::new(DashMap::new()))
    }

    pub fn add(id: usize) {
        Self::shared().insert(id, PageCache::default());
    }

    pub fn drop(id: usize) {
        Self::shared().remove(&id);
    }

    pub fn get(
        id: usize,
        opts: &ExportOptions,
        depth: usize,
    ) -> (Option<SkImage>, usize) {
        Self::shared()
            .get(&id)
            .map(|cache| match cache.is_valid(opts) && depth >= cache.depth {
                true => (cache.image.clone(), cache.depth),
                false => (None, 0),
            })
            .unwrap_or((None, 0))
    }

    pub fn set(id: usize, image: SkImage, opts: &ExportOptions, depth: usize) {
        if let Some(mut cache) = Self::shared().get_mut(&id) {
            // save the bitmap if it's newer than the cached version, or is
            // replacing an invaildated cache
            if !cache.is_valid(opts) || depth > cache.depth {
                *cache = Self {
                    image: Some(image),
                    color_type: opts.working_color_type,
                    color_space: opts.working_color_space.clone(),
                    density: opts.density,
                    matte: opts.matte,
                    msaa: opts.msaa,
                    depth,
                }
            }
        }
    }

    pub fn materialize(
        id: usize,
        engine: &RenderingEngine,
        options: &ExportOptions,
    ) {
        if let Some(mut cache) = Self::shared().get_mut(&id) {
            // nothing to be done if the image isn't currently in GPU memory
            // or if the options have changed (so the cache is invalid anyway)
            if let Some(ref img) = cache.image
                && (!cache.is_valid(options) || !img.is_texture_backed())
            {
                return;
            }

            // otherwise move the image to main memory
            engine.with_direct_context(|context| {
                cache.image = cache
                    .image
                    .as_ref()
                    .and_then(|img| img.make_non_texture_image(context))
            });
        }
    }

    #[cfg(not(any(feature = "metal", feature = "vulkan")))]
    fn _blit(
        &self,
        _surface: &mut Surface,
        dst_info: &ImageInfo,
        src: IRect,
        pixels: &mut [u8],
    ) -> Option<bool> {
        self.image.as_ref().map(|image| {
            image.read_pixels(
                dst_info,
                pixels,
                dst_info.min_row_bytes(),
                (src.x(), src.y()),
                CachingHint::Allow,
            )
        })
    }

    #[cfg(any(feature = "metal", feature = "vulkan"))]
    fn _blit(
        &self,
        surface: &mut Surface,
        dst_info: &ImageInfo,
        src: IRect,
        pixels: &mut [u8],
    ) -> Option<bool> {
        let context = &mut surface.direct_context();
        self.image.as_ref().map(|image| {
            image.read_pixels_with_context(
                context,
                dst_info,
                pixels,
                dst_info.min_row_bytes(),
                (src.x(), src.y()),
                CachingHint::Allow,
            )
        })
    }

    pub fn is_valid(&self, opts: &ExportOptions) -> bool {
        self.density == opts.density
            && self.color_type == opts.working_color_type
            && self.color_space == opts.working_color_space
            && self.matte == opts.matte
            && self.msaa == opts.msaa
            && self.image.is_some()
            && opts.is_raster()
    }
}

//
// Helpers
//

pub fn pages_arg(
    cx: &mut FunctionContext,
    idx: usize,
    opts: &ExportOptions,
    canvas: &BoxedCanvas,
) -> NeonResult<PageSequence> {
    let engine = canvas.borrow_mut().engine();
    let pages = cx
        .argument::<JsArray>(idx)?
        .to_vec(cx)?
        .iter()
        .map(|obj| obj.downcast::<BoxedContext2D, _>(cx))
        .filter(|ctx| ctx.is_ok())
        // SAFETY: `.filter(|ctx| ctx.is_ok())` ensures only `Ok` values reach
        // here.
        .map(|obj| obj.unwrap().borrow().get_page_for_export(opts, &engine))
        .collect();
    Ok(PageSequence::from(pages, engine))
}

fn pdf_document<'a>(
    buffer: &'a mut impl std::io::Write,
    metadata: &'a pdf::Metadata<'a>,
) -> Document<'a> {
    pdf::new_document(buffer, Some(metadata))
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportOptions {
    pub format: String,
    pub quality: f32,
    pub density: f32,
    pub outline: bool,
    pub matte: Option<Color>,
    pub msaa: Option<usize>,
    /// Output colour type.
    pub color_type: ColorType,
    /// Output colour space.
    pub color_space: ColorSpace,
    /// Compositing colour type (the canvas `colorType`).
    pub working_color_type: ColorType,
    /// Compositing colour space (the canvas `colorSpace`).
    pub working_color_space: ColorSpace,
    /// Premultiply colour by alpha in `raw` and `getImageData` output.
    pub premultiplied: bool,
    /// SDR reference white in nits for PQ and HLG output.
    pub hdr_reference_white: f32,
    pub jpeg_downsample: bool,
    pub text_contrast: f32,
    pub text_gamma: f32,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: "raw".to_string(),
            quality: 0.92,
            density: 1.0,
            matte: None,
            jpeg_downsample: false,
            text_contrast: 0.0,
            text_gamma: 1.4,
            msaa: None,
            color_type: ColorType::RGBA8888,
            color_space: ColorSpace::new_srgb(),
            working_color_type: ColorType::RGBA8888,
            working_color_space: ColorSpace::new_srgb(),
            premultiplied: false,
            hdr_reference_white: transfer::DEFAULT_REFERENCE_WHITE,
            outline: true,
        }
    }
}

impl ExportOptions {
    pub fn surface_props(&self) -> SurfaceProps {
        SurfaceProps::new_with_text_properties(
            SurfacePropsFlags::default(),
            PixelGeometry::Unknown,
            self.text_contrast,
            self.text_gamma,
        )
    }

    pub fn svg_flags(&self) -> Option<skia_safe::svg::canvas::Flags> {
        match self.outline {
            true => Some(Flags::CONVERT_TEXT_TO_PATHS),
            _ => None,
        }
    }

    pub fn msaa_from(&self, valid_msaa: &Vec<usize>) -> Result<usize, String> {
        let samples = self.msaa.unwrap_or_else(|| {
            if valid_msaa.contains(&4) {
                4
            }
            // 4x is a good default if available
            else {
                valid_msaa.last().copied().unwrap_or(0)
            }
        });
        match valid_msaa.contains(&samples) {
            true => Ok(samples),
            false => Err(format!(
                "{}x MSAA not supported by GPU (options: {:?})",
                samples, valid_msaa
            )),
        }
    }

    pub fn is_raster(&self) -> bool {
        self.format != "pdf" && self.format != "svg"
    }

    /// `true` when the output encoding equals the working space, so no
    /// conversion is needed.
    pub fn is_working_encoding(&self) -> bool {
        self.color_type == self.working_color_type
            && self.color_space == self.working_color_space
    }

    /// The output `ImageInfo` for `dimensions`.
    pub fn output_info(
        &self,
        dimensions: impl Into<ISize>,
        alpha_type: AlphaType,
    ) -> ImageInfo {
        ImageInfo::new(
            dimensions,
            self.color_type,
            alpha_type,
            self.color_space.clone(),
        )
    }

    /// The alpha type of `raw` and `getImageData` output.
    pub fn raw_alpha_type(&self) -> AlphaType {
        match self.premultiplied {
            true => AlphaType::Premul,
            false => AlphaType::Unpremul,
        }
    }
}

/// Read `crop` from a working-space surface in the output encoding of `opts`.
///
/// SDR outputs are converted by Skia. PQ and HLG outputs are read as linear
/// Rec.2020 floats and encoded by [`transfer`], because Skia supports only a
/// fixed reference white and approximates PQ.
fn read_output(
    surface: &mut Surface,
    crop: IRect,
    opts: &ExportOptions,
    alpha_type: AlphaType,
) -> Result<Vec<u8>, String> {
    let error = || format!("Could not read pixels as {:?}", opts.color_type);
    let read = |surface: &mut Surface, info: &ImageInfo| {
        let mut pixels = vec![0; info.compute_min_byte_size()];
        match surface.read_pixels(
            info,
            &mut pixels,
            info.min_row_bytes(),
            (crop.x(), crop.y()),
        ) {
            true => Ok(pixels),
            false => Err(error()),
        }
    };

    match HdrTransfer::of(&opts.color_space) {
        None => read(surface, &opts.output_info(crop.size(), alpha_type)),
        Some(hdr) => {
            let linear = transfer::rec2020_linear()
                .ok_or("Skia cannot construct linear Rec.2020")?;
            let float_info = |alpha_type| {
                ImageInfo::new(
                    crop.size(),
                    ColorType::RGBAF32,
                    alpha_type,
                    linear.clone(),
                )
            };
            let bytes = read(surface, &float_info(AlphaType::Unpremul))?;

            let mut rgba: Vec<f32> = bytes
                .chunks_exact(4)
                .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            hdr.encode(&mut rgba, opts.hdr_reference_white);
            if alpha_type == AlphaType::Premul {
                rgba.chunks_exact_mut(4).for_each(|pixel| {
                    let alpha = pixel[3];
                    pixel[..3].iter_mut().for_each(|c| *c *= alpha);
                });
            }
            let bytes: Vec<u8> =
                rgba.iter().flat_map(|v| v.to_ne_bytes()).collect();

            // same space and alpha type on both sides: Skia converts only
            // the colour type (clamp and quantization)
            let encoded_info = float_info(alpha_type);
            let dst_info = ImageInfo::new(
                crop.size(),
                opts.color_type,
                alpha_type,
                linear.clone(),
            );
            match opts.color_type {
                ColorType::RGBAF32 => Ok(bytes),
                _ => {
                    let image = images::raster_from_data(
                        &encoded_info,
                        Data::new_copy(&bytes),
                        encoded_info.min_row_bytes(),
                    )
                    .ok_or_else(error)?;
                    let mut pixels = vec![0; dst_info.compute_min_byte_size()];
                    match image.read_pixels(
                        &dst_info,
                        &mut pixels,
                        dst_info.min_row_bytes(),
                        (0, 0),
                        CachingHint::Disallow,
                    ) {
                        true => Ok(pixels),
                        false => Err(error()),
                    }
                }
            }
        }
    }
}
