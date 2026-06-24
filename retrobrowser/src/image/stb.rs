use super::{Image, ImageError};

extern "C" {
    fn stbi_load_from_memory(
        buffer:           *const u8,
        len:              i32,
        x:                *mut i32,
        y:                *mut i32,
        channels_in_file: *mut i32,
        desired_channels: i32,
    ) -> *mut u8;

    fn stbi_image_free(ptr: *mut u8);

    fn stbir_resize_uint8_srgb(
        input_pixels:           *const u8,
        input_w:                i32,
        input_h:                i32,
        input_stride_in_bytes:  i32,
        output_pixels:          *mut u8,
        output_w:               i32,
        output_h:               i32,
        output_stride_in_bytes: i32,
        pixel_layout:           i32,
    ) -> *mut u8;
}

pub fn decode_stb(data: &[u8]) -> Result<Image, ImageError> {
    let mut w        = 0i32;
    let mut h        = 0i32;
    let mut channels = 0i32;

    let ptr = unsafe {
        stbi_load_from_memory(
            data.as_ptr(),
            data.len() as i32,
            &mut w,
            &mut h,
            &mut channels,
            4, // force RGBA
        )
    };

    if ptr.is_null() {
        return Err(ImageError("stb_image: decode failed".into()));
    }

    let pixel_count = (w * h * 4) as usize;
    let pixels = unsafe {
        std::slice::from_raw_parts(ptr, pixel_count).to_vec()
    };
    unsafe { stbi_image_free(ptr); }

    let mut image = Image::new(w as u32, h as u32);
    image.pixels = pixels;
    Ok(image)
}

pub fn resize_stb(src: &Image, target_w: u32, target_h: u32) -> Image {
    let mut dst = Image::new(target_w, target_h);

    let result = unsafe {
        stbir_resize_uint8_srgb(
            src.pixels.as_ptr(),
            src.width  as i32,
            src.height as i32,
            0, // stride 0 = tightly packed
            dst.pixels.as_mut_ptr(),
            target_w as i32,
            target_h as i32,
            0,
            4, // STBIR_RGBA
        )
    };

    if result.is_null() {
        eprintln!("[RetroX] stb_image_resize2: resize failed");
    }

    dst
}