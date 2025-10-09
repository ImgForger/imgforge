#[cfg(test)]
mod test_processing {
    use crate::processing::options::{parse_all_options, Crop, ProcessingOption, Resize, Watermark};
    use crate::processing::transform;
    use image::{ImageBuffer, Rgba};
    use lazy_static::lazy_static;
    use libvips::{VipsApp, VipsImage};

    lazy_static! {
        static ref APP: VipsApp = {
            let app = VipsApp::new("Test", false).expect("Cannot initialize libvips");
            app.concurrency_set(1);
            app
        };
    }

    #[test]
    fn test_parse_all_options_empty() {
        let options = vec![];
        let parsed = parse_all_options(options).unwrap();
        assert!(parsed.resize.is_none());
        assert!(parsed.blur.is_none());
        assert!(parsed.crop.is_none());
    }

    #[test]
    fn test_parse_resize_option() {
        let options = vec![ProcessingOption {
            name: "resize".to_string(),
            args: vec!["fill".to_string(), "300".to_string(), "200".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        let resize = parsed.resize.unwrap();
        assert_eq!(resize.resizing_type, "fill");
        assert_eq!(resize.width, 300);
        assert_eq!(resize.height, 200);
    }

    #[test]
    fn test_apply_resize_fit() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: 200,
            height: 150,
        };
        let resized_img = transform::apply_resize(img, &resize, &None).unwrap();
        assert_eq!(resized_img.get_width(), 200);
        assert_eq!(resized_img.get_height(), 150);
    }

    #[test]
    fn test_apply_resize_fill() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
        let resize = Resize {
            resizing_type: "fill".to_string(),
            width: 200,
            height: 200,
        };
        let resized_img = transform::apply_resize(img, &resize, &Some("center".to_string())).unwrap();
        assert_eq!(resized_img.get_width(), 200);
        assert_eq!(resized_img.get_height(), 200);
    }

