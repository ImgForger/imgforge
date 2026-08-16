//! Reading and re-attaching source metadata.
//!
//! Two options need more than the encoder's `keep` flags can express.
//! `enforce_thumbnail` needs the EXIF thumbnail pulled out of the source before
//! anything is decoded, and `keep_copyright` needs one field carried across a
//! strip that libvips can only perform wholesale — its `keep` flags are
//! `none|exif|xmp|iptc|icc|other|gainmap|all`, with no copyright granularity.

use exif::{In, Tag, Value};
use tracing::debug;

/// JPEG marker bytes.
const MARKER_PREFIX: u8 = 0xFF;
const MARKER_SOI: u8 = 0xD8;
const MARKER_APP1: u8 = 0xE1;
const MARKER_SOS: u8 = 0xDA;

/// The identifier that opens the Exif payload of an APP1 segment.
const EXIF_IDENTIFIER: &[u8] = b"Exif\0\0";

/// The eight bytes every PNG opens with.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// EXIF tag numbers, as they appear in an IFD entry.
const TAG_COPYRIGHT: u16 = 0x8298;
const TAG_ARTIST: u16 = 0x013B;

/// EXIF field type for a NUL-terminated ASCII string.
const TYPE_ASCII: u16 = 2;

/// A copyright statement recovered from a source image.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Copyright {
    pub copyright: Option<String>,
    pub artist: Option<String>,
}

impl Copyright {
    pub fn is_empty(&self) -> bool {
        self.copyright.is_none() && self.artist.is_none()
    }
}

/// Locates the Exif TIFF block inside a JPEG's APP1 segment.
///
/// Returned as a slice of the input so nothing is copied for the common case of
/// a source that has no copyright to preserve.
fn jpeg_exif_block(image_bytes: &[u8]) -> Option<&[u8]> {
    if image_bytes.len() < 4 || image_bytes[0] != MARKER_PREFIX || image_bytes[1] != MARKER_SOI {
        return None;
    }

    let mut offset = 2;
    while offset + 4 <= image_bytes.len() {
        if image_bytes[offset] != MARKER_PREFIX {
            return None;
        }
        let marker = image_bytes[offset + 1];
        // Scan data starts here; every segment worth reading is behind us.
        if marker == MARKER_SOS {
            return None;
        }

        let length = usize::from(u16::from_be_bytes([image_bytes[offset + 2], image_bytes[offset + 3]]));
        if length < 2 {
            return None;
        }
        let payload_start = offset + 4;
        let payload_end = payload_start.checked_add(length - 2)?;
        if payload_end > image_bytes.len() {
            return None;
        }

        if marker == MARKER_APP1 && image_bytes[payload_start..payload_end].starts_with(EXIF_IDENTIFIER) {
            return Some(&image_bytes[payload_start + EXIF_IDENTIFIER.len()..payload_end]);
        }

        offset = payload_end;
    }

    None
}

