//! Data format conversion routines.

use crate::windows::Clipboard;
use bindings::Windows::Win32::Graphics::Gdi::{BITMAPINFO, BI_BITFIELDS};
use image::{Pixel, Rgb, RgbImage};
use thiserror::Error;

/// Errors that can occur whilst converting an image.
#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("Image data pointer is null")]
    NullPointer,
    #[error("Image uses unsupported compression format {0}")]
    UnsupportedCompressionFormat(u32),
    #[error("Image has an unsupported bit depth of {0}-bits")]
    UnsupportedBitDepth(u16),
}

/// Reads the subpixel byte order of a device-independent bitmap.
///
/// E.g. a return value of `(0, 1, 2)` means that the red byte is the first
/// byte, followed by the green, then blue bytes (i.e. RGB subpixel ordering).
unsafe fn subpixel_ordering(color_masks: *const u32) -> (u32, u32, u32) {
    let red_mask = *color_masks;
    let green_mask = *color_masks.offset(1);
    let blue_mask = *color_masks.offset(2);

    // Don't ever run this on a big endian system :^)
    (
        (red_mask.trailing_zeros() / 8),
        (green_mask.trailing_zeros() / 8),
        (blue_mask.trailing_zeros() / 8),
    )
}

/// Copies the image data from a device-independent bitmap into an [`RgbImage`].
///
/// This function can currently only handle [`BI_BITFIELDS`] formatted DIB
/// images, with a bit depth of 32-bpp.
///
/// This function can handle various subpixel orders, as well as both bottom and
/// top-left corner origins.
///
/// [`RgbImage`]: RgbImage
/// [`BI_BITFIELDS`]: BI_BITFIELDS
pub fn dib_to_image(
    dib_image: *const BITMAPINFO,
    _clipboard: &Clipboard,
) -> Result<RgbImage, ConversionError> {
    unsafe {
        // Pre-flight sanity checks
        if dib_image.is_null() {
            return Err(ConversionError::NullPointer);
        }

        let compression_format = (*dib_image).bmiHeader.biCompression;
        let bit_depth = (*dib_image).bmiHeader.biBitCount;

        if compression_format != BI_BITFIELDS as u32 {
            return Err(ConversionError::UnsupportedCompressionFormat(
                compression_format,
            ));
        }

        if bit_depth != 32 {
            return Err(ConversionError::UnsupportedBitDepth(bit_depth));
        }

        // Read DIB header
        let width = (*dib_image).bmiHeader.biWidth.abs() as u32;
        let height = (*dib_image).bmiHeader.biHeight;

        // Detect bottom-left corner origin
        let flip = height > 0;
        let height = height.abs() as u32;

        let bytes = (*dib_image).bmiHeader.biSizeImage;
        let data_offset = (*dib_image).bmiHeader.biSize;

        let dib_image_bytes = dib_image as *const u8;
        let color_masks = dib_image_bytes.offset(data_offset as isize) as *const u32;
        let image_data = color_masks.offset(3) as *const u8;

        let (r, g, b) = subpixel_ordering(color_masks);

        // Copy pixel data
        let mut image = RgbImage::new(width as u32, height as u32);

        for i in (0..bytes).step_by(4) {
            let px = i / 4;
            let x = px % width;
            let y = if flip {
                height - (px / width) - 1
            } else {
                px / width
            };

            image.put_pixel(
                x,
                y,
                Rgb::from_channels(
                    *image_data.offset((i + r) as isize),
                    *image_data.offset((i + g) as isize),
                    *image_data.offset((i + b) as isize),
                    0,
                ),
            );
        }

        Ok(image)
    }
}
