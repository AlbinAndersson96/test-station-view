use tsv_render::text::{MAX_LABEL_PX, label_texture_size, mip_chain};

#[test]
fn label_textures_have_four_pixels_per_millimetre() {
    assert_eq!(label_texture_size(10.0, 2.5), (40, 10));
}

#[test]
fn label_texture_size_is_clamped() {
    assert_eq!(label_texture_size(0.0, 10_000.0), (1, MAX_LABEL_PX));
}

#[test]
fn mip_chain_halves_down_to_one_pixel() {
    let base = vec![0u8; 4 * 2 * 4];
    let sizes: Vec<(u32, u32)> = mip_chain(&base, 4, 2)
        .iter()
        .map(|(w, h, _)| (*w, *h))
        .collect();
    assert_eq!(sizes, vec![(4, 2), (2, 1), (1, 1)]);
    let odd: Vec<(u32, u32)> = mip_chain(&[0u8; 5 * 3 * 4], 5, 3)
        .iter()
        .map(|(w, h, _)| (*w, *h))
        .collect();
    assert_eq!(odd, vec![(5, 3), (2, 1), (1, 1)]);
}

#[test]
fn mip_levels_average_their_texels() {
    let base = [
        [200u8, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 100, 0, 255],
        [0, 0, 0, 255],
    ]
    .concat();
    let chain = mip_chain(&base, 2, 2);
    assert_eq!(chain[1], (1, 1, vec![50, 25, 0, 255]));
}
