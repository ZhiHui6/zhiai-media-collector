use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::error::DownloadError;
use crate::extract::normalize::media_hash;
use crate::fetch::REFERER;
use crate::imaging;
use crate::model::{HistoryEntry, ImageItem, Settings, UiEvent};
use crate::storage;

use super::EventSink;
use super::limiter::BandwidthLimiter;

pub const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_VIDEO_BYTES: u64 = 4 * 1024 * 1024 * 1024;

const MAGIC_HEAD_BYTES: usize = 16;

pub type ReservedPaths = Arc<Mutex<HashSet<String>>>;

pub fn new_reserved() -> ReservedPaths {
    Arc::new(Mutex::new(HashSet::new()))
}

pub struct TaskOutcome {
    pub path: String,
    pub bytes: u64,
    pub format: String,
    pub skipped: bool,
}

pub async fn run(
    client: &reqwest::Client,
    item: &ImageItem,
    settings: &Settings,
    limiter: &Arc<BandwidthLimiter>,
    reserved: &ReservedPaths,
    sink: &EventSink,
) -> Result<TaskOutcome, DownloadError> {
    if item.kind.is_video() {
        return run_video(client, item, settings, limiter, reserved, sink).await;
    }
    run_image(client, item, settings, limiter, reserved, sink).await
}