fn ascii_field(exif: &exif::Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    match &field.value {
        Value::Ascii(values) => {
            let text: String = values
                .iter()
                .flat_map(|bytes| String::from_utf8_lossy(bytes).into_owned().chars().collect::<Vec<_>>())
                .collect();
            let trimmed = text.trim_matches(char::from(0)).trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        _ => None,
    }
}

/// Reads the copyright statement a source carries, if any.
pub fn read_copyright(image_bytes: &[u8]) -> Copyright {
    let Ok(exif) = exif::Reader::new().read_from_container(&mut std::io::Cursor::new(image_bytes)) else {
        return Copyright::default();
    };

    Copyright {
        copyright: ascii_field(&exif, Tag::Copyright),
        artist: ascii_field(&exif, Tag::Artist),
    }
}

/// Builds a minimal little-endian TIFF/Exif block carrying only the fields in
/// `copyright`.
///
/// This is the payload every container wants: JPEG wraps it behind the `Exif\0\0`
/// identifier in an APP1 segment, PNG stores it bare in an `eXIf` chunk, and
/// WebP stores it bare in an `EXIF` chunk.
fn build_exif_tiff(copyright: &Copyright) -> Option<Vec<u8>> {
    let mut entries: Vec<(u16, Vec<u8>)> = Vec::new();
    if let Some(value) = copyright.copyright.as_deref() {
        entries.push((TAG_COPYRIGHT, nul_terminated(value)));
    }
    if let Some(value) = copyright.artist.as_deref() {
        entries.push((TAG_ARTIST, nul_terminated(value)));
    }
    if entries.is_empty() {
        return None;
    }

    // IFD0 sits at offset 8, immediately after the TIFF header. Values longer
    // than the four bytes an entry can hold inline are appended after the
    // directory and referenced by offset.
    let entry_count = entries.len();
    let directory_end = 8 + 2 + entry_count * 12 + 4;
    let mut values: Vec<u8> = Vec::new();
    let mut directory: Vec<u8> = Vec::new();

    directory.extend_from_slice(&(entry_count as u16).to_le_bytes());
    for (tag, value) in &entries {
        directory.extend_from_slice(&tag.to_le_bytes());
        directory.extend_from_slice(&TYPE_ASCII.to_le_bytes());
        directory.extend_from_slice(&(value.len() as u32).to_le_bytes());

        if value.len() <= 4 {
            let mut inline = [0u8; 4];
            inline[..value.len()].copy_from_slice(value);
            directory.extend_from_slice(&inline);
        } else {
            let offset = u32::try_from(directory_end + values.len()).ok()?;
            directory.extend_from_slice(&offset.to_le_bytes());
            values.extend_from_slice(value);
            // Keep every value on an even boundary, as the TIFF spec requires.
            if values.len() % 2 == 1 {
                values.push(0);
            }
        }
    }
    // No IFD1: the thumbnail, if there was one, went with the strip.
    directory.extend_from_slice(&0u32.to_le_bytes());

    let mut tiff = Vec::with_capacity(8 + directory.len() + values.len());
    tiff.extend_from_slice(b"II");
    tiff.extend_from_slice(&42u16.to_le_bytes());
    tiff.extend_from_slice(&8u32.to_le_bytes());
    tiff.extend_from_slice(&directory);
    tiff.extend_from_slice(&values);

    Some(tiff)
}

fn nul_terminated(value: &str) -> Vec<u8> {
    let mut bytes = value.as_bytes().to_vec();
    bytes.push(0);
    bytes
}

/// Re-attaches a copyright statement to an encoded image.
///
/// libvips' `keep` flags are `none|exif|xmp|iptc|icc|other|gainmap|all`, with no
/// copyright granularity, so retaining one field across a metadata strip means
/// putting it back afterwards. Each container stores the same TIFF/Exif block
/// differently, so there is one writer per container and an untouched return
/// for anything else — callers can apply this unconditionally.
pub fn attach_copyright(encoded: Vec<u8>, copyright: &Copyright) -> Vec<u8> {
    if copyright.is_empty() {
        return encoded;
    }
    let Some(tiff) = build_exif_tiff(copyright) else {
        return encoded;
    };

    let attached = if is_jpeg(&encoded) {
        attach_to_jpeg(&encoded, &tiff)
    } else if is_png(&encoded) {
        attach_to_png(&encoded, &tiff)
    } else if is_webp(&encoded) {
        attach_to_webp(&encoded, &tiff)
    } else {
        // Not "cannot carry EXIF" — TIFF, AVIF and HEIF all can. imgforge just
        // has no writer for them, and saying otherwise sent anyone reading the
        // log looking for a container limitation that does not exist.
        debug!("Copyright retention skipped: imgforge does not write EXIF into this output container");
        None
    };

    attached.unwrap_or(encoded)
}

fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == MARKER_PREFIX && bytes[1] == MARKER_SOI
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(PNG_SIGNATURE)
}

fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP"
}

/// JPEG: an APP1 segment holding `Exif\0\0` and then the TIFF block, spliced in
/// directly after the start-of-image marker.
fn attach_to_jpeg(encoded: &[u8], tiff: &[u8]) -> Option<Vec<u8>> {
    let payload_len = EXIF_IDENTIFIER.len() + tiff.len();
    // A segment carries its own length in two bytes, including those two.
    if payload_len + 2 > usize::from(u16::MAX) {
        return None;
    }

    let mut out = Vec::with_capacity(encoded.len() + payload_len + 4);
    out.extend_from_slice(&encoded[..2]);
    out.push(MARKER_PREFIX);
    out.push(MARKER_APP1);
    out.extend_from_slice(&((payload_len + 2) as u16).to_be_bytes());
    out.extend_from_slice(EXIF_IDENTIFIER);
    out.extend_from_slice(tiff);
    out.extend_from_slice(&encoded[2..]);
    Some(out)
}

