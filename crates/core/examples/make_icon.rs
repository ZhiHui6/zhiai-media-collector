use std::io::Cursor;
use std::path::Path;

use image::codecs::png::PngEncoder;
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageEncoder, Rgba, RgbaImage};

fn main() {
    let dir = Path::new("src-tauri/icons");
    std::fs::create_dir_all(dir).expect("create icons dir");

    let base = render(1024);
    save_png(&base, &dir.join("icon.png"));
    save_png(
        &image::imageops::resize(&base, 256, 256, FilterType::Lanczos3),
        &dir.join("128x128@2x.png"),
    );
    save_png(
        &image::imageops::resize(&base, 128, 128, FilterType::Lanczos3),
        &dir.join("128x128.png"),
    );
    save_png(
        &image::imageops::resize(&base, 32, 32, FilterType::Lanczos3),
        &dir.join("32x32.png"),
    );

    let mut sizes: Vec<(u32, Vec<u8>)> = Vec::new();
    for size in [16u32, 24, 32, 48, 64, 128, 256] {
        let scaled = image::imageops::resize(&base, size, size, FilterType::Lanczos3);
        sizes.push((size, encode_png(&scaled)));
    }
    write_ico(&dir.join("icon.ico"), &sizes);
    println!("icons written to {}", dir.display());
}

fn render(size: u32) -> RgbaImage {
    let n = size as f32;
    let mut image = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    let inset = n * 0.055;
    let radius = n * 0.215;
    let center = n / 2.0;
    let half = n / 2.0 - inset;
    let stroke = n * 0.088;
    let green = Rgba([0x07, 0xC1, 0x60, 255]);
    let white = Rgba([255, 255, 255, 255]);

    let points = [
        (n * 0.315, n * 0.305),
        (n * 0.685, n * 0.305),
        (n * 0.315, n * 0.695),
        (n * 0.685, n * 0.695),
    ];
    let segments = [
        (points[0], points[1]),
        (points[1], points[2]),
        (points[2], points[3]),
    ];

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let qx = (px - center).abs() - (half - radius);
            let qy = (py - center).abs() - (half - radius);
            let outside =
                (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - radius;
            let coverage = (0.75 - outside).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            let mut pixel = green;
            pixel.0[3] = (coverage * 255.0) as u8;

            let mut mark = 0.0f32;
            for (a, b) in segments {
                let distance = segment_distance(px, py, a, b) - stroke / 2.0;
                mark = mark.max((0.75 - distance).clamp(0.0, 1.0));
            }
            if mark > 0.0 {
                let mix = mark.min(1.0);
                for channel in 0..3 {
                    pixel.0[channel] = (white.0[channel] as f32 * mix
                        + green.0[channel] as f32 * (1.0 - mix))
                        as u8;
                }
            }
            image.put_pixel(x, y, pixel);
        }
    }
    image
}

fn segment_distance(px: f32, py: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let length = (dx * dx + dy * dy).sqrt();
    if length == 0.0 {
        return ((px - a.0).powi(2) + (py - a.1).powi(2)).sqrt();
    }
    let t = (((px - a.0) * dx + (py - a.1) * dy) / (length * length)).clamp(0.0, 1.0);
    let cx = a.0 + dx * t;
    let cy = a.1 + dy * t;
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

fn encode_png(image: &RgbaImage) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    PngEncoder::new(&mut output)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .expect("encode png");
    output.into_inner()
}

fn save_png(image: &RgbaImage, path: &Path) {
    std::fs::write(path, encode_png(image)).expect("write png");
}

fn write_ico(path: &Path, images: &[(u32, Vec<u8>)]) {
    let count = images.len() as u16;
    let mut offset = 6 + 16 * count as u32;
    let mut data = Vec::new();
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&count.to_le_bytes());
    for (size, png) in images {
        let dimension = if *size >= 256 { 0u8 } else { *size as u8 };
        data.push(dimension);
        data.push(dimension);
        data.push(0);
        data.push(0);
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&32u16.to_le_bytes());
        data.extend_from_slice(&(png.len() as u32).to_le_bytes());
        data.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in images {
        data.extend_from_slice(png);
    }
    std::fs::write(path, data).expect("write ico");
}