async fn run_image(
    client: &reqwest::Client,
    item: &ImageItem,
    settings: &Settings,
    limiter: &Arc<BandwidthLimiter>,
    reserved: &ReservedPaths,
    sink: &EventSink,
) -> Result<TaskOutcome, DownloadError> {
    let payload = download(client, item, limiter, sink).await?;
    let target = settings.convert_enabled.then_some(settings.convert_target);
    let jpeg_quality = settings.jpeg_quality.clamp(1, 100);
    let webp_quality = settings.webp_quality.clamp(1, 100);
    let processed = tokio::task::spawn_blocking(move || {
        imaging::process(payload, target, jpeg_quality, webp_quality)
    })
    .await
    .map_err(|e| DownloadError::Imaging(e.to_string()))?
    .map_err(|e| DownloadError::Imaging(e.to_string()))?;

    let directory = storage::output_dir(&settings.download_root, &item.page_title);
    std::fs::create_dir_all(&directory).map_err(|e| DownloadError::Io(e.to_string()))?;
    let hash = media_hash(&item.url).unwrap_or_else(|| format!("{:03}", item.seq));
    let stem = storage::file_stem(item.seq, item.kind.slug(), &hash);
    let Some(path) = reserve(&directory, &stem, &processed.extension, settings, reserved) else {
        return Ok(skipped());
    };

    let bytes = processed.bytes.len() as u64;
    if !file_matches(&path, bytes, &processed.bytes) {
        let part = path.with_extension("part");
        std::fs::write(&part, &processed.bytes).map_err(|e| DownloadError::Io(e.to_string()))?;
        std::fs::rename(&part, &path).map_err(|e| DownloadError::Io(e.to_string()))?;
    }

    let path_string = path.to_string_lossy().into_owned();
    let format = processed.format;
    emit_completed(item, &path_string, bytes, &format, sink);

    let thumbnail_source = processed.bytes.clone();
    let id = item.id;
    let sink_clone = sink.clone();
    tokio::task::spawn_blocking(move || {
        if let Some((rgba, width, height)) = imaging::thumbnail(&thumbnail_source, 256) {
            sink_clone.send(UiEvent::ThumbnailReady {
                id,
                rgba,
                width,
                height,
            });
        }
    });

    Ok(TaskOutcome {
        path: path_string,
        bytes,
        format,
        skipped: false,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResumeState {
    url: String,
    total: u64,
    etag: Option<String>,
    last_modified: Option<String>,
}

async fn run_video(
    client: &reqwest::Client,
    item: &ImageItem,
    settings: &Settings,
    limiter: &Arc<BandwidthLimiter>,
    reserved: &ReservedPaths,
    sink: &EventSink,
) -> Result<TaskOutcome, DownloadError> {
    let directory = storage::output_dir(&settings.download_root, &item.page_title);
    std::fs::create_dir_all(&directory).map_err(|e| DownloadError::Io(e.to_string()))?;
    let hash = media_hash(&item.url).unwrap_or_else(|| format!("{:03}", item.seq));
    let stem = storage::file_stem(item.seq, item.kind.slug(), &hash);
    let extension = video_extension(&item.url);
    let Some(path) = find_resumable_video(&directory, &stem, extension, item, reserved)
        .or_else(|| reserve(&directory, &stem, extension, settings, reserved))
    else {
        return Ok(skipped());
    };
    let part = path.with_extension("part");
    let state_path = path.with_extension("part.json");

    let mut offset = resumable_offset(&part, &state_path, item);
    let mut response = request_video(client, item, offset).await?;
    if response.status().as_u16() == 416 && offset > 0 {
        reset_part(&part, &state_path);
        offset = 0;
        response = request_video(client, item, 0).await?;
    }
    if response.status().as_u16() == 401 || response.status().as_u16() == 403 {
        return Err(DownloadError::Verification);
    }
    if response.status().as_u16() == 206
        && offset > 0
        && !resume_compatible(&response, &state_path, item, offset)
    {
        reset_part(&part, &state_path);
        offset = 0;
        response = request_video(client, item, 0).await?;
    }

    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err(DownloadError::Verification);
    }
    if !status.is_success() {
        return Err(DownloadError::Status(status.as_u16()));
    }
    let content_type = header_value(&response, reqwest::header::CONTENT_TYPE).unwrap_or_default();
    if content_type.starts_with("text/html") {
        return Err(DownloadError::Verification);
    }
    if status.as_u16() == 206
        && offset == 0
        && let Some((start, _)) = content_range_parts(&response)
        && start != 0
    {
        return Err(DownloadError::Status(206));
    }
    let resume_mode = status.as_u16() == 206 && offset > 0;
    let total = if resume_mode {
        content_range_total(&response)
            .unwrap_or_else(|| offset + response.content_length().unwrap_or(0))
    } else {
        response.content_length().unwrap_or(0)
    };
    if total > MAX_VIDEO_BYTES {
        return Err(DownloadError::TooLarge);
    }

    let state = ResumeState {
        url: item.url.clone(),
        total,
        etag: header_value(&response, reqwest::header::ETAG),
        last_modified: header_value(&response, reqwest::header::LAST_MODIFIED),
    };
    if let Ok(bytes) = serde_json::to_vec(&state) {
        let _ = std::fs::write(&state_path, bytes);
    }

    let mut file = if resume_mode {
        std::fs::OpenOptions::new()
            .append(true)
            .open(&part)
            .map_err(|e| DownloadError::Io(e.to_string()))?
    } else {
        offset = 0;
        std::fs::File::create(&part).map_err(|e| DownloadError::Io(e.to_string()))?
    };
    let bytes = match write_video(response, &mut file, item.id, offset, total, limiter, sink).await
    {
        Ok(bytes) => bytes,
        // 失败时保留 .part 与续传元数据，下次重试继续。
        Err(err) => {
            drop(file);
            return Err(err);
        }
    };
    if total > 0 && bytes != total {
        // 连接提前结束但无错误：保留 .part，交给下一次重试续传。
        drop(file);
        return Err(DownloadError::Network(format!(
            "下载不完整：{bytes}/{total} 字节"
        )));
    }
    file.flush().map_err(|e| DownloadError::Io(e.to_string()))?;
    drop(file);
    std::fs::rename(&part, &path).map_err(|e| DownloadError::Io(e.to_string()))?;
    let _ = std::fs::remove_file(&state_path);

    let path_string = path.to_string_lossy().into_owned();
    let format = extension.to_ascii_uppercase();
    emit_completed(item, &path_string, bytes, &format, sink);
    Ok(TaskOutcome {
        path: path_string,
        bytes,
        format,
        skipped: false,
    })
}

fn reserve(
    directory: &Path,
    stem: &str,
    extension: &str,
    settings: &Settings,
    reserved: &ReservedPaths,
) -> Option<PathBuf> {
    let mut guard = reserved.lock().unwrap_or_else(|error| error.into_inner());
    storage::reserve_path(
        directory,
        stem,
        extension,
        settings.duplicate_policy,
        &mut guard,
    )
}

fn skipped() -> TaskOutcome {
    TaskOutcome {
        path: String::new(),
        bytes: 0,
        format: String::new(),
        skipped: true,
    }
}

fn file_matches(path: &Path, expected_len: u64, expected: &[u8]) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if metadata.len() != expected_len {
        return false;
    }
    if !imaging::has_known_magic(expected) {
        return true;
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut head = [0u8; MAGIC_HEAD_BYTES];
    let Ok(read) = file.read(&mut head) else {
        return false;
    };
    imaging::has_known_magic(&head[..read])
}

fn emit_completed(item: &ImageItem, path: &str, bytes: u64, format: &str, sink: &EventSink) {
    sink.send(UiEvent::ItemCompleted {
        id: item.id,
        path: path.to_string(),
        bytes,
        format: format.to_string(),
    });
    sink.send(UiEvent::HistoryAppended(HistoryEntry {
        page_url: item.page_url.clone(),
        page_title: item.page_title.clone(),
        image_url: item.url.clone(),
        kind: item.kind,
        path: path.to_string(),
        bytes,
        format: format.to_string(),
        finished_at: chrono::Local::now().to_rfc3339(),
    }));
}

/// 重试时优先续用上一轮的 `.part`，避免冲突策略把续传路径改成 `_1` 后从头下载。
fn find_resumable_video(
    directory: &Path,
    stem: &str,
    extension: &str,
    item: &ImageItem,
    reserved: &ReservedPaths,
) -> Option<PathBuf> {
    let mut candidates = Vec::with_capacity(101);
    candidates.push(directory.join(format!("{stem}.{extension}")));
    for index in 1..=100 {
        candidates.push(directory.join(format!("{stem}_{index}.{extension}")));
    }
    for path in candidates {
        if path.exists() {
            continue;
        }
        let part = path.with_extension("part");
        if !part.is_file() {
            continue;
        }
        let state_path = path.with_extension("part.json");
        let Ok(bytes) = std::fs::read(&state_path) else {
            continue;
        };
        let Ok(state) = serde_json::from_slice::<ResumeState>(&bytes) else {
            continue;
        };
        if state.url != item.url {
            continue;
        }
        if let Ok(mut guard) = reserved.lock() {
            guard.insert(storage::path_key(&path));
        }
        return Some(path);
    }
    None
}

fn resumable_offset(part: &Path, state_path: &Path, item: &ImageItem) -> u64 {
    let Ok(metadata) = std::fs::metadata(part) else {
        return 0;
    };
    if metadata.len() == 0 {
        return 0;
    }
    let Ok(bytes) = std::fs::read(state_path) else {
        let _ = std::fs::remove_file(part);
        return 0;
    };
    match serde_json::from_slice::<ResumeState>(&bytes) {
        Ok(state) if state.url == item.url => metadata.len(),
        _ => {
            reset_part(part, state_path);
            0
        }
    }
}

async fn request_video(
    client: &reqwest::Client,
    item: &ImageItem,
    offset: u64,
) -> Result<reqwest::Response, DownloadError> {
    let mut request = client
        .get(&item.url)
        .header(reqwest::header::REFERER, REFERER);
    if offset > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
    }
    request
        .send()
        .await
        .map_err(|e| DownloadError::Network(e.to_string()))
}

fn resume_compatible(
    response: &reqwest::Response,
    state_path: &Path,
    item: &ImageItem,
    offset: u64,
) -> bool {
    if let Some((start, _)) = content_range_parts(response)
        && start != offset
    {
        return false;
    }
    let Ok(bytes) = std::fs::read(state_path) else {
        return false;
    };
    let Ok(state) = serde_json::from_slice::<ResumeState>(&bytes) else {
        return false;
    };
    if state.url != item.url {
        return false;
    }
    if let Some((_, Some(total))) = content_range_parts(response)
        && state.total > 0
        && total != state.total
    {
        return false;
    }
    let etag = header_value(response, reqwest::header::ETAG);
    let last_modified = header_value(response, reqwest::header::LAST_MODIFIED);
    if let (Some(saved), Some(current)) = (state.etag, etag)
        && saved != current
    {
        return false;
    }
    if let (Some(saved), Some(current)) = (state.last_modified, last_modified)
        && saved != current
    {
        return false;
    }
    true
}

fn reset_part(part: &Path, state_path: &Path) {
    let _ = std::fs::remove_file(part);
    let _ = std::fs::remove_file(state_path);
}

fn header_value(response: &reqwest::Response, name: reqwest::header::HeaderName) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn content_range_parts(response: &reqwest::Response) -> Option<(u64, Option<u64>)> {
    parse_content_range(&header_value(response, reqwest::header::CONTENT_RANGE)?)
}

fn content_range_total(response: &reqwest::Response) -> Option<u64> {
    content_range_parts(response).and_then(|(_, total)| total)
}

fn parse_content_range(value: &str) -> Option<(u64, Option<u64>)> {
    let (range, total) = value.split_once('/')?;
    let start = range
        .trim()
        .strip_prefix("bytes")?
        .trim()
        .split('-')
        .next()?
        .trim()
        .parse()
        .ok()?;
    let total = total.trim().parse().ok();
    Some((start, total))
}

async fn write_video(
    response: reqwest::Response,
    file: &mut std::fs::File,
    id: u64,
    start: u64,
    total: u64,
    limiter: &Arc<BandwidthLimiter>,
    sink: &EventSink,
) -> Result<u64, DownloadError> {
    let mut stream = response.bytes_stream();
    let mut written = start;
    let mut last_emit = Instant::now();
    loop {
        let next = tokio::time::timeout(Duration::from_secs(60), stream.next())
            .await
            .map_err(|_| DownloadError::Timeout)?;
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk.map_err(|e| DownloadError::Network(e.to_string()))?;
        limiter.acquire(chunk.len() as u64).await;
        file.write_all(&chunk)
            .map_err(|e| DownloadError::Io(e.to_string()))?;
        written += chunk.len() as u64;
        if written > MAX_VIDEO_BYTES {
            return Err(DownloadError::TooLarge);
        }
        if last_emit.elapsed() >= Duration::from_millis(200) {
            sink.send(UiEvent::ItemProgress {
                id,
                done: written,
                total,
            });
            last_emit = Instant::now();
        }
    }
    sink.send(UiEvent::ItemProgress {
        id,
        done: written,
        total,
    });
    Ok(written)
}

fn video_extension(url: &str) -> &'static str {
    crate::extract::video::extension_of(url).unwrap_or("mp4")
}

