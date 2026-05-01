use skia_safe::{AlphaType, ColorType};

use crate::native::color::{LinearColorSpace, OutputColorSpace};
use crate::native::error::NativeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    Rgba8UnormPremul,
    Rgba8UnormUnpremul,
    Rgba16fPremul,
    Rgba32fPremul,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlphaMode {
    Premultiplied,
    Unpremultiplied,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceOptions {
    pub color_space: LinearColorSpace,
    pub density: f32,
    pub msaa: Option<usize>,
}

impl Default for SurfaceOptions {
    fn default() -> Self {
        Self {
            color_space: LinearColorSpace::Srgb,
            density: 1.0,
            msaa: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawFrameOptions {
    pub pixel_format: PixelFormat,
    pub color_space: OutputColorSpace,
}

impl Default for RawFrameOptions {
    fn default() -> Self {
        Self {
            pixel_format: PixelFormat::Rgba8UnormUnpremul,
            color_space: OutputColorSpace::Srgb,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawFrame {
    width: u32,
    height: u32,
    stride: usize,
    pixel_format: PixelFormat,
    color_space: OutputColorSpace,
    pixels: Vec<u8>,
}

impl RawFrame {
    pub(crate) fn new(
        width: u32,
        height: u32,
        stride: usize,
        pixel_format: PixelFormat,
        color_space: OutputColorSpace,
        pixels: Vec<u8>,
    ) -> Self {
        Self {
            width,
            height,
            stride,
            pixel_format,
            color_space,
            pixels,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn stride(&self) -> usize {
        self.stride
    }

    pub fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    pub fn color_space(&self) -> OutputColorSpace {
        self.color_space
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }
}

impl PixelFormat {
    pub(crate) fn to_skia_color_type(self) -> Result<ColorType, NativeError> {
        match self {
            Self::Rgba8UnormPremul | Self::Rgba8UnormUnpremul => Ok(ColorType::RGBA8888),
            Self::Rgba16fPremul => Ok(ColorType::RGBAF16),
            Self::Rgba32fPremul => Ok(ColorType::RGBAF32),
        }
    }

    pub(crate) fn to_skia_alpha_type(self) -> AlphaType {
        match self {
            Self::Rgba8UnormUnpremul => AlphaType::Unpremul,
            Self::Rgba8UnormPremul | Self::Rgba16fPremul | Self::Rgba32fPremul => AlphaType::Premul,
        }
    }

    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgba8UnormPremul | Self::Rgba8UnormUnpremul => 4,
            Self::Rgba16fPremul => 8,
            Self::Rgba32fPremul => 16,
        }
    }
}
