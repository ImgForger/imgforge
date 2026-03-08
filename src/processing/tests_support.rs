use crate::processing::save;
use crate::processing::watermark;
use bytes::Bytes;
use image::{ImageBuffer, Rgba, RgbaImage};
use lazy_static::lazy_static;
use libvips::{ops, VipsApp, VipsImage};

lazy_static! {
    static ref APP: VipsApp = {
        let app = VipsApp::new("Test", false).expect("Cannot initialize libvips");
        app.concurrency_set(1);
        app
    };
}

pub fn init_vips() {
    let _ = &*APP;
}

pub fn create_test_image(width: u32, height: u32) -> Vec<u8> {
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for (_x, _y, pixel) in img.enumerate_pixels_mut() {
        *pixel = Rgba([255, 0, 0, 255]);
    }
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

pub fn create_quadrant_test_image(width: u32, height: u32) -> Vec<u8> {
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = if x < width / 2 && y < height / 2 {
            Rgba([255, 0, 0, 255])
        } else if x >= width / 2 && y < height / 2 {
            Rgba([0, 255, 0, 255])
        } else if x < width / 2 && y >= height / 2 {
            Rgba([0, 0, 255, 255])
        } else {
            Rgba([255, 255, 0, 255])
        };
    }
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

pub fn create_orientation_test_image() -> Vec<u8> {
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(3, 2);
    img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
    img.put_pixel(1, 0, Rgba([0, 255, 0, 255]));
    img.put_pixel(2, 0, Rgba([0, 0, 255, 255]));
    img.put_pixel(0, 1, Rgba([255, 255, 0, 255]));
    img.put_pixel(1, 1, Rgba([255, 0, 255, 255]));
    img.put_pixel(2, 1, Rgba([0, 255, 255, 255]));

    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

pub fn create_test_image_jpeg(width: u32, height: u32) -> Vec<u8> {
    let mut img: ImageBuffer<image::Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for (_x, _y, pixel) in img.enumerate_pixels_mut() {
        *pixel = image::Rgb([255, 0, 0]);
    }
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
        .unwrap();
    bytes
}

pub fn decode_rgba(img: &VipsImage) -> RgbaImage {
    let img_copy = ops::copy(img).unwrap();
    let png_bytes = save::save_image(img_copy, "png", 90).unwrap();
    image::load_from_memory(&png_bytes).unwrap().to_rgba8()
}

pub fn rgba_pixel(decoded: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
    let pixel = decoded.get_pixel(x, y);
    [pixel[0], pixel[1], pixel[2], pixel[3]]
}

pub fn collect_rgba_pixels(decoded: &RgbaImage) -> Vec<[u8; 4]> {
    decoded
        .pixels()
        .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
        .collect()
}

pub fn cached_watermark_from_bytes(bytes: Vec<u8>) -> watermark::CachedWatermark {
    watermark::CachedWatermark::from_bytes(Bytes::from(bytes))
}
