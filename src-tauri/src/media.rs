use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::sync::{Arc, Mutex};

use image::{ImageFormat, RgbaImage};

const LIMIT: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MediaKind {
    Thumb,
    Full,
}

pub struct MediaCache {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    map: HashMap<(MediaKind, u64), Arc<Vec<u8>>>,
    order: VecDeque<(MediaKind, u64)>,
}

impl MediaCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }

    pub fn insert_rgba(
        &self,
        kind: MediaKind,
        id: u64,
        rgba: Vec<u8>,
        width: usize,
        height: usize,
    ) {
        let Some(png) = encode_png(&rgba, width, height) else {
            return;
        };
        self.insert(kind, id, png);
    }

    fn insert(&self, kind: MediaKind, id: u64, png: Vec<u8>) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        let key = (kind, id);
        if inner.map.insert(key, Arc::new(png)).is_none() {
            inner.order.push_back(key);
        }
        while inner.order.len() > LIMIT {
            if let Some(oldest) = inner.order.pop_front() {
                inner.map.remove(&oldest);
            }
        }
    }

    pub fn get(&self, kind: MediaKind, id: u64) -> Option<Arc<Vec<u8>>> {
        self.inner.lock().ok()?.map.get(&(kind, id)).cloned()
    }
}

fn encode_png(rgba: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || rgba.len() != width.checked_mul(height)?.checked_mul(4)? {
        return None;
    }
    let image = RgbaImage::from_raw(width as u32, height as u32, rgba.to_vec())?;
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, ImageFormat::Png).ok()?;
    Some(output.into_inner())
}

/// 按磁盘路径读取图片字节，返回 (内容, MIME 类型)。
pub fn read_local(path: &str) -> Option<(Vec<u8>, &'static str)> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some((bytes, mime_of(path)))
}

fn mime_of(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        _ => "image/png",
    }
}
