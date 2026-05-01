use std::fmt;

use crate::native::color::{LinearColorSpace, OutputColorSpace};
use crate::native::geometry::Rect;
use crate::native::pixels::PixelFormat;

#[derive(Debug, Clone, PartialEq)]
pub enum NativeError {
    InvalidDimensions { width: f32, height: f32 },
    InvalidRect { rect: Rect },
    UnsupportedColorSpace { color_space: LinearColorSpace },
    UnsupportedOutputColorSpace { color_space: OutputColorSpace },
    UnsupportedPixelFormat { pixel_format: PixelFormat },
    DecodeImage { reason: String },
    Render { reason: String },
}

impl fmt::Display for NativeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid dimensions: {width}x{height}")
            }
            Self::InvalidRect { rect } => write!(f, "invalid rect: {rect:?}"),
            Self::UnsupportedColorSpace { color_space } => {
                write!(f, "unsupported linear color space: {color_space:?}")
            }
            Self::UnsupportedOutputColorSpace { color_space } => {
                write!(f, "unsupported output color space: {color_space:?}")
            }
            Self::UnsupportedPixelFormat { pixel_format } => {
                write!(f, "unsupported pixel format: {pixel_format:?}")
            }
            Self::DecodeImage { reason } => write!(f, "decode image failed: {reason}"),
            Self::Render { reason } => write!(f, "render failed: {reason}"),
        }
    }
}

impl std::error::Error for NativeError {}