/// PNG: an `eXIf` chunk holding the TIFF block bare, placed before the first
/// `IDAT` as the specification requires.
///
/// Any `eXIf` chunk already present is dropped rather than left in place. A
/// reader takes the first one it finds, so appending a second would leave the
/// copyright unreadable behind whatever the encoder had already written.
fn attach_to_png(encoded: &[u8], tiff: &[u8]) -> Option<Vec<u8>> {
    let chunk = png_chunk(b"eXIf", tiff)?;

    let mut out = Vec::with_capacity(encoded.len() + chunk.len());
    out.extend_from_slice(PNG_SIGNATURE);

    let mut inserted = false;
    for (chunk_type, span) in png_chunks(encoded)? {
        if &chunk_type == b"eXIf" {
            continue;
        }
        // The specification wants metadata before the image data.
        if !inserted && &chunk_type == b"IDAT" {
            out.extend_from_slice(&chunk);
            inserted = true;
        }
        out.extend_from_slice(&encoded[span]);
    }

    inserted.then_some(out)
}

/// Walks a PNG's chunk list, yielding each chunk's type and its byte range.
fn png_chunks(encoded: &[u8]) -> Option<Vec<([u8; 4], std::ops::Range<usize>)>> {
    let mut chunks = Vec::new();
    let mut offset = PNG_SIGNATURE.len();

    while offset + 8 <= encoded.len() {
        let length = u32::from_be_bytes(encoded[offset..offset + 4].try_into().ok()?) as usize;
        let chunk_type: [u8; 4] = encoded[offset + 4..offset + 8].try_into().ok()?;
        // The length covers the data alone; the chunk also carries a 4-byte
        // length, a 4-byte type, and a 4-byte CRC.
        let end = offset.checked_add(length)?.checked_add(12)?;
        if end > encoded.len() {
            return None;
        }

        chunks.push((chunk_type, offset..end));
        offset = end;
    }

    Some(chunks)
}

fn png_chunk(chunk_type: &[u8; 4], data: &[u8]) -> Option<Vec<u8>> {
    let length = u32::try_from(data.len()).ok()?;

    let mut chunk = Vec::with_capacity(data.len() + 12);
    chunk.extend_from_slice(&length.to_be_bytes());
    chunk.extend_from_slice(chunk_type);
    chunk.extend_from_slice(data);
    // The CRC covers the type and the data, but not the length.
    chunk.extend_from_slice(&crc32(&chunk[4..]).to_be_bytes());
    Some(chunk)
}

