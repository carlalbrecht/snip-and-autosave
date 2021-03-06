//! Extension methods for various types.

use crate::settings::Settings;
use bindings::Windows::Win32::Foundation::PSTR;
use image::codecs::png::PngDecoder;
use image::{ColorType, DynamicImage, ImageDecoder, RgbImage};
use rayon::prelude::*;
use std::ffi::CString;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;
use std::{fs, io};

/// Extension methods for [`CString`] instances.
///
/// [`CString`]: CString
pub trait CStringExtensions {
    /// Gets a [`PSTR`], which points to the character data of this string.
    ///
    /// # Safety
    ///
    /// The returned pointer is always mutable. Many Win32 functions do not
    /// actually mutate the data, but this cannot be guaranteed.
    unsafe fn as_pstr(&self) -> PSTR;
}

impl CStringExtensions for CString {
    unsafe fn as_pstr(&self) -> PSTR {
        PSTR(self.as_ptr() as *mut u8)
    }
}

/// Extension methods for [`ImageBuffer`] instances.
///
/// [`ImageBuffer`]: image::ImageBuffer
pub trait ImageExtensions {
    /// Returns whether or not this image is the same as the last captured
    /// screenshot (i.e. has equal dimensions and pixel content).
    fn is_same_as_last_screenshot(&self) -> bool;
}

impl ImageExtensions for RgbImage {
    fn is_same_as_last_screenshot(&self) -> bool {
        let mut screenshot_path = PathBuf::new();
        Settings::read(|s| screenshot_path = s.paths.screenshots.clone());

        if let Ok(Some(newest_file)) = newest_file_in_dir(&screenshot_path) {
            println!(
                "Newest file in screenshot dir: {}",
                newest_file.to_string_lossy()
            );

            // TODO clean this up :(
            if let Ok(file) = File::open(newest_file) {
                if let Ok(decoder) = PngDecoder::new(file) {
                    // Fail-fast if the image isn't comparable to our new screenshot
                    if decoder.dimensions() != self.dimensions() {
                        return false;
                    }

                    if decoder.color_type() != ColorType::Rgb8 {
                        return false;
                    }

                    // There's a good chance that this image might actually be equal to our new
                    // screenshot, so we now go to the effort of decoding it
                    if let Ok(image) = DynamicImage::from_decoder(decoder) {
                        if let Some(image) = image.as_rgb8() {
                            return image_content_is_equal(self, image);
                        }
                    }
                }
            }
        }

        false
    }
}

/// Calculates in parallel, row by row, whether or not two images, with equal
/// dimensions, have the same pixel content.
fn image_content_is_equal(image_a: &RgbImage, image_b: &RgbImage) -> bool {
    if image_a.dimensions() != image_b.dimensions() {
        return false;
    }

    let result = AtomicBool::new(true);

    image_a
        .rows()
        .zip(image_b.rows())
        .par_bridge()
        .for_each(|(row_a, row_b)| {
            if !result.load(Ordering::Relaxed) {
                // Skip processing rows once we know that the images aren't equal
                return;
            }

            for (a, b) in row_a.zip(row_b) {
                if a != b {
                    result.store(false, Ordering::Relaxed);
                }
            }
        });

    result.into_inner()
}

/// Gets the path to the last-created file in a directory.
///
/// Note that this function uses files' created at time, not modified at.
fn newest_file_in_dir(dir: &Path) -> io::Result<Option<PathBuf>> {
    if !dir.exists() {
        return Ok(None);
    }

    assert!(dir.is_dir());

    let mut newest_path = None;
    let mut newest_time = SystemTime::UNIX_EPOCH;

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let metadata = fs::metadata(&path)?;
        let created_at = metadata.created()?;

        if created_at > newest_time {
            newest_path = Some(path);
            newest_time = created_at;
        }
    }

    Ok(newest_path)
}
