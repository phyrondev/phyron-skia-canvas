#![allow(clippy::upper_case_acronyms)]
use crate::context::page::{ExportOptions, Page};
use serde_json::{Value, json};
use skia_safe::{
    Color, ColorType, Image, ImageInfo, Matrix, Rect, Surface,
    gpu::DirectContext, surfaces,
};
use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// Set once the GPU contexts are released for process exit. After that, GPU
/// surface requests fail and exports fall back to CPU raster.
#[cfg_attr(not(feature = "node-addon"), allow(dead_code))]
static CONTEXTS_RELEASED: AtomicBool = AtomicBool::new(false);

/// `true` once [`release_contexts`] ran.
#[cfg_attr(not(any(feature = "vulkan", feature = "metal")), allow(dead_code))]
pub(crate) fn contexts_released() -> bool {
    CONTEXTS_RELEASED.load(Ordering::SeqCst)
}

/// Drop the rayon workers' GPU contexts and stop the idle watcher, before
/// process exit.
///
/// AIDEV-NOTE: worker contexts that are still alive at exit crash the process
/// with `SIGSEGV` when several processes use the GPU. Measured with parallel
/// `node --test` on a Vulkan host: 5 of 12 stressed runs crashed; waiting out
/// the 5 s context lifespan, or this release, gave 0 of 12 and 0 of 20. Do not
/// drop the calling thread's context: surfaces that JS objects own (the
/// `getImageData` surface) belong to it and are freed during exit teardown,
/// which then crashes every time.
#[cfg_attr(not(feature = "node-addon"), allow(dead_code))]
pub fn release_contexts() {
    CONTEXTS_RELEASED.store(true, Ordering::SeqCst);
    Engine::release_contexts();
}

#[cfg(feature = "metal")]
mod metal;
#[cfg(feature = "metal")]
use crate::gpu::metal::MetalEngine as Engine;
#[cfg(all(feature = "metal", feature = "window"))]
pub use crate::gpu::metal::MetalRenderer as Renderer;

#[cfg(feature = "vulkan")]
mod vulkan;
#[cfg(feature = "vulkan")]
use crate::gpu::vulkan::engine::VulkanEngine as Engine;
#[cfg(all(feature = "vulkan", feature = "window"))]
pub use crate::gpu::vulkan::renderer::VulkanRenderer as Renderer;

#[cfg(not(any(feature = "vulkan", feature = "metal")))]
struct Engine {}
#[cfg(not(any(feature = "vulkan", feature = "metal")))]
impl Engine {
    pub fn api() -> Option<String> {
        None
    }

    pub fn supported() -> bool {
        false
    }

    pub fn status() -> Value {
        serde_json::json!({
            "renderer": "CPU",
            "api": Value::Null,
            "device": "CPU-based renderer (compiled without GPU support)",
            "error": Value::Null,
        })
    }

    // placeholders that match the GPU signatures (for the type-checker) but
    // will never be called (see the RenderingEngine methods for their
    // inline implementation when in CPU mode)
    pub fn make_surface(
        _info: &ImageInfo,
        _opts: &ExportOptions,
    ) -> Result<Surface, String> {
        panic!()
    }

    pub fn release_contexts() {}

    pub fn with_direct_context(_f: impl FnOnce(Option<&mut DirectContext>)) {
        panic!()
    }
}

#[derive(Copy, Clone, Debug)]
pub enum RenderingEngine {
    CPU,
    GPU,
}

impl Default for RenderingEngine {
    fn default() -> Self {
        if Engine::supported() {
            Self::GPU
        } else {
            Self::CPU
        }
    }
}

/// Shared record of the colour type whose last surface fell back from the GPU
/// to CPU raster. A canvas owns one and hands clones to its exports, which
/// run on other threads.
#[derive(Clone, Default)]
pub struct FallbackCell(Arc<Mutex<Option<ColorType>>>);

impl FallbackCell {
    pub fn get(&self) -> Option<ColorType> {
        self.0.lock().ok().and_then(|guard| *guard)
    }

    fn set(&self, color_type: Option<ColorType>) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = color_type;
        }
    }
}

/// Export options compare equal whatever cell they report to.
impl PartialEq for FallbackCell {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl fmt::Debug for FallbackCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("FallbackCell").field(&self.get()).finish()
    }
}

#[allow(dead_code)]
impl RenderingEngine {
    pub fn selectable(&self) -> bool {
        match self {
            Self::GPU => Engine::supported(),
            Self::CPU => true,
        }
    }

