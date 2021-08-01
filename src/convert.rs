use bindings::Windows::Win32::Graphics::Gdi::BITMAPINFO;
use image::{Pixel, Rgb, RgbImage};

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

pub fn dib_to_image(dib_image: *const BITMAPINFO) -> Result<RgbImage, String> {
    unsafe {
        if dib_image.is_null() {
            return Err("Image is null".to_string());
        }

        if (*dib_image).bmiHeader.biCompression != 3 {
            return Err("Unsupported compression format".to_string());
        }

        if (*dib_image).bmiHeader.biBitCount != 32 {
            return Err("Unsupported bit depth".to_string());
        }

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
