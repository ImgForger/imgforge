use crate::processing::transform;
use libvips::VipsImage;

use super::tests_support::*;

type ExifOrientationTestCase = (u32, (u32, u32), Vec<[u8; 4]>);

#[test]
fn test_apply_exif_orientation_all_branches() {
    init_vips();
    let a = [255, 0, 0, 255];
    let b = [0, 255, 0, 255];
    let c = [0, 0, 255, 255];
    let d = [255, 255, 0, 255];
    let e = [255, 0, 255, 255];
    let f = [0, 255, 255, 255];

    let cases: [ExifOrientationTestCase; 8] = [
        (1, (3, 2), vec![a, b, c, d, e, f]),
        (2, (3, 2), vec![c, b, a, f, e, d]),
        (3, (3, 2), vec![f, e, d, c, b, a]),
        (4, (3, 2), vec![d, e, f, a, b, c]),
        (5, (2, 3), vec![a, d, b, e, c, f]),
        (6, (2, 3), vec![d, a, e, b, f, c]),
        (7, (2, 3), vec![f, c, e, b, d, a]),
        (8, (2, 3), vec![c, f, b, e, a, d]),
    ];

    for (orientation, (expected_w, expected_h), expected_pixels) in cases {
        let source_bytes = create_orientation_test_image();
        let img = VipsImage::new_from_buffer(&source_bytes, "").unwrap();
        let oriented = transform::apply_exif_orientation(img, orientation).unwrap();
        assert_eq!(oriented.get_width(), expected_w as i32);
        assert_eq!(oriented.get_height(), expected_h as i32);

        let decoded = decode_rgba(&oriented);
        assert_eq!(collect_rgba_pixels(&decoded), expected_pixels);
    }
}

#[test]
fn test_apply_exif_rotation_without_orientation_keeps_image_unchanged() {
    init_vips();
    let image_bytes = create_orientation_test_image();
    let img = VipsImage::new_from_buffer(&image_bytes, "").unwrap();
    let rotated = transform::apply_exif_rotation(&image_bytes, img).unwrap();
    assert_eq!(rotated.get_width(), 3);
    assert_eq!(rotated.get_height(), 2);

    let expected = vec![
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
        [255, 0, 255, 255],
        [0, 255, 255, 255],
    ];
    let decoded = decode_rgba(&rotated);
    assert_eq!(collect_rgba_pixels(&decoded), expected);
}
