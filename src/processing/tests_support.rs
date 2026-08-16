use crate::processing::save;
use crate::processing::watermark;
use bytes::Bytes;
use image::{ImageBuffer, Rgba, RgbaImage};
use libvips::{ops, VipsImage};

pub use crate::test_support::{clear_vips_error, init_vips, vips_error_buffer};

/// Decodes encoded bytes into a `VipsImage`, keeping the buffer alive as long
/// as the image needs it.
///
/// libvips decodes lazily and holds a pointer into the input, so the buffer has
/// to outlive every operation that eventually reads pixels. Passing a temporary
/// — `VipsImage::new_from_buffer(&create_test_image(..), "")` — hands it a
/// pointer that is freed at the end of the statement. That survives only while
/// the allocation happens not to be reused, so it fails unpredictably: adding a
/// stage to the pipeline was enough to turn it into
/// "VipsJpeg: Not a JPEG file: starts with 0x80 0xeb".
///
/// Taking ownership and leaking is bounded in a test binary and keeps call
/// sites to one line, which matters when there are eighty of them.
pub fn image_from(bytes: Vec<u8>) -> VipsImage {
    let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    VipsImage::new_from_buffer(leaked, "").expect("test image should decode")
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

/// Opaque white on the left, fully transparent black on the right.
///
/// The transparent half carries black RGB, which is invisible until a resize
/// kernel averages it into neighbouring pixels. Downscaling this without
/// premultiplying drags the edge toward black; done correctly, RGB stays white
/// and only alpha varies.
pub fn create_transparent_edge_image(width: u32, height: u32) -> Vec<u8> {
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for (x, _y, pixel) in img.enumerate_pixels_mut() {
        *pixel = if x < width / 2 {
            Rgba([255, 255, 255, 255])
        } else {
            Rgba([0, 0, 0, 0])
        };
    }
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

/// A `width` x `height` image of `border`, with a `inner_w` x `inner_h` block of
/// `subject` inset at (`x`, `y`). Used to check that trimming removes exactly
/// the border and nothing else.
pub fn create_bordered_image(
    size: (u32, u32),
    border: [u8; 4],
    inset: (u32, u32, u32, u32),
    subject: [u8; 4],
) -> Vec<u8> {
    let (width, height) = size;
    let (x, y, inner_w, inner_h) = inset;
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(width, height, Rgba(border));
    for py in y..(y + inner_h).min(height) {
        for px in x..(x + inner_w).min(width) {
            img.put_pixel(px, py, Rgba(subject));
        }
    }
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

/// An animated GIF of `frames` solid-colour frames, for exercising the
/// multi-frame path.
///
/// Each frame is a different colour so a test can tell them apart, and can
/// therefore catch a pipeline that silently reinterprets the frame boundaries
/// rather than merely losing frames.
pub fn create_animated_gif(width: u32, height: u32, frames: usize) -> Vec<u8> {
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame};
    use std::time::Duration;

    let palette = [
        Rgba([255, 0, 0, 255]),
        Rgba([0, 255, 0, 255]),
        Rgba([0, 0, 255, 255]),
        Rgba([255, 255, 0, 255]),
        Rgba([255, 0, 255, 255]),
        Rgba([0, 255, 255, 255]),
    ];

    let mut bytes: Vec<u8> = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        for index in 0..frames {
            let buffer: RgbaImage = ImageBuffer::from_pixel(width, height, palette[index % palette.len()]);
            encoder
                .encode_frame(Frame::from_parts(
                    buffer,
                    0,
                    0,
                    Delay::from_saturating_duration(Duration::from_millis(100)),
                ))
                .expect("gif frame encodes");
        }
    }
    bytes
}

/// How many frames an encoded image holds, as libvips reports them.
pub fn frame_count(bytes: &[u8]) -> i32 {
    let leaked: &'static [u8] = Box::leak(bytes.to_vec().into_boxed_slice());
    VipsImage::new_from_buffer(leaked, "n=-1")
        .expect("encoded image should decode")
        .get_n_pages()
}