async fn download(
    client: &reqwest::Client,
    item: &ImageItem,
    limiter: &Arc<BandwidthLimiter>,
    sink: &EventSink,
) -> Result<Vec<u8>, DownloadError> {
    let response = client
        .get(&item.url)
        .header(reqwest::header::REFERER, REFERER)
        .send()
        .await
        .map_err(|e| DownloadError::Network(e.to_string()))?;
    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err(DownloadError::Verification);
    }
    if !status.is_success() {
        return Err(DownloadError::Status(status.as_u16()));
    }
    let is_html = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/html"));
    if is_html {
        return Err(DownloadError::Verification);
    }
    let total = response.content_length().unwrap_or(0);
    let mut stream = response.bytes_stream();
    let mut buffer: Vec<u8> = Vec::with_capacity(total.min(MAX_IMAGE_BYTES) as usize);
    let mut last_emit = Instant::now();
    loop {
        let chunk = tokio::time::timeout(Duration::from_secs(20), stream.next())
            .await
            .map_err(|_| DownloadError::Timeout)?;
        let Some(chunk) = chunk else {
            break;
        };
        let chunk = chunk.map_err(|e| DownloadError::Network(e.to_string()))?;
        limiter.acquire(chunk.len() as u64).await;
        buffer.extend_from_slice(&chunk);
        if buffer.len() as u64 > MAX_IMAGE_BYTES {
            return Err(DownloadError::TooLarge);
        }
        if last_emit.elapsed() >= Duration::from_millis(100) {
            sink.send(UiEvent::ItemProgress {
                id: item.id,
                done: buffer.len() as u64,
                total,
            });
            last_emit = Instant::now();
        }
    }
    sink.send(UiEvent::ItemProgress {
        id: item.id,
        done: buffer.len() as u64,
        total,
    });
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_range() {
        assert_eq!(
            parse_content_range("bytes 100-199/1000"),
            Some((100, Some(1000)))
        );
        assert_eq!(parse_content_range("bytes 0-99/*"), Some((0, None)));
        assert_eq!(parse_content_range("bytes */1000"), None);
        assert_eq!(parse_content_range("garbage"), None);
    }
}
