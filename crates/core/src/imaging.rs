use std::io::Cursor;

use image::{DynamicImage, ExtendedColorType, ImageFormat, RgbImage};

use crate::error::ImagingError;
use crate::model::ConvertTarget;

pub struct Processed {
    pub bytes: Vec<u8>,
    pub format: String,
    pub extension: String,
    pub converted: bool,
}

pub fn sniff(bytes: &[u8]) -> Option<ImageFormat> {
    image::guess_format(bytes).ok()
}

/// 判断文件头是否属于已知图片格式，用于识别“长度对但内容坏了”的残留文件。
pub fn has_known_magic(head: &[u8]) -> bool {
    if head.starts_with(&[0xFF, 0xD8])
        || head.starts_with(b"\x89PNG\r\n\x1a\n")
        || head.starts_with(b"GIF8")
        || head.starts_with(b"BM")
        || head.starts_with(b"II*\0")
        || head.starts_with(b"MM\0*")
    {
        return true;
    }
    if head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        return true;
    }
    head.len() >= 12 && &head[4..8] == b"ftyp"
}

pub fn format_name(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "JPEG",
        ImageFormat::Png => "PNG",
        ImageFormat::Gif => "GIF",
        ImageFormat::WebP => "WebP",
        ImageFormat::Bmp => "BMP",
        ImageFormat::Tiff => "TIFF",
        ImageFormat::Avif => "AVIF",
        ImageFormat::Ico => "ICO",
        _ => "未知",
    }
}

pub fn extension_for(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Png => "png",
        ImageFormat::Gif => "gif",
        ImageFormat::WebP => "webp",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
        ImageFormat::Avif => "avif",
        ImageFormat::Ico => "ico",
        _ => "dat",
    }
}

fn target_format(target: ConvertTarget) -> ImageFormat {
    match target {
        ConvertTarget::Jpg => ImageFormat::Jpeg,
        ConvertTarget::Png => ImageFormat::Png,
        ConvertTarget::WebP => ImageFormat::WebP,
    }
}

pub fn process(
    bytes: Vec<u8>,
    target: Option<ConvertTarget>,
    jpeg_quality: u8,
    webp_quality: u8,
) -> Result<Processed, ImagingError> {
    let Some(format) = sniff(&bytes) else {
        return Ok(Processed {
            bytes,
            format: "未知".to_string(),
            extension: "dat".to_string(),
            converted: false,
        });
    };
    let unchanged = |bytes: Vec<u8>| Processed {
        bytes,
        format: format_name(format).to_string(),
        extension: extension_for(format).to_string(),
        converted: false,
    };
    let Some(target) = target else {
        return Ok(unchanged(bytes));
    };
    if format == target_format(target) || format == ImageFormat::Gif || is_animated_webp(&bytes) {
        return Ok(unchanged(bytes));
    }
    let decoded = image::load_from_memory_with_format(&bytes, format)
        .map_err(|e| ImagingError::Decode(e.to_string()))?;
    let encoded = match target {
        ConvertTarget::Jpg => encode_jpeg(&decoded, jpeg_quality)?,
        ConvertTarget::Png => {
            let mut output = Cursor::new(Vec::new());
            decoded
                .write_to(&mut output, ImageFormat::Png)
                .map_err(|e| ImagingError::Convert(e.to_string()))?;
            output.into_inner()
        }
        ConvertTarget::WebP => encode_webp(&decoded, webp_quality),
    };
    Ok(Processed {
        bytes: encoded,
        format: target.label().to_string(),
        extension: target.extension().to_string(),
        converted: true,
    })
}

fn encode_jpeg(image: &DynamicImage, quality: u8) -> Result<Vec<u8>, ImagingError> {
    let rgb = flatten_to_rgb(image);
    let mut output = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality);
    encoder
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )
        .map_err(|e| ImagingError::Convert(e.to_string()))?;
    Ok(output)
}

fn flatten_to_rgb(image: &DynamicImage) -> RgbImage {
    if !image.color().has_alpha() {
        return image.to_rgb8();
    }
    let rgba = image.to_rgba8();
    let mut rgb = RgbImage::new(rgba.width(), rgba.height());
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = pixel[3] as u16;
        let blend = |channel: u8| ((channel as u16 * alpha + 255 * (255 - alpha)) / 255) as u8;
        rgb.put_pixel(
            x,
            y,
            image::Rgb([blend(pixel[0]), blend(pixel[1]), blend(pixel[2])]),
        );
    }
    rgb
}