    pub fn make_surface(
        &self,
        image_info: &ImageInfo,
        opts: &ExportOptions,
    ) -> Result<Surface, String> {
        let raster = || {
            surfaces::raster(image_info, None, Some(&opts.surface_props()))
                .ok_or(format!(
                    "Could not allocate new {}×{} bitmap (color type: {:?})",
                    image_info.width(),
                    image_info.height(),
                    image_info.color_type()
                ))
        };
        match self {
            // AIDEV-NOTE: a GPU may not allocate float or 16-bit surfaces
            // (spec 001, contracts/gpu-fallback.md); render those with CPU
            // raster and report it in `canvas.engine.fallback`.
            Self::GPU => match Engine::make_surface(image_info, opts) {
                Ok(surface) => {
                    opts.fallback.set(None);
                    Ok(surface)
                }
                Err(_) => raster().inspect(|_| {
                    opts.fallback.set(Some(image_info.color_type()))
                }),
            },
            Self::CPU => raster().inspect(|_| opts.fallback.set(None)),
        }
    }

    pub fn with_direct_context(
        &self,
        f: impl FnOnce(Option<&mut DirectContext>),
    ) {
        match self {
            Self::GPU => Engine::with_direct_context(f),
            Self::CPU => f(None),
        }
    }

    pub fn status(&self, is_manually_disabled: bool) -> serde_json::Value {
        match is_manually_disabled {
            true => json!({
                "renderer":"CPU",
                "api": Engine::api(),
                "device": "CPU-based renderer (GPU manually disabled)",
                "driver": "N/A",
                "threads": rayon::current_num_threads()
            }),
            false => Engine::status(),
        }
    }

    pub fn lacks_gpu_support(&self) -> Option<String> {
        match Engine::supported() {
            true => None,
            false => {
                let mut msg = vec!["No windowing support".to_string()];
                if let Some(Value::String(error)) =
                    Engine::status().get("error")
                {
                    msg.push(error.to_string());
                }
                Some(msg.join(": "))
            }
        }
    }
}

/// Get the default backend status without creating a canvas.
/// Returns JSON with renderer (CPU/GPU), api, device, driver, threads, and
/// error fields.
pub fn get_backend_status() -> serde_json::Value {
    let mut status = Engine::status();
    // Add thread count for CPU info.
    if let serde_json::Value::Object(ref mut map) = status {
        map.insert("threads".to_string(), json!(rayon::current_num_threads()));
        map.insert("gpuAvailable".to_string(), json!(Engine::supported()));
    }
    status
}

#[allow(dead_code)]
pub struct RenderCache {
    image: Option<Image>,
    content: Rect,
    page: Page,
    matte: Color,
    dpr: f32,
    state: RenderState,
}

impl Default for RenderCache {
    fn default() -> Self {
        Self {
            image: None,
            content: Rect::new_empty(),
            page: Page::default(),
            dpr: 0.0,
            matte: Color::TRANSPARENT,
            state: RenderState::Clean,
        }
    }
}

#[allow(dead_code)]
impl RenderCache {
    pub fn validate(
        &mut self,
        page: &Page,
        matte: Color,
        dpr: f32,
        clip: Rect,
    ) -> Option<(&Image, &Rect, Rect)> {
        if self.state == RenderState::Dirty
            || self.page.id != page.id
            || self.matte != matte
            || self.dpr != dpr
        {
            *self = Self::default();
        }

        self.image.as_ref().map(|img| {
            let (dst, _) = Matrix::scale((dpr, dpr)).map_rect(clip);
            (img, &self.content, dst)
        })
    }

    pub fn depth(&self) -> usize {
        self.page.layers.len()
    }

    pub fn update(
        &mut self,
        image: Image,
        page: &Page,
        matte: Color,
        dpr: f32,
        content: Rect,
    ) {
        if self.state == RenderState::Resizing {
            // mark the framebuffer as needing a full redraw and skip updating
            // cached image during resize
            self.state = RenderState::Dirty;
        } else {
            let state = RenderState::Clean;
            let (content, _) =
                skia_safe::Matrix::scale((dpr, dpr)).map_rect(content);
            *self = Self {
                image: Some(image),
                page: page.clone(),
                matte,
                dpr,
                content,
                state,
            };
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RenderState {
    Clean,
    Dirty,
    Resizing,
}
