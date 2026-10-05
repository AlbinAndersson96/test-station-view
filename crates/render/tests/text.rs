use tsv_render::text::{MAX_LABEL_PX, label_texture_size};

#[test]
fn label_textures_have_four_pixels_per_millimetre() {
    assert_eq!(label_texture_size(10.0, 2.5), (40, 10));
}

#[test]
fn label_texture_size_is_clamped() {
    assert_eq!(label_texture_size(0.0, 10_000.0), (1, MAX_LABEL_PX));
}
