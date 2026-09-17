//! PQ and HLG transfer functions for HDR output encoding.
//!
//! Skia constructs PQ and HLG colour spaces only with a fixed SDR reference
//! white of 203 nits, and its raster PQ stage uses an approximate `powf`. So the
//! export path reads linear Rec.2020 floats from Skia and encodes them here with
//! the exact ST 2084 and BT.2100 formulas (see
//! `specs/001-wide-gamut-hdr-export/research.md`).
//!
//! All functions take unpremultiplied RGBA `f32` pixels, where linear `1.0` is
//! the SDR reference white. Alpha is not changed.

#[cfg(test)]
mod tests;

/// SDR reference white in nits (ITU-R BT.2408).
pub const DEFAULT_REFERENCE_WHITE: f32 = 203.0;

/// Peak luminance of the PQ signal range in nits.
const PQ_PEAK: f64 = 10_000.0;

/// Nominal peak luminance of the HLG display in nits (BT.2100).
const HLG_PEAK: f64 = 1_000.0;

/// HLG system gamma for a 1000-nit display (BT.2100).
const HLG_GAMMA: f64 = 1.2;

/// BT.2020 luma coefficients.
const LUMA: [f64; 3] = [0.2627, 0.6780, 0.0593];

/// ST 2084 inverse EOTF: absolute luminance in nits to a PQ signal in
/// `[0, 1]`.
pub fn pq_from_nits(nits: f64) -> f64 {
    const M1: f64 = 2610.0 / 16384.0;
    const M2: f64 = 2523.0 / 4096.0 * 128.0;
    const C1: f64 = 3424.0 / 4096.0;
    const C2: f64 = 2413.0 / 4096.0 * 32.0;
    const C3: f64 = 2392.0 / 4096.0 * 32.0;

    let y = (nits / PQ_PEAK).clamp(0.0, 1.0).powf(M1);
    ((C1 + C2 * y) / (1.0 + C3 * y)).powf(M2)
}

/// BT.2100 HLG OETF: normalized scene light in `[0, 1]` to an HLG signal in
/// `[0, 1]`.
pub fn hlg_from_scene(scene: f64) -> f64 {
    const A: f64 = 0.178_832_77;
    const B: f64 = 1.0 - 4.0 * A;
    let c = 0.5 - A * (4.0 * A).ln();

    let e = scene.clamp(0.0, 1.0);
    if e <= 1.0 / 12.0 {
        (3.0 * e).sqrt()
    } else {
        A * (12.0 * e - B).ln() + c
    }
}

/// Encode unpremultiplied linear RGBA pixels as PQ signals, in place.
pub fn encode_pq(rgba: &mut [f32], reference_white: f32) {
    let scale = f64::from(reference_white);
    rgba.chunks_exact_mut(4).for_each(|pixel| {
        pixel[..3].iter_mut().for_each(|channel| {
            *channel = pq_from_nits(f64::from(*channel) * scale) as f32;
        })
    });
}

/// Encode unpremultiplied linear RGBA pixels as display-referred HLG signals
/// (1000-nit display, system gamma 1.2, black level 0), in place.
pub fn encode_hlg(rgba: &mut [f32], reference_white: f32) {
    let scale = f64::from(reference_white) / HLG_PEAK;
    rgba.chunks_exact_mut(4).for_each(|pixel| {
        let display: [f64; 3] =
            std::array::from_fn(|i| (f64::from(pixel[i]) * scale).max(0.0));
        let luma: f64 = display.iter().zip(LUMA).map(|(e, k)| e * k).sum();
        // inverse OOTF: E_s = E_d * Y_d^((1 - gamma) / gamma)
        let gain = match luma > 0.0 {
            true => luma.powf((1.0 - HLG_GAMMA) / HLG_GAMMA),
            false => 0.0,
        };
        pixel[..3].iter_mut().zip(display).for_each(|(channel, e)| {
            *channel = hlg_from_scene(e * gain) as f32;
        })
    });
}
