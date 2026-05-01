use skia_safe::ColorSpace as SkColorSpace;
use skia_safe::{named_primaries, named_transfer_fn};

use crate::native::error::NativeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinearColorSpace {
    Srgb,
    DisplayP3,
    Rec2020,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputColorSpace {
    Srgb,
    DisplayP3,
    Rec2020,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbaLinear {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl RgbaLinear {
    pub fn new_premultiplied(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn opaque(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub fn with_opacity(self, opacity: f32) -> Self {
        let clamped = opacity.clamp(0.0, 1.0);
        Self {
            r: self.r * clamped,
            g: self.g * clamped,
            b: self.b * clamped,
            a: self.a * clamped,
        }
    }
}

impl LinearColorSpace {
    pub(crate) fn to_skia_color_space(self) -> Result<SkColorSpace, NativeError> {
        match self {
            Self::Srgb => Ok(SkColorSpace::new_srgb_linear()),
            Self::DisplayP3 => SkColorSpace::new_cicp(
                named_primaries::CicpId::SMPTE_EG_432_1,
                named_transfer_fn::CicpId::Linear,
            )
            .ok_or(NativeError::UnsupportedColorSpace { color_space: self }),
            Self::Rec2020 => SkColorSpace::new_cicp(
                named_primaries::CicpId::Rec2020,
                named_transfer_fn::CicpId::Linear,
            )
            .ok_or(NativeError::UnsupportedColorSpace { color_space: self }),
        }
    }
}

impl OutputColorSpace {
    pub(crate) fn to_skia_color_space(self) -> Result<SkColorSpace, NativeError> {
        match self {
            Self::Srgb => Ok(SkColorSpace::new_srgb()),
            Self::DisplayP3 => SkColorSpace::new_cicp(
                named_primaries::CicpId::SMPTE_EG_432_1,
                named_transfer_fn::CicpId::IEC61966_2_1,
            )
            .ok_or(NativeError::UnsupportedOutputColorSpace { color_space: self }),
            Self::Rec2020 => SkColorSpace::new_cicp(
                named_primaries::CicpId::Rec2020,
                named_transfer_fn::CicpId::Rec709,
            )
            .ok_or(NativeError::UnsupportedOutputColorSpace { color_space: self }),
        }
    }
}
