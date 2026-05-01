use crate::native::error::NativeError;
use crate::native::pixels::SurfaceOptions;
use crate::native::surface::NativeSurface;

/// Entry point for the Rust-only `phyron-skia-canvas` API. Owns construction
/// of surfaces and any future shared resource registries. Currently a thin
/// CPU-only factory; GPU contexts will be added later without changing the
/// public surface.
#[derive(Debug, Default)]
pub struct NativeBackend {
    _private: (),
}

impl NativeBackend {
    pub fn new() -> Self {
        Self { _private: () }
    }

    pub fn create_surface(
        &self,
        width: u32,
        height: u32,
        options: SurfaceOptions,
    ) -> Result<NativeSurface, NativeError> {
        NativeSurface::new(width, height, options)
    }
}
