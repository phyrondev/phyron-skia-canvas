//! Stable Rust-only facade for `skia-canvas`.
//!
//! Rust library consumers should use the types in this module; the Neon/JS
//! modules at the crate root are kept for Node addon compatibility and
//! intentionally leak `skia_safe` and `neon` types in their public
//! signatures.
//!
//! Public signatures in this module never expose `skia_safe` or `neon`
//! types -- a compile-time pin in
//! `tests/native_studio_renderer_adapter.rs` verifies this.
//!
//! See the [crate-level docs](crate) for a worked example. The repository
//! has a longer reference at [`docs/api/native-rust.md`][api-doc].
//!
//! [api-doc]: https://github.com/phyrondev/phyron-skia-canvas/blob/main/docs/api/native-rust.md

pub mod backend;
pub mod color;
pub mod error;
pub mod filter;
pub mod font;
pub mod geometry;
pub mod image;
pub mod paint;
pub mod path;
pub mod pixels;
pub mod recorder;
pub mod shader;
pub mod surface;
pub mod text;

pub use backend::{Backend, EngineKind, EngineStatus, RenderEngine};
pub use color::{LinearColorSpace, OutputColorSpace, RgbaLinear};
pub use error::Error;
pub use filter::{BlurStyle, ColorFilter, ImageFilter, MaskFilter};
pub use font::{FontAxisTag, FontManager, FontVariation, InvalidFontAxisTag};
pub use geometry::{Affine, Point, Rect, Size};
pub use image::Image;
pub use paint::{BlendMode, DashPattern, Paint, PaintStyle, StrokeCap};
pub use path::{FillRule, Path};
pub use pixels::{
    AlphaMode, ExportedPixels, PixelColorSpace, PixelDepth, PixelExportOptions,
    PixelFormat, RawFrame, RawFrameOptions, SamplingMode, SurfaceOptions,
};
pub use recorder::{Canvas, Recorder, SaveLayerOptions};
pub use shader::{GradientInterpolation, GradientStop, Shader};
pub use surface::Surface;
pub use text::{
    FontFeature, LineMetrics, RichTextSpan, StrutStyle, TextAlign,
    TextBoxOptions, TextDecoration, TextDecorationStyle, TextEngine,
    TextHeightBehavior, TextLayout, TextShadow, TextSlant, TextStyle,
    VerticalAlign,
};