/// CRC-32/ISO-HDLC, which is what PNG chunks carry.
///
/// Computed bitwise rather than through a table: a copyright string is a few
/// dozen bytes, so the table would cost more to build than it saves.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// WebP: an `EXIF` chunk holding the TIFF block bare, appended to the RIFF
/// container.
///
/// The container specification only permits metadata chunks in the *extended*
/// format, which is announced by a `VP8X` chunk carrying a flags byte. A simple
/// lossy or lossless WebP has no such chunk, so one is synthesised from the
/// canvas size; a file that already has one — every animation does — just has
/// its EXIF flag set. As with PNG, an existing `EXIF` chunk is replaced rather
/// than duplicated.
fn attach_to_webp(encoded: &[u8], tiff: &[u8]) -> Option<Vec<u8>> {
    let mut body: Vec<u8> = Vec::with_capacity(encoded.len() + tiff.len() + 16);
    let mut has_extended_header = false;

    for (fourcc, span) in webp_chunks(encoded)? {
        if &fourcc == b"EXIF" {
            continue;
        }
        let start = body.len();
        body.extend_from_slice(&encoded[span]);
        if &fourcc == b"VP8X" {
            // Bit 3 of the flags byte, which is the first byte of the payload,
            // marks the presence of an EXIF chunk.
            *body.get_mut(start + 8)? |= 0b0000_1000;
            has_extended_header = true;
        }
    }

    if !has_extended_header {
        let (width, height, has_alpha) = webp_canvas(&body)?;
        if width > 1 << 24 || height > 1 << 24 {
            return None;
        }

        let mut header = Vec::with_capacity(body.len() + 18);
        header.extend_from_slice(b"VP8X");
        header.extend_from_slice(&10u32.to_le_bytes());
        // Bit 3 marks EXIF; bit 4 marks alpha. Declaring EXIF while omitting
        // the alpha a VP8L bitstream actually carries leaves the container's
        // feature flags contradicting its contents, which a strict reader may
        // refuse or read as fully opaque.
        header.push(0b0000_1000 | if has_alpha { 0b0001_0000 } else { 0 });
        header.extend_from_slice(&[0, 0, 0]);
        // The canvas dimensions are stored as 24-bit values, one less than the
        // real size.
        header.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
        header.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
        header.extend_from_slice(&body);
        body = header;
    }

    body.extend_from_slice(b"EXIF");
    body.extend_from_slice(&u32::try_from(tiff.len()).ok()?.to_le_bytes());
    body.extend_from_slice(tiff);
    // Every RIFF chunk is padded to an even length.
    if !body.len().is_multiple_of(2) {
        body.push(0);
    }

    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(b"RIFF");
    // The RIFF size counts everything after itself, which includes "WEBP".
    out.extend_from_slice(&u32::try_from(body.len() + 4).ok()?.to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(&body);
    Some(out)
}

/// Walks a WebP's chunk list, yielding each chunk's fourCC and its byte range
/// including the header and any padding byte.
fn webp_chunks(encoded: &[u8]) -> Option<Vec<([u8; 4], std::ops::Range<usize>)>> {
    let mut chunks = Vec::new();
    let mut offset = 12;

    while offset + 8 <= encoded.len() {
        let fourcc: [u8; 4] = encoded[offset..offset + 4].try_into().ok()?;
        let size = u32::from_le_bytes(encoded[offset + 4..offset + 8].try_into().ok()?) as usize;
        let padded = size + usize::from(!size.is_multiple_of(2));
        let end = offset.checked_add(8)?.checked_add(padded)?;
        if end > encoded.len() {
            return None;
        }

        chunks.push((fourcc, offset..end));
        offset = end;
    }

    Some(chunks)
}

/// Reads the canvas size and alpha flag out of a simple WebP's bitstream.
///
/// Only needed for a file with no `VP8X` chunk, which by definition is a single
/// lossy (`VP8 `) or lossless (`VP8L`) frame.
fn webp_canvas(body: &[u8]) -> Option<(u32, u32, bool)> {
    let fourcc = body.get(..4)?;
    let payload = body.get(8..)?;

    if fourcc == b"VP8 " {
        // A key frame: a 3-byte tag, the 3-byte start code, then 14-bit
        // dimensions each followed by a 2-bit scale. Lossy WebP without a VP8X
        // header has no alpha channel.
        let header = payload.get(6..10)?;
        let width = u32::from(u16::from_le_bytes([header[0], header[1]]) & 0x3FFF);
        let height = u32::from(u16::from_le_bytes([header[2], header[3]]) & 0x3FFF);
        (width > 0 && height > 0).then_some((width, height, false))
    } else if fourcc == b"VP8L" {
        // A 1-byte signature, then 14 bits of width-1, 14 bits of height-1, and
        // a single alpha-is-used bit, packed little-endian.
        let bits = u32::from_le_bytes(payload.get(1..5)?.try_into().ok()?);
        let width = (bits & 0x3FFF) + 1;
        let height = ((bits >> 14) & 0x3FFF) + 1;
        let has_alpha = (bits >> 28) & 1 == 1;
        Some((width, height, has_alpha))
    } else {
        None
    }
}

/// Extracts the JPEG thumbnail embedded in a source's EXIF data.
///
/// The offsets recorded in IFD1 are relative to the start of the TIFF block, so
/// the APP1 segment has to be located first; that is why this does not simply
/// hand the whole file to the EXIF reader.
pub fn embedded_thumbnail(image_bytes: &[u8]) -> Option<Vec<u8>> {
    let tiff = jpeg_exif_block(image_bytes)?;
    let exif = exif::Reader::new().read_raw(tiff.to_vec()).ok()?;

    let offset = exif
        .get_field(Tag::JPEGInterchangeFormat, In::THUMBNAIL)?
        .value
        .get_uint(0)? as usize;
    let length = exif
        .get_field(Tag::JPEGInterchangeFormatLength, In::THUMBNAIL)?
        .value
        .get_uint(0)? as usize;

    let end = offset.checked_add(length)?;
    if length == 0 || end > tiff.len() {
        return None;
    }

    let thumbnail = &tiff[offset..end];
    // Anything that is not a JPEG stream is not something to hand the decoder.
    (thumbnail.starts_with(&[MARKER_PREFIX, MARKER_SOI])).then(|| thumbnail.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processing::save;
    use crate::test_support::init_vips;
    use libvips::{ops, VipsImage};

    fn copyright() -> Copyright {
        Copyright {
            copyright: Some("(c) 2026 Example".to_string()),
            artist: Some("A Photographer".to_string()),
        }
    }

    /// A real encode of the given format, so the container being spliced is the
    /// one imgforge actually produces rather than a hand-built approximation.
    fn encoded(format: &str) -> Vec<u8> {
        init_vips();
        let image = ops::black(17, 9).expect("probe image");
        // An odd width and height make the RIFF padding path matter.
        save::save_image(image, format, 80).expect("format encodes")
    }

    /// The whole point of `keep_copyright`: after a strip and a re-attach, an
    /// ordinary EXIF parser has to find the fields again. A splice that any
    /// tool cannot read is the same as having dropped them.
    #[test]
    fn copyright_round_trips_through_every_container_that_carries_exif() {
        for format in ["jpeg", "png", "webp"] {
            let bare = encoded(format);
            assert!(
                read_copyright(&bare).is_empty(),
                "{format}: the probe should start with no copyright"
            );

            let tagged = attach_copyright(bare.clone(), &copyright());
            assert_ne!(tagged, bare, "{format}: nothing was spliced in");
            assert_eq!(read_copyright(&tagged), copyright(), "{format}: not readable again");

            // And the result must still decode as an image of the same size.
            let decoded = VipsImage::new_from_buffer(&tagged, "").expect("tagged image still decodes");
            assert_eq!((decoded.get_width(), decoded.get_height()), (17, 9), "{format}");
        }
    }

    /// An animated WebP already carries a VP8X chunk, so the writer sets its
    /// EXIF flag rather than synthesising a second one.
    #[test]
    fn copyright_survives_on_a_webp_that_already_has_an_extended_header() {
        init_vips();
        // A gradient rather than a flat colour: libwebp collapses frames that
        // are byte-identical, so three black frames would encode as one and the
        // test would prove nothing about whether the splice kept them.
        let gradient = ops::xyz(16, 24).expect("probe image");
        let gradient = ops::cast(&gradient, ops::BandFormat::Uchar).expect("cast to 8 bit");
        let animated = save::save_image_with_options(
            gradient,
            "webp",
            80,
            &crate::processing::options::SaveOptions::default(),
            Some(8),
            None,
        )
        .expect("animated webp encodes");

        let before = VipsImage::new_from_buffer(&animated, "n=-1").expect("animation decodes");
        assert_eq!(before.get_n_pages(), 3, "the probe should be a three-frame animation");

        let tagged = attach_copyright(animated, &copyright());
        assert_eq!(read_copyright(&tagged), copyright());

        // The extended header was already there, so the writer sets its EXIF
        // flag rather than synthesising a second one — which would leave the
        // file with two VP8X chunks and no decoder willing to read it.
        let decoded = VipsImage::new_from_buffer(&tagged, "n=-1").expect("tagged animation still decodes");
        assert_eq!(decoded.get_n_pages(), 3, "the frames must survive the splice");
    }

    #[test]
    fn attaching_nothing_leaves_the_bytes_alone() {
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x02];
        assert_eq!(attach_copyright(jpeg.clone(), &Copyright::default()), jpeg);

        // A container with nowhere to put EXIF is returned untouched rather
        // than corrupted with a chunk it cannot describe.
        let tiff = b"II*\0rest of a tiff".to_vec();
        assert_eq!(attach_copyright(tiff.clone(), &copyright()), tiff);
    }

    #[test]
    fn a_source_without_exif_has_no_copyright_and_no_thumbnail() {
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x02];
        assert!(read_copyright(&jpeg).is_empty());
        assert_eq!(embedded_thumbnail(&jpeg), None);
        assert_eq!(embedded_thumbnail(b"not an image"), None);
    }

    #[test]
    fn png_chunk_crcs_match_the_reference_algorithm() {
        // IEND is the one PNG chunk whose bytes are fixed by the specification,
        // so its CRC is a published constant to check the implementation against.
        let chunk = png_chunk(b"IEND", &[]).expect("chunk builds");
        assert_eq!(chunk, vec![0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]);
    }
}
