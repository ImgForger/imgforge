#[cfg(test)]
mod test_processing {
    use crate::constants::ENV_WATERMARK_PATH;
    use crate::processing::options::{parse_all_options, Crop, ProcessingOption, Resize, Watermark};
    use crate::processing::transform;
    use crate::processing::utils;
    use crate::processing::watermark;
    use bytes::Bytes;
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
        let resized_img = transform::apply_resize(img, &resize, &None, &None).unwrap();
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
        let resized_img = transform::apply_resize(img, &resize, &Some("center".to_string()), &None).unwrap();
        assert_eq!(resized_img.get_width(), 200);
        assert_eq!(resized_img.get_height(), 200);
    }

    #[test]
    fn test_apply_resize_fill_width_only() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
        let resize = Resize {
            resizing_type: "fill".to_string(),
            width: 200,
            height: 0,
        };
        let resized_img = transform::apply_resize(img, &resize, &Some("center".to_string()), &None).unwrap();
        assert_eq!(resized_img.get_width(), 200);
        assert_eq!(resized_img.get_height(), 150);
    }

    #[test]
    fn test_apply_resize_fill_height_only() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
        let resize = Resize {
            resizing_type: "fill".to_string(),
            width: 0,
            height: 150,
        };
        let resized_img = transform::apply_resize(img, &resize, &Some("center".to_string()), &None).unwrap();
        assert_eq!(resized_img.get_width(), 200);
        assert_eq!(resized_img.get_height(), 150);
    }

    #[test]
    fn test_apply_resize_force_width_only() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
        let resize = Resize {
            resizing_type: "force".to_string(),
            width: 200,
            height: 0,
        };
        let resized_img = transform::apply_resize(img, &resize, &None, &None).unwrap();
        assert_eq!(resized_img.get_width(), 200);
        assert_eq!(resized_img.get_height(), 300);
    }

    #[test]
    fn test_apply_resize_force_height_only() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
        let resize = Resize {
            resizing_type: "force".to_string(),
            width: 0,
            height: 150,
        };
        let resized_img = transform::apply_resize(img, &resize, &None, &None).unwrap();
        assert_eq!(resized_img.get_width(), 400);
        assert_eq!(resized_img.get_height(), 150);
    }

    #[test]
    fn test_apply_resize_force_zero_dimensions_error() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
        let resize = Resize {
            resizing_type: "force".to_string(),
            width: 0,
            height: 0,
        };
        let result = transform::apply_resize(img, &resize, &None, &None);
        assert!(result.is_err());
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

    #[test]
    fn test_apply_background_color_no_alpha() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image_jpeg(100, 100), "").unwrap();
        let bands_before = img.get_bands();
        let bg_applied_img = transform::apply_background_color(img, [255, 0, 0, 255]).unwrap();
        assert_eq!(bg_applied_img.get_bands(), bands_before);
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

    fn create_test_image_jpeg(width: u32, height: u32) -> Vec<u8> {
        let mut img: ImageBuffer<image::Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
        for (_x, _y, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Rgb([255, 0, 0]);
        }
        let mut bytes: Vec<u8> = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
            .unwrap();
        bytes
    }

    fn cached_watermark_from_bytes(bytes: Vec<u8>) -> watermark::CachedWatermark {
        watermark::CachedWatermark::from_bytes(Bytes::from(bytes))
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
            name: "rotate".to_string(),
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
        let min_dims_img = transform::apply_min_dimensions(img, Some(200), Some(150), &None).unwrap();
        assert_eq!(min_dims_img.get_width(), 200);
        assert_eq!(min_dims_img.get_height(), 200); // Scales by max(2, 1.5) = 2
    }

    #[test]
    fn test_apply_zoom() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let zoomed_img = transform::apply_zoom(img, 2.0, &None).unwrap();
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
        let pixelated_img = transform::apply_pixelate(img, 10, &None).unwrap();
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
        let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
        let watermark_path = "/tmp/test_watermark.png";
        std::fs::write(watermark_path, watermark.bytes.clone()).unwrap();
        std::env::set_var(ENV_WATERMARK_PATH, watermark_path);

        let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
        let watermark_opts = Watermark {
            opacity: 0.5,
            position: "center".to_string(),
        };
        let watermarked_img = watermark::apply_watermark(img, &watermark, &watermark_opts, &None).unwrap();

        assert_eq!(watermarked_img.get_width(), 200);
        assert_eq!(watermarked_img.get_height(), 200);

        // Cleanup
        std::fs::remove_file(watermark_path).unwrap();
        std::env::remove_var("WATERMARK_PATH");
    }

    // Error handling tests
    #[test]
    fn test_parse_resize_type_only() {
        let options = vec![ProcessingOption {
            name: "resize".to_string(),
            args: vec!["fill".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        let resize = parsed.resize.unwrap();
        assert_eq!(resize.resizing_type, "fill");
        assert_eq!(resize.width, 0);
        assert_eq!(resize.height, 0);
    }

    #[test]
    fn test_parse_resize_meta_enlarge_extend() {
        let options = vec![ProcessingOption {
            name: "resize".to_string(),
            args: vec![
                "fit".to_string(),
                "640".to_string(),
                "480".to_string(),
                "true".to_string(),
                "true".to_string(),
            ],
        }];
        let parsed = parse_all_options(options).unwrap();
        let resize = parsed.resize.unwrap();
        assert_eq!(resize.resizing_type, "fit");
        assert_eq!(resize.width, 640);
        assert_eq!(resize.height, 480);
        assert!(parsed.enlarge);
        assert!(parsed.extend);
    }

    #[test]
    fn test_parse_resize_meta_enlarge_only() {
        let options = vec![ProcessingOption {
            name: "resize".to_string(),
            args: vec!["".to_string(), "".to_string(), "".to_string(), "true".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert!(parsed.resize.is_none());
        assert!(parsed.enlarge);
        assert!(!parsed.extend);
    }

    #[test]
    fn test_parse_resize_invalid_width() {
        let options = vec![ProcessingOption {
            name: "resize".to_string(),
            args: vec!["fill".to_string(), "abc".to_string(), "200".to_string()],
        }];
        assert!(parse_all_options(options).is_err());
    }

    #[test]
    fn test_parse_background_invalid_hex() {
        let options = vec![ProcessingOption {
            name: "background".to_string(),
            args: vec!["gggggg".to_string()],
        }];
        assert!(parse_all_options(options).is_err());
    }

    #[test]
    fn test_parse_background_short_hex() {
        let options = vec![ProcessingOption {
            name: "background".to_string(),
            args: vec!["fff".to_string()],
        }];
        assert!(parse_all_options(options).is_err());
    }

    #[test]
    fn test_parse_quality_clamping() {
        let options = vec![ProcessingOption {
            name: "quality".to_string(),
            args: vec!["150".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.quality, Some(100));
    }

    #[test]
    fn test_parse_quality_zero() {
        let options = vec![ProcessingOption {
            name: "quality".to_string(),
            args: vec!["0".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.quality, Some(1));
    }

    #[test]
    fn test_parse_dpr_out_of_range() {
        let options = vec![ProcessingOption {
            name: "dpr".to_string(),
            args: vec!["10.0".to_string()],
        }];
        assert!(parse_all_options(options).is_err());
    }

    #[test]
    fn test_parse_dpr_below_minimum() {
        let options = vec![ProcessingOption {
            name: "dpr".to_string(),
            args: vec!["0.5".to_string()],
        }];
        assert!(parse_all_options(options).is_err());
    }

    #[test]
    fn test_parse_crop_invalid_args() {
        let options = vec![ProcessingOption {
            name: "crop".to_string(),
            args: vec!["10".to_string(), "20".to_string()],
        }];
        assert!(parse_all_options(options).is_err());
    }

    #[test]
    fn test_parse_padding_invalid_count() {
        let options = vec![ProcessingOption {
            name: "padding".to_string(),
            args: vec!["10".to_string(), "20".to_string(), "30".to_string()],
        }];
        assert!(parse_all_options(options).is_err());
    }

    // Edge case tests
    #[test]
    fn test_resize_very_small_image() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(10, 10), "").unwrap();
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: 5,
            height: 5,
        };
        let resized_img = transform::apply_resize(img, &resize, &None, &None).unwrap();
        assert_eq!(resized_img.get_width(), 5);
        assert_eq!(resized_img.get_height(), 5);
    }

    #[test]
    fn test_resize_extreme_scale_up() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(10, 10), "").unwrap();
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: 1000,
            height: 1000,
        };
        let resized_img = transform::apply_resize(img, &resize, &None, &None).unwrap();
        assert_eq!(resized_img.get_width(), 1000);
        assert_eq!(resized_img.get_height(), 1000);
    }

    #[test]
    fn test_resize_extreme_aspect_ratio() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let resize = Resize {
            resizing_type: "fill".to_string(),
            width: 1000,
            height: 10,
        };
        let resized_img = transform::apply_resize(img, &resize, &None, &None).unwrap();
        assert_eq!(resized_img.get_width(), 1000);
        assert_eq!(resized_img.get_height(), 10);
    }

    #[test]
    fn test_crop_at_edge() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let crop = Crop {
            x: 0,
            y: 0,
            width: 50,
            height: 50,
        };
        let cropped_img = transform::crop_image(img, crop).unwrap();
        assert_eq!(cropped_img.get_width(), 50);
        assert_eq!(cropped_img.get_height(), 50);
    }

    #[test]
    fn test_crop_bottom_right_corner() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let crop = Crop {
            x: 50,
            y: 50,
            width: 50,
            height: 50,
        };
        let cropped_img = transform::crop_image(img, crop).unwrap();
        assert_eq!(cropped_img.get_width(), 50);
        assert_eq!(cropped_img.get_height(), 50);
    }

    #[test]
    fn test_rotation_on_non_square() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(150, 100), "").unwrap();
        let rotated_img = transform::apply_rotation(img, 90).unwrap();
        assert_eq!(rotated_img.get_width(), 100);
        assert_eq!(rotated_img.get_height(), 150);
    }

    #[test]
    fn test_rotation_180_degrees() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 200), "").unwrap();
        let rotated_img = transform::apply_rotation(img, 180).unwrap();
        assert_eq!(rotated_img.get_width(), 100);
        assert_eq!(rotated_img.get_height(), 200);
    }

    #[test]
    fn test_rotation_270_degrees() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 200), "").unwrap();
        let rotated_img = transform::apply_rotation(img, 270).unwrap();
        assert_eq!(rotated_img.get_width(), 200);
        assert_eq!(rotated_img.get_height(), 100);
    }

    #[test]
    fn test_rotation_unsupported_angle() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let rotated_img = transform::apply_rotation(img, 45).unwrap();
        // Should return original image unchanged
        assert_eq!(rotated_img.get_width(), 100);
        assert_eq!(rotated_img.get_height(), 100);
    }

    #[test]
    fn test_pixelate_zero() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let original_width = img.get_width();
        let pixelated_img = transform::apply_pixelate(img, 0, &None).unwrap();
        assert_eq!(pixelated_img.get_width(), original_width);
    }

    #[test]
    fn test_pixelate_small_amount() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let pixelated_img = transform::apply_pixelate(img, 1, &None).unwrap();
        assert_eq!(pixelated_img.get_width(), 100);
    }

    #[test]
    fn test_pixelate_large_amount() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
        let pixelated_img = transform::apply_pixelate(img, 50, &None).unwrap();
        assert_eq!(pixelated_img.get_width(), 200);
        assert_eq!(pixelated_img.get_height(), 200);
    }

    // Multiple transformations tests
    #[test]
    fn test_crop_then_resize() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(400, 400), "").unwrap();
        let crop = Crop {
            x: 50,
            y: 50,
            width: 200,
            height: 200,
        };
        let cropped = transform::crop_image(img, crop).unwrap();
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: 100,
            height: 100,
        };
        let final_img = transform::apply_resize(cropped, &resize, &None, &None).unwrap();
        assert_eq!(final_img.get_width(), 100);
        assert_eq!(final_img.get_height(), 100);
    }

    #[test]
    fn test_resize_then_blur() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: 100,
            height: 100,
        };
        let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
        let blurred = transform::apply_blur(resized, 3.0).unwrap();
        assert_eq!(blurred.get_width(), 100);
        assert_eq!(blurred.get_height(), 100);
    }

    #[test]
    fn test_resize_then_sharpen() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: 300,
            height: 300,
        };
        let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
        let sharpened = transform::apply_sharpen(resized, 1.0).unwrap();
        assert_eq!(sharpened.get_width(), 300);
        assert_eq!(sharpened.get_height(), 300);
    }

    #[test]
    fn test_rotation_then_resize() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 200), "").unwrap();
        let rotated = transform::apply_rotation(img, 90).unwrap();
        // After rotation: 200x100
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: 100,
            height: 100,
        };
        let resized = transform::apply_resize(rotated, &resize, &None, &None).unwrap();
        // Fit scales based on width: 200x100 -> 100x50
        assert_eq!(resized.get_width(), 100);
        assert_eq!(resized.get_height(), 50);
    }

    #[test]
    fn test_padding_with_background_color() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let padded = transform::apply_padding(img, 20, 30, 40, 50, &Some([255, 255, 255, 255])).unwrap();
        assert_eq!(padded.get_width(), 180);
        assert_eq!(padded.get_height(), 160);
    }

    #[test]
    fn test_extend_with_different_gravities() {
        let _ = &*APP;
        for gravity in &["north", "south", "east", "west", "center"] {
            let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
            let extended =
                transform::extend_image(img, 200, 200, &Some(gravity.to_string()), &Some([0, 0, 0, 0])).unwrap();
            assert_eq!(extended.get_width(), 200);
            assert_eq!(extended.get_height(), 200);
        }
    }

    #[test]
    fn test_resize_fill_with_different_gravities() {
        let _ = &*APP;
        for gravity in &["north", "south", "east", "west", "center"] {
            let img = VipsImage::new_from_buffer(&create_test_image(200, 100), "").unwrap();
            let resize = Resize {
                resizing_type: "fill".to_string(),
                width: 100,
                height: 100,
            };
            let resized = transform::apply_resize(img, &resize, &Some(gravity.to_string()), &None).unwrap();
            assert_eq!(resized.get_width(), 100);
            assert_eq!(resized.get_height(), 100);
        }
    }

    #[test]
    fn test_resize_fill_with_lanczos2_kernel() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(800, 600), "").unwrap();
        let resize = Resize {
            resizing_type: "fill".to_string(),
            width: 300,
            height: 400,
        };
        let resized =
            transform::apply_resize(img, &resize, &Some("center".to_string()), &Some("lanczos2".to_string())).unwrap();
        assert_eq!(resized.get_width(), 300);
        assert_eq!(resized.get_height(), 400);
    }

    #[test]
    fn test_resize_fit_with_nearest_kernel() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(800, 600), "").unwrap();
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: 300,
            height: 400,
        };
        let resized = transform::apply_resize(img, &resize, &None, &Some("nearest".to_string())).unwrap();
        assert_eq!(resized.get_width(), 300);
        assert_eq!(resized.get_height(), 225);
    }

    #[test]
    fn test_watermark_all_positions() {
        let _ = &*APP;
        let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
        let positions = vec![
            "north",
            "south",
            "east",
            "west",
            "center",
            "north_west",
            "north_east",
            "south_west",
            "south_east",
        ];

        for position in positions {
            let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
            let watermark_opts = Watermark {
                opacity: 0.5,
                position: position.to_string(),
            };
            let watermarked = watermark::apply_watermark(img, &watermark, &watermark_opts, &None).unwrap();
            assert_eq!(watermarked.get_width(), 200);
            assert_eq!(watermarked.get_height(), 200);
        }
    }

    #[test]
    fn test_watermark_full_opacity() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
        let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
        let watermark_opts = Watermark {
            opacity: 1.0,
            position: "center".to_string(),
        };
        let watermarked = watermark::apply_watermark(img, &watermark, &watermark_opts, &None).unwrap();
        assert_eq!(watermarked.get_width(), 200);
        assert_eq!(watermarked.get_height(), 200);
    }

    #[test]
    fn test_watermark_zero_opacity() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
        let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
        let watermark_opts = Watermark {
            opacity: 0.0,
            position: "center".to_string(),
        };
        let watermarked = watermark::apply_watermark(img, &watermark, &watermark_opts, &None).unwrap();
        assert_eq!(watermarked.get_width(), 200);
        assert_eq!(watermarked.get_height(), 200);
    }

    // Resize type tests
    #[test]
    fn test_resize_fit_width_only() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(200, 100), "").unwrap();
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: 100,
            height: 0,
        };
        let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
        assert_eq!(resized.get_width(), 100);
        assert_eq!(resized.get_height(), 50);
    }

    #[test]
    fn test_resize_fit_height_only() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(200, 100), "").unwrap();
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: 0,
            height: 50,
        };
        let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
        assert_eq!(resized.get_width(), 100);
        assert_eq!(resized.get_height(), 50);
    }

    #[test]
    fn test_resize_auto_portrait_to_portrait() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 200), "").unwrap();
        let resize = Resize {
            resizing_type: "auto".to_string(),
            width: 50,
            height: 100,
        };
        let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
        assert_eq!(resized.get_width(), 50);
        assert_eq!(resized.get_height(), 100);
    }

    #[test]
    fn test_resize_auto_landscape_to_landscape() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(200, 100), "").unwrap();
        let resize = Resize {
            resizing_type: "auto".to_string(),
            width: 100,
            height: 50,
        };
        let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
        assert_eq!(resized.get_width(), 100);
        assert_eq!(resized.get_height(), 50);
    }

    #[test]
    fn test_resize_auto_portrait_to_landscape() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 200), "").unwrap();
        let resize = Resize {
            resizing_type: "auto".to_string(),
            width: 150,
            height: 100,
        };
        let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
        // Uses fit mode when orientations differ, fitting within 150x100 while keeping aspect.
        assert_eq!(resized.get_width(), 50);
        assert_eq!(resized.get_height(), 100);
    }

    // Utils tests
    #[test]
    fn test_parse_hex_color_with_hash() {
        let color = utils::parse_hex_color("#ff0000").unwrap();
        assert_eq!(color, [255, 0, 0, 255]);
    }

    #[test]
    fn test_parse_hex_color_without_hash() {
        let color = utils::parse_hex_color("00ff00").unwrap();
        assert_eq!(color, [0, 255, 0, 255]);
    }

    #[test]
    fn test_parse_hex_color_invalid() {
        assert!(utils::parse_hex_color("gg0000").is_err());
    }

    #[test]
    fn test_parse_hex_color_wrong_length() {
        assert!(utils::parse_hex_color("fff").is_err());
        assert!(utils::parse_hex_color("fffffff").is_err());
    }

    #[test]
    fn test_parse_boolean_true_variants() {
        assert!(utils::parse_boolean("1"));
        assert!(utils::parse_boolean("true"));
    }

    #[test]
    fn test_parse_boolean_false_variants() {
        assert!(!utils::parse_boolean("0"));
        assert!(!utils::parse_boolean("false"));
        assert!(!utils::parse_boolean(""));
        assert!(!utils::parse_boolean("yes"));
    }

    #[test]
    fn test_is_portrait() {
        assert!(utils::is_portrait(100, 200));
        assert!(!utils::is_portrait(200, 100));
        assert!(!utils::is_portrait(100, 100));
    }

    // Min dimensions tests
    #[test]
    fn test_apply_min_width_only() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let result = transform::apply_min_dimensions(img, Some(200), None, &None).unwrap();
        assert_eq!(result.get_width(), 200);
        assert_eq!(result.get_height(), 200);
    }

    #[test]
    fn test_apply_min_height_only() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let result = transform::apply_min_dimensions(img, None, Some(150), &None).unwrap();
        assert_eq!(result.get_width(), 150);
        assert_eq!(result.get_height(), 150);
    }

    #[test]
    fn test_apply_min_dimensions_already_larger() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
        let result = transform::apply_min_dimensions(img, Some(100), Some(100), &None).unwrap();
        assert_eq!(result.get_width(), 200);
        assert_eq!(result.get_height(), 200);
    }

    #[test]
    fn test_apply_zoom_scale_down() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
        let zoomed = transform::apply_zoom(img, 0.5, &None).unwrap();
        assert_eq!(zoomed.get_width(), 100);
        assert_eq!(zoomed.get_height(), 100);
    }

    #[test]
    fn test_apply_zoom_scale_up() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let zoomed = transform::apply_zoom(img, 3.0, &None).unwrap();
        assert_eq!(zoomed.get_width(), 300);
        assert_eq!(zoomed.get_height(), 300);
    }

    // Blur edge cases
    #[test]
    fn test_apply_blur_minimal() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let blurred = transform::apply_blur(img, 0.1).unwrap();
        assert_eq!(blurred.get_width(), 100);
        assert_eq!(blurred.get_height(), 100);
    }

    #[test]
    fn test_apply_blur_extreme() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let blurred = transform::apply_blur(img, 50.0).unwrap();
        assert_eq!(blurred.get_width(), 100);
        assert_eq!(blurred.get_height(), 100);
    }

    // Sharpen edge cases
    #[test]
    fn test_apply_sharpen_minimal() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let sharpened = transform::apply_sharpen(img, 0.1).unwrap();
        assert_eq!(sharpened.get_width(), 100);
        assert_eq!(sharpened.get_height(), 100);
    }

    #[test]
    fn test_apply_sharpen_extreme() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let sharpened = transform::apply_sharpen(img, 10.0).unwrap();
        assert_eq!(sharpened.get_width(), 100);
        assert_eq!(sharpened.get_height(), 100);
    }

    #[test]
    fn test_apply_sharpen_clamps_sigma() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(50, 50), "").unwrap();
        let sharpened = transform::apply_sharpen(img, 100.0).unwrap();
        assert_eq!(sharpened.get_width(), 50);
        assert_eq!(sharpened.get_height(), 50);
    }

    // Complex multi-operation scenarios
    #[test]
    fn test_complex_pipeline_crop_resize_blur_rotate() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(400, 400), "").unwrap();

        // Crop
        let crop = Crop {
            x: 50,
            y: 50,
            width: 300,
            height: 300,
        };
        let img = transform::crop_image(img, crop).unwrap();
        assert_eq!(img.get_width(), 300);

        // Resize
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: 200,
            height: 200,
        };
        let img = transform::apply_resize(img, &resize, &None, &None).unwrap();
        assert_eq!(img.get_width(), 200);

        // Blur
        let img = transform::apply_blur(img, 2.0).unwrap();

        // Rotate
        let img = transform::apply_rotation(img, 90).unwrap();
        assert_eq!(img.get_width(), 200);
        assert_eq!(img.get_height(), 200);
    }

    #[test]
    fn test_complex_pipeline_resize_padding_watermark() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();

        // Resize
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: 150,
            height: 150,
        };
        let img = transform::apply_resize(img, &resize, &None, &None).unwrap();

        // Padding
        let img = transform::apply_padding(img, 10, 10, 10, 10, &Some([255, 255, 255, 255])).unwrap();
        assert_eq!(img.get_width(), 170);
        assert_eq!(img.get_height(), 170);

        // Watermark
        let watermark = cached_watermark_from_bytes(create_test_image(30, 30));
        let watermark_opts = Watermark {
            opacity: 0.7,
            position: "south_east".to_string(),
        };
        let img = watermark::apply_watermark(img, &watermark, &watermark_opts, &None).unwrap();
        assert_eq!(img.get_width(), 170);
    }

    // Shorthand option tests
    #[test]
    fn test_parse_resize_short() {
        let options = vec![ProcessingOption {
            name: "rs".to_string(),
            args: vec!["fill".to_string(), "300".to_string(), "200".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert!(parsed.resize.is_some());
    }

    #[test]
    fn test_parse_quality_short() {
        let options = vec![ProcessingOption {
            name: "q".to_string(),
            args: vec!["80".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.quality, Some(80));
    }

    #[test]
    fn test_parse_blur_short() {
        let options = vec![ProcessingOption {
            name: "bl".to_string(),
            args: vec!["3.5".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.blur, Some(3.5));
    }

    #[test]
    fn test_parse_watermark_short() {
        let options = vec![ProcessingOption {
            name: "wm".to_string(),
            args: vec!["0.8".to_string(), "south".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert!(parsed.watermark.is_some());
    }

    // Combined options test
    #[test]
    fn test_parse_multiple_options() {
        let options = vec![
            ProcessingOption {
                name: "resize".to_string(),
                args: vec!["fill".to_string(), "300".to_string(), "200".to_string()],
            },
            ProcessingOption {
                name: "quality".to_string(),
                args: vec!["90".to_string()],
            },
            ProcessingOption {
                name: "blur".to_string(),
                args: vec!["2.0".to_string()],
            },
            ProcessingOption {
                name: "format".to_string(),
                args: vec!["webp".to_string()],
            },
        ];
        let parsed = parse_all_options(options).unwrap();
        assert!(parsed.resize.is_some());
        assert_eq!(parsed.quality, Some(90));
        assert_eq!(parsed.blur, Some(2.0));
        assert_eq!(parsed.format, Some("webp".to_string()));
    }

    // Background color tests
    #[test]
    fn test_apply_background_color_with_transparency() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let result = transform::apply_background_color(img, [255, 255, 255, 255]).unwrap();
        // Should flatten to 3 bands (RGB)
        assert_eq!(result.get_bands(), 3);
    }

    // Size option test
    #[test]
    fn test_parse_size_option() {
        let options = vec![ProcessingOption {
            name: "size".to_string(),
            args: vec!["640".to_string(), "480".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert!(parsed.resize.is_some());
        let resize = parsed.resize.unwrap();
        assert_eq!(resize.resizing_type, "fit");
        assert_eq!(resize.width, 640);
        assert_eq!(resize.height, 480);
    }

    #[test]
    fn test_parse_size_short() {
        let options = vec![ProcessingOption {
            name: "sz".to_string(),
            args: vec!["800".to_string(), "600".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert!(parsed.resize.is_some());
    }

    #[test]
    fn test_parse_size_meta_full() {
        let options = vec![ProcessingOption {
            name: "size".to_string(),
            args: vec![
                "320".to_string(),
                "240".to_string(),
                "true".to_string(),
                "true".to_string(),
            ],
        }];
        let parsed = parse_all_options(options).unwrap();
        let resize = parsed.resize.unwrap();
        assert_eq!(resize.resizing_type, "fit");
        assert_eq!(resize.width, 320);
        assert_eq!(resize.height, 240);
        assert!(parsed.enlarge);
        assert!(parsed.extend);
    }

    #[test]
    fn test_parse_size_meta_enlarge_only() {
        let options = vec![ProcessingOption {
            name: "size".to_string(),
            args: vec!["".to_string(), "".to_string(), "true".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert!(parsed.resize.is_none());
        assert!(parsed.enlarge);
        assert!(!parsed.extend);
    }

    #[test]
    fn test_parse_size_short_alias_s() {
        let options = vec![ProcessingOption {
            name: "s".to_string(),
            args: vec![
                "1024".to_string(),
                "".to_string(),
                "true".to_string(),
                "true".to_string(),
            ],
        }];
        let parsed = parse_all_options(options).unwrap();
        let resize = parsed.resize.unwrap();
        assert_eq!(resize.resizing_type, "fit");
        assert_eq!(resize.width, 1024);
        assert_eq!(resize.height, 0);
        assert!(parsed.extend);
        assert!(parsed.enlarge);
    }

    #[test]
    fn test_parse_width_default_zero() {
        let options = vec![ProcessingOption {
            name: "width".to_string(),
            args: vec![],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.width, Some(0));
        let resize = parsed.resize.unwrap();
        assert_eq!(resize.resizing_type, "fit");
        assert_eq!(resize.width, 0);
        assert_eq!(resize.height, 0);
    }

    #[test]
    fn test_parse_width_blank_argument() {
        let options = vec![ProcessingOption {
            name: "width".to_string(),
            args: vec!["".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.width, Some(0));
    }

    #[test]
    fn test_parse_width_short_blank_defaults() {
        let options = vec![ProcessingOption {
            name: "w".to_string(),
            args: vec![],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.width, Some(0));
    }

    #[test]
    fn test_parse_height_default_zero() {
        let options = vec![ProcessingOption {
            name: "height".to_string(),
            args: vec![],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.height, Some(0));
    }

    #[test]
    fn test_parse_height_blank_argument() {
        let options = vec![ProcessingOption {
            name: "h".to_string(),
            args: vec!["".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.height, Some(0));
    }

    #[test]
    fn test_parse_resizing_algorithm_full() {
        let options = vec![ProcessingOption {
            name: "resizing_algorithm".to_string(),
            args: vec!["cubic".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.resizing_algorithm, Some("cubic".to_string()));
    }

    #[test]
    fn test_parse_resizing_algorithm_short() {
        let options = vec![ProcessingOption {
            name: "ra".to_string(),
            args: vec!["linear".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.resizing_algorithm, Some("linear".to_string()));
    }

    #[test]
    fn test_parse_resizing_algorithm_case_insensitive() {
        let options = vec![ProcessingOption {
            name: "ra".to_string(),
            args: vec!["LANCZOS3".to_string()],
        }];
        let parsed = parse_all_options(options).unwrap();
        assert_eq!(parsed.resizing_algorithm, Some("lanczos3".to_string()));
    }

    #[test]
    fn test_parse_resizing_algorithm_invalid() {
        let options = vec![ProcessingOption {
            name: "resizing_algorithm".to_string(),
            args: vec!["invalid".to_string()],
        }];
        let result = parse_all_options(options);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid resizing algorithm"));
    }

    #[test]
    fn test_apply_resize_with_cubic_algorithm() {
        let _ = &*APP;
        let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: 200,
            height: 150,
        };

        // Test with cubic - should also work
        let resized_img2 = transform::apply_resize(img, &resize, &None, &Some("cubic".to_string())).unwrap();
        assert_eq!(resized_img2.get_width(), 200);
        assert_eq!(resized_img2.get_height(), 150);
    }
}
