use super::*;

/// PQ signal values for absolute luminance, from ST 2084 Table 4 / BT.2100
/// reference tables (rounded to 1e-4).
#[test]
fn pq_matches_reference_values() {
    [
        (0.0, 0.0),
        (100.0, 0.5081),
        (203.0, 0.5807),
        (1000.0, 0.7518),
        (10000.0, 1.0),
    ]
    .into_iter()
    .for_each(|(nits, signal)| {
        let got = pq_from_nits(nits);
        assert!(
            (got - signal).abs() < 1e-4,
            "{nits} nits: {got} != {signal}"
        );
    });
}

#[test]
fn pq_clamps_out_of_range() {
    assert_eq!(pq_from_nits(-5.0), pq_from_nits(0.0));
    assert_eq!(pq_from_nits(20_000.0), 1.0);
}

#[test]
fn hlg_oetf_is_continuous_at_the_knee() {
    let knee = 1.0 / 12.0;
    assert!((hlg_from_scene(knee) - 0.5).abs() < 1e-6);
    assert!((hlg_from_scene(1.0) - 1.0).abs() < 1e-6);
}

/// BT.2408: 203 nits graphics white is 75% HLG on a 1000-nit display.
#[test]
fn hlg_reference_white_is_75_percent() {
    let mut pixel = [1.0, 1.0, 1.0, 1.0];
    encode_hlg(&mut pixel, DEFAULT_REFERENCE_WHITE);
    pixel[..3].iter().for_each(|signal| {
        assert!((signal - 0.75).abs() < 1e-3, "{signal}");
    });
    assert_eq!(pixel[3], 1.0);
}

#[test]
fn pq_reference_white_scales_linear_input() {
    let mut pixel = [1.0, 2.0, 0.0, 0.5];
    encode_pq(&mut pixel, DEFAULT_REFERENCE_WHITE);
    assert!((f64::from(pixel[0]) - pq_from_nits(203.0)).abs() < 1e-6);
    assert!((f64::from(pixel[1]) - pq_from_nits(406.0)).abs() < 1e-6);
    assert_eq!(pixel[2], pq_from_nits(0.0) as f32);
    assert_eq!(pixel[3], 0.5);

    let mut pixel = [1.0, 1.0, 1.0, 1.0];
    encode_pq(&mut pixel, 100.0);
    assert!((f64::from(pixel[0]) - pq_from_nits(100.0)).abs() < 1e-6);
}

#[test]
fn negative_input_encodes_as_black() {
    let mut pixel = [-0.5, -0.1, -2.0, 1.0];
    encode_hlg(&mut pixel, DEFAULT_REFERENCE_WHITE);
    assert_eq!(&pixel[..3], &[0.0, 0.0, 0.0]);

    // ST 2084 maps 0 nits to c1^m2 (7.3e-7), which quantizes to code 0.
    let black = pq_from_nits(0.0) as f32;
    assert!(black < 1.0 / 65535.0 / 2.0);
    let mut pixel = [-0.5, -0.1, -2.0, 1.0];
    encode_pq(&mut pixel, DEFAULT_REFERENCE_WHITE);
    assert_eq!(&pixel[..3], &[black, black, black]);
}
