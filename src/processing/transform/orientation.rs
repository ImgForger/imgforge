//! Rotation and flipping, both the EXIF-driven kind and the kind the URL asks
//! for explicitly.

use super::{vips, TransformError};
use crate::processing::options::Flip;
use crate::utils::read_exif_orientation;
use libvips::{ops, VipsImage};
use tracing::debug;

/// Applies EXIF rotation to an image based on orientation data.
pub fn apply_exif_rotation(image_bytes: &[u8], mut img: VipsImage) -> Result<VipsImage, TransformError> {
    if let Some(orientation) = read_exif_orientation(image_bytes) {
        debug!("Found EXIF orientation: {:?}", orientation);
        img = apply_exif_orientation(img, orientation)?;
    }
    Ok(img)
}

pub fn apply_exif_orientation(mut img: VipsImage, orientation: u32) -> Result<VipsImage, TransformError> {
    match orientation {
        2 => img = ops::flip(&img, ops::Direction::Horizontal).map_err(vips("Error flipping horizontally"))?,
        3 => img = ops::rot(&img, ops::Angle::D180).map_err(vips("Error rotating 180"))?,
        4 => img = ops::flip(&img, ops::Direction::Vertical).map_err(vips("Error flipping vertically"))?,
        5 => {
            img = ops::flip(
                &ops::rot(&img, ops::Angle::D90).map_err(vips("Error rotating 90"))?,
                ops::Direction::Horizontal,
            )
            .map_err(vips("Error flipping after rotate"))?
        }
        6 => img = ops::rot(&img, ops::Angle::D90).map_err(vips("Error rotating 90"))?,
        7 => {
            img = ops::flip(
                &ops::rot(&img, ops::Angle::D270).map_err(vips("Error rotating 270"))?,
                ops::Direction::Horizontal,
            )
            .map_err(vips("Error flipping after rotate"))?
        }
        8 => img = ops::rot(&img, ops::Angle::D270).map_err(vips("Error rotating 270"))?,
        _ => {}
    }
    Ok(img)
}

/// Applies rotation to an image.
pub fn apply_rotation(img: VipsImage, rotation: u16) -> Result<VipsImage, TransformError> {
    match rotation {
        0 => Ok(img),
        90 => ops::rot(&img, ops::Angle::D90).map_err(vips("Error rotating 90")),
        180 => ops::rot(&img, ops::Angle::D180).map_err(vips("Error rotating 180")),
        270 => ops::rot(&img, ops::Angle::D270).map_err(vips("Error rotating 270")),
        _ => Err(TransformError::invalid(
            "rotation",
            format!("Unsupported rotation angle: {rotation}"),
        )),
    }
}

/// Applies horizontal and/or vertical flips to an image.
pub fn apply_flip(mut img: VipsImage, flip: Flip) -> Result<VipsImage, TransformError> {
    if flip.horizontal {
        img = ops::flip(&img, ops::Direction::Horizontal).map_err(vips("Error flipping horizontally"))?;
    }
    if flip.vertical {
        img = ops::flip(&img, ops::Direction::Vertical).map_err(vips("Error flipping vertically"))?;
    }
    Ok(img)
}
