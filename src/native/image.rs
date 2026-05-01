use skia_safe::{Data, Image as SkImage};

use crate::native::error::NativeError;

#[derive(Debug, Clone)]
pub struct NativeImage {
    pub(crate) inner: SkImage,
}

impl NativeImage {
    pub fn from_encoded(bytes: &[u8]) -> Result<Self, NativeError> {
        let data = Data::new_copy(bytes);
        let image = SkImage::from_encoded(data).ok_or_else(|| NativeError::DecodeImage {
            reason: "skia could not decode the encoded image bytes".to_string(),
        })?;
        Ok(Self { inner: image })
    }

    pub fn width(&self) -> u32 {
        self.inner.width().max(0) as u32
    }

    pub fn height(&self) -> u32 {
        self.inner.height().max(0) as u32
    }
}