fn encode_webp(image: &DynamicImage, quality: u8) -> Vec<u8> {
    let rgba = image.to_rgba8();
    let encoder = webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height());
    encoder.encode(quality as f32).to_vec()
}

fn is_animated_webp(bytes: &[u8]) -> bool {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return false;
    }
    let mut position = 12;
    while position + 8 <= bytes.len() {
        let tag = &bytes[position..position + 4];
        if tag == b"ANIM" || tag == b"ANMF" {
            return true;
        }
        let size =
            u32::from_le_bytes(bytes[position + 4..position + 8].try_into().unwrap()) as usize;
        position = position
            .saturating_add(8)
            .saturating_add(size)
            .saturating_add(size & 1);
    }
    false
}

pub fn thumbnail(bytes: &[u8], max_side: u32) -> Option<(Vec<u8>, usize, usize)> {
    let image = image::load_from_memory(bytes).ok()?;
    let rgba = image.thumbnail(max_side, max_side).to_rgba8();
    let (width, height) = rgba.dimensions();
    Some((rgba.into_raw(), width as usize, height as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIF_1PX: &[u8] = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";

    fn sample_png() -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(8, 6, image::Rgba([200, 30, 40, 128]));
        let mut output = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut output, ImageFormat::Png)
            .unwrap();
        output.into_inner()
    }

    #[test]
    fn sniffs_real_format() {
        assert_eq!(sniff(&sample_png()), Some(ImageFormat::Png));
        assert_eq!(sniff(GIF_1PX), Some(ImageFormat::Gif));
    }

    #[test]
    fn recognizes_known_magic_heads() {
        assert!(has_known_magic(&sample_png()));
        assert!(has_known_magic(GIF_1PX));
        assert!(has_known_magic(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(has_known_magic(b"RIFF\x10\x00\x00\x00WEBPVP8 "));
        assert!(!has_known_magic(b"<html>not an image"));
        assert!(!has_known_magic(b""));
    }

    #[test]
    fn keeps_original_without_target() {
        let png = sample_png();
        let processed = process(png.clone(), None, 92, 85).unwrap();
        assert!(!processed.converted);
        assert_eq!(processed.extension, "png");
        assert_eq!(processed.bytes, png);
    }

    #[test]
    fn converts_to_jpeg_and_webp() {
        let jpeg = process(sample_png(), Some(ConvertTarget::Jpg), 80, 80).unwrap();
        assert!(jpeg.converted);
        assert_eq!(&jpeg.bytes[0..3], &[0xFF, 0xD8, 0xFF]);
        assert_eq!(jpeg.extension, "jpg");

        let webp = process(sample_png(), Some(ConvertTarget::WebP), 80, 80).unwrap();
        assert!(webp.converted);
        assert_eq!(&webp.bytes[0..4], b"RIFF");
        assert_eq!(&webp.bytes[8..12], b"WEBP");
    }

    #[test]
    fn never_converts_gif_or_animated_webp() {
        let gif = process(GIF_1PX.to_vec(), Some(ConvertTarget::Jpg), 90, 90).unwrap();
        assert!(!gif.converted);
        assert_eq!(gif.extension, "gif");

        let animated =
            b"RIFF\x00\x00\x00\x00WEBPANIM\x06\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00".to_vec();
        assert!(is_animated_webp(&animated));
        let processed = process(animated, Some(ConvertTarget::Png), 90, 90).unwrap();
        assert!(!processed.converted);
    }

    #[test]
    fn unknown_bytes_keep_dat_extension() {
        let processed =
            process(b"not an image".to_vec(), Some(ConvertTarget::Png), 90, 90).unwrap();
        assert_eq!(processed.extension, "dat");
        assert_eq!(processed.format, "未知");
        assert!(!processed.converted);
    }

    #[test]
    fn thumbnail_fits_side_limit() {
        let (rgba, width, height) = thumbnail(&sample_png(), 4).unwrap();
        assert!(width <= 4 && height <= 4);
        assert_eq!(rgba.len(), width * height * 4);
    }
}