    #[test]
    fn test_crop_image() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
        let crop = Crop {
            x: 10,
            y: 20,
            width: 100,
            height: 150,
        };
        let cropped_img = transform::crop_image(img, crop).unwrap();
        assert_eq!(cropped_img.get_width(), 100);
        assert_eq!(cropped_img.get_height(), 150);
    }

    #[test]
    fn test_extend_image() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let extended_img =
            transform::extend_image(img, 200, 200, &Some("center".to_string()), &Some([0, 0, 0, 0])).unwrap();
        assert_eq!(extended_img.get_width(), 200);
        assert_eq!(extended_img.get_height(), 200);
    }

    #[test]
    fn test_apply_padding() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let padded_img = transform::apply_padding(img, 10, 20, 30, 40, &Some([0, 0, 0, 0])).unwrap();
        assert_eq!(padded_img.get_width(), 160);
        assert_eq!(padded_img.get_height(), 140);
    }

    #[test]
    fn test_apply_rotation() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 200), "").unwrap();
        let rotated_img = transform::apply_rotation(img, 90).unwrap();
        assert_eq!(rotated_img.get_width(), 200);
        assert_eq!(rotated_img.get_height(), 100);
    }

    #[test]
    fn test_apply_blur() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let blurred_img = transform::apply_blur(img, 5.0).unwrap();
        assert_eq!(blurred_img.get_width(), 100);
        assert_eq!(blurred_img.get_height(), 100);
    }

    #[test]
    fn test_apply_background_color() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let bg_applied_img = transform::apply_background_color(img, [255, 0, 0, 255]).unwrap();
        assert_eq!(bg_applied_img.get_bands(), 3);
    }

    fn create_test_image(width: u32, height: u32) -> Vec<u8> {
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
        for (_x, _y, pixel) in img.enumerate_pixels_mut() {
            *pixel = Rgba([255, 0, 0, 255]);
        }
        let mut bytes: Vec<u8> = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn test_parse_quality_option() {
        let options = vec![ProcessingOption {
            name: "quality".to_string(),
            args: vec!["90".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.quality, Some(90));
    }

    #[test]
    fn test_parse_blur_option() {
        let options = vec![ProcessingOption {
            name: "blur".to_string(),
            args: vec!["5".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.blur, Some(5.0));
    }

    #[test]
    fn test_parse_background_option() {
        let options = vec![ProcessingOption {
            name: "background".to_string(),
            args: vec!["ff0000".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.background, Some([255, 0, 0, 255]));
    }

    #[test]
    fn test_parse_padding_option() {
        let options = vec![ProcessingOption {
            name: "padding".to_string(),
            args: vec!["10".to_string(), "20".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.padding, Some((10, 20, 10, 20)));
    }

    #[test]
    fn test_parse_rotation_option() {
        let options = vec![ProcessingOption {
            name: "rotation".to_string(),
            args: vec!["90".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.rotation, Some(90));
    }

    #[test]
    fn test_parse_enlarge_option() {
        let options = vec![ProcessingOption {
            name: "enlarge".to_string(),
            args: vec!["true".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert!(parsed.enlarge);
    }

    #[test]
    fn test_parse_extend_option() {
        let options = vec![ProcessingOption {
            name: "extend".to_string(),
            args: vec!["1".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert!(parsed.extend);
    }

    #[test]
    fn test_parse_gravity_option() {
        let options = vec![ProcessingOption {
            name: "gravity".to_string(),
            args: vec!["north".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.gravity, Some("north".to_string()));
    }

    #[test]
    fn test_parse_crop_option() {
        let options = vec![ProcessingOption {
            name: "crop".to_string(),
            args: vec!["10".to_string(), "20".to_string(), "100".to_string(), "150".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        let crop = parsed.crop.unwrap();
        assert_eq!(crop.x, 10);
        assert_eq!(crop.y, 20);
        assert_eq!(crop.width, 100);
        assert_eq!(crop.height, 150);
    }

    #[test]
    fn test_parse_format_option() {
        let options = vec![ProcessingOption {
            name: "format".to_string(),
            args: vec!["webp".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.format, Some("webp".to_string()));
    }

    #[test]
    fn test_parse_dpr_option() {
        let options = vec![ProcessingOption {
            name: "dpr".to_string(),
            args: vec!["2.5".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.dpr, Some(2.5));
    }

    #[test]
    fn test_parse_auto_rotate_option() {
        let options = vec![ProcessingOption {
            name: "auto_rotate".to_string(),
            args: vec!["false".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert!(!parsed.auto_rotate);
    }

    #[test]
    fn test_parse_raw_option() {
        let options = vec![ProcessingOption {
            name: "raw".to_string(),
            args: vec![],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert!(parsed.raw);
    }

    #[test]
    fn test_parse_max_src_resolution_option() {
        let options = vec![ProcessingOption {
            name: "max_src_resolution".to_string(),
            args: vec!["10.5".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.max_src_resolution, Some(10.5));
    }

    #[test]
    fn test_parse_max_src_file_size_option() {
        let options = vec![ProcessingOption {
            name: "max_src_file_size".to_string(),
            args: vec!["1024".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.max_src_file_size, Some(1024));
    }

    #[test]
    fn test_parse_cache_buster_option() {
        let options = vec![ProcessingOption {
            name: "cache_buster".to_string(),
            args: vec!["12345".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.cache_buster, Some("12345".to_string()));
    }

    #[test]
    fn test_parse_min_width_option() {
        let options = vec![ProcessingOption {
            name: "min_width".to_string(),
            args: vec!["500".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.min_width, Some(500));
    }

    #[test]
    fn test_parse_min_height_option() {
        let options = vec![ProcessingOption {
            name: "min_height".to_string(),
            args: vec!["600".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.min_height, Some(600));
    }

    #[test]
    fn test_parse_zoom_option() {
        let options = vec![ProcessingOption {
            name: "zoom".to_string(),
            args: vec!["1.5".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.zoom, Some(1.5));
    }

    #[test]
    fn test_parse_sharpen_option() {
        let options = vec![ProcessingOption {
            name: "sharpen".to_string(),
            args: vec!["0.5".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.sharpen, Some(0.5));
    }

    #[test]
    fn test_parse_pixelate_option() {
        let options = vec![ProcessingOption {
            name: "pixelate".to_string(),
            args: vec!["10".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.pixelate, Some(10));
    }

    #[test]
    fn test_apply_min_dimensions() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let min_dims_img = transform::apply_min_dimensions(img, Some(200), Some(150)).unwrap();
        assert_eq!(min_dims_img.get_width(), 200);
        assert_eq!(min_dims_img.get_height(), 200); // Scales by max(2, 1.5) = 2
    }

    #[test]
    fn test_apply_zoom() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let zoomed_img = transform::apply_zoom(img, 2.0).unwrap();
        assert_eq!(zoomed_img.get_width(), 200);
        assert_eq!(zoomed_img.get_height(), 200);
    }

    #[test]
    fn test_apply_sharpen() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let sharpened_img = transform::apply_sharpen(img, 0.5).unwrap();
        assert_eq!(sharpened_img.get_width(), 100);
        assert_eq!(sharpened_img.get_height(), 100);
    }

    #[test]
    fn test_apply_pixelate() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let pixelated_img = transform::apply_pixelate(img, 10).unwrap();
        assert_eq!(pixelated_img.get_width(), 100);
        assert_eq!(pixelated_img.get_height(), 100);
    }

    #[test]
    fn test_parse_watermark_option() {
        let options = vec![ProcessingOption {
            name: "watermark".to_string(),
            args: vec!["0.5".to_string(), "center".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        let watermark = parsed.watermark.unwrap();
        assert_eq!(watermark.opacity, 0.5);
        assert_eq!(watermark.position, "center");
    }

    #[test]
    fn test_apply_watermark() {
        let _ = &*APP;
        // Create a dummy watermark image
        let watermark_bytes = create_test_image(50, 50);
        let watermark_path = "/tmp/test_watermark.png";
        std::fs::write(watermark_path, watermark_bytes).unwrap();
        std::env::set_var("WATERMARK_PATH", watermark_path);

        let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
        let watermark_opts = Watermark {
            opacity: 0.5,
            position: "center".to_string(),
        };
        let watermarked_img = transform::apply_watermark(img, &watermark_opts).unwrap();

        assert_eq!(watermarked_img.get_width(), 200);
        assert_eq!(watermarked_img.get_height(), 200);

        // Cleanup
        std::fs::remove_file(watermark_path).unwrap();
        std::env::remove_var("WATERMARK_PATH");
    }
}
