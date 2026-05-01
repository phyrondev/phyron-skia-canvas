//! # Native Rust API
//!
//! Stable Rust-only facade for `phyron-skia-canvas`. Rust library consumers
//! should use the types in this module; the Neon/JS modules at the crate root
//! are kept for Node addon compatibility and intentionally leak Skia and Neon
//! types in their public signatures.
//!
//! Public signatures in this module never expose `skia_safe` or `neon` types.

pub mod backend;
pub mod color;
pub mod error;
pub mod geometry;
pub mod image;
pub mod paint;
pub mod pixels;
pub mod recorder;
pub mod surface;
pub mod text;

pub use backend::NativeBackend;
pub use color::{LinearColorSpace, OutputColorSpace, RgbaLinear};
pub use error::NativeError;
pub use geometry::{Point, Rect, Size};
pub use image::NativeImage;
pub use paint::ShapePaint;
pub use pixels::{
    AlphaMode, ExportedPixels, PixelColorSpace, PixelDepth, PixelExportOptions, PixelFormat,
    RawFrame, RawFrameOptions, SurfaceOptions,
};
pub use recorder::{NativeCanvas, NativeRecorder};
pub use surface::NativeSurface;
pub use text::{TextAlign, TextBoxOptions, VerticalAlign};
