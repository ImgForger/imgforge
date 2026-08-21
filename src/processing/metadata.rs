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

/// Builds a minimal little-endian Exif APP1 payload carrying only the fields in
/// `copyright`.
fn build_exif_payload(copyright: &Copyright) -> Option<Vec<u8>> {
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

    let mut payload = Vec::from(EXIF_IDENTIFIER);
    payload.extend_from_slice(b"II");
    payload.extend_from_slice(&42u16.to_le_bytes());
    payload.extend_from_slice(&8u32.to_le_bytes());
    payload.extend_from_slice(&directory);
    payload.extend_from_slice(&values);

    // An APP1 segment carries its own length in two bytes, including those two.
    (payload.len() + 2 <= usize::from(u16::MAX)).then_some(payload)
}

fn nul_terminated(value: &str) -> Vec<u8> {
    let mut bytes = value.as_bytes().to_vec();
    bytes.push(0);
    bytes
}

/// Re-attaches a copyright statement to encoded JPEG bytes.
///
/// Only JPEG: it is the format that carries EXIF natively and the one that
/// nearly every copyright-bearing source uses. Returns the input untouched when
/// there is nothing to attach or the output is not a JPEG, so callers can apply
/// it unconditionally.
pub fn attach_copyright(encoded: Vec<u8>, copyright: &Copyright) -> Vec<u8> {
    if copyright.is_empty() {
        return encoded;
    }
    if encoded.len() < 2 || encoded[0] != MARKER_PREFIX || encoded[1] != MARKER_SOI {
        debug!("Copyright retention skipped: output is not a JPEG");
        return encoded;
    }
    let Some(payload) = build_exif_payload(copyright) else {
        return encoded;
    };

    let mut out = Vec::with_capacity(encoded.len() + payload.len() + 4);
    out.extend_from_slice(&encoded[..2]);
    out.push(MARKER_PREFIX);
    out.push(MARKER_APP1);
    out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&payload);
    out.extend_from_slice(&encoded[2..]);
    out
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

    #[test]
    fn copyright_round_trips_through_a_rebuilt_app1_segment() {
        // A JPEG stripped of metadata, then given its copyright back, has to be
        // readable by an ordinary EXIF parser again — otherwise `keep_copyright`
        // silently produces a file whose copyright no tool can find.
        let mut jpeg = vec![0xFF, 0xD8];
        jpeg.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02]);

        let copyright = Copyright {
            copyright: Some("(c) 2026 Example".to_string()),
            artist: Some("A Photographer".to_string()),
        };

        let tagged = attach_copyright(jpeg.clone(), &copyright);
        assert_ne!(tagged, jpeg, "the segment should have been spliced in");
        assert_eq!(read_copyright(&tagged), copyright);
    }

    #[test]
    fn attaching_nothing_leaves_the_bytes_alone() {
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x02];
        assert_eq!(attach_copyright(jpeg.clone(), &Copyright::default()), jpeg);

        // A non-JPEG output cannot carry an APP1 segment, so it is returned
        // untouched rather than corrupted with one.
        let png = vec![0x89, b'P', b'N', b'G'];
        let copyright = Copyright {
            copyright: Some("(c) 2026".to_string()),
            artist: None,
        };
        assert_eq!(attach_copyright(png.clone(), &copyright), png);
    }

    #[test]
    fn a_source_without_exif_has_no_copyright_and_no_thumbnail() {
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x02];
        assert!(read_copyright(&jpeg).is_empty());
        assert_eq!(embedded_thumbnail(&jpeg), None);
        assert_eq!(embedded_thumbnail(b"not an image"), None);
    }
}
