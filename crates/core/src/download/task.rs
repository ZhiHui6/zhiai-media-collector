use std::io::Write;
use std::time::{Duration, Instant};

use futures_util::StreamExt;

use crate::error::DownloadError;
use crate::extract::normalize::media_hash;
use crate::fetch::REFERER;
use crate::imaging;
use crate::model::{HistoryEntry, ImageItem, Settings, UiEvent};
use crate::storage;

use super::EventSink;

pub const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_VIDEO_BYTES: u64 = 4 * 1024 * 1024 * 1024;

pub struct TaskOutcome {
    pub path: String,
    pub bytes: u64,
    pub format: String,
}

pub async fn run(
    client: &reqwest::Client,
    item: &ImageItem,
    settings: &Settings,
    sink: &EventSink,
) -> Result<TaskOutcome, DownloadError> {
    if item.kind.is_video() {
        return run_video(client, item, settings, sink).await;
    }
    let payload = download(client, item, sink).await?;
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
    let file_name = storage::file_name(item.seq, item.kind.slug(), &hash, &processed.extension);
    let path = directory.join(file_name);

    let already_present = std::fs::metadata(&path)
        .map(|meta| meta.len() == processed.bytes.len() as u64)
        .unwrap_or(false);
    if !already_present {
        let part = path.with_extension("part");
        std::fs::write(&part, &processed.bytes).map_err(|e| DownloadError::Io(e.to_string()))?;
        std::fs::rename(&part, &path).map_err(|e| DownloadError::Io(e.to_string()))?;
    }

    let bytes = processed.bytes.len() as u64;
    let path_string = path.to_string_lossy().into_owned();
    let format = processed.format;
    sink.send(UiEvent::ItemCompleted {
        id: item.id,
        path: path_string.clone(),
        bytes,
        format: format.clone(),
    });
    sink.send(UiEvent::HistoryAppended(HistoryEntry {
        page_url: item.page_url.clone(),
        page_title: item.page_title.clone(),
        image_url: item.url.clone(),
        kind: item.kind,
        path: path_string.clone(),
        bytes,
        format: format.clone(),
        finished_at: chrono::Local::now().to_rfc3339(),
    }));

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
    })
}

async fn run_video(
    client: &reqwest::Client,
    item: &ImageItem,
    settings: &Settings,
    sink: &EventSink,
) -> Result<TaskOutcome, DownloadError> {
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
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    if content_type.starts_with("text/html") {
        return Err(DownloadError::Verification);
    }
    let total = response.content_length().unwrap_or(0);
    if total > MAX_VIDEO_BYTES {
        return Err(DownloadError::TooLarge);
    }
    let extension = video_extension(&item.url, &content_type);
    let directory = storage::output_dir(&settings.download_root, &item.page_title);
    std::fs::create_dir_all(&directory).map_err(|e| DownloadError::Io(e.to_string()))?;
    let hash = media_hash(&item.url).unwrap_or_else(|| format!("{:03}", item.seq));
    let file_name = storage::file_name(item.seq, item.kind.slug(), &hash, extension);
    let path = directory.join(file_name);
    let part = path.with_extension("part");
    let mut file = std::fs::File::create(&part).map_err(|e| DownloadError::Io(e.to_string()))?;
    let bytes = match write_video(response, &mut file, item.id, total, sink).await {
        Ok(bytes) => bytes,
        Err(err) => {
            drop(file);
            let _ = std::fs::remove_file(&part);
            return Err(err);
        }
    };
    file.flush().map_err(|e| DownloadError::Io(e.to_string()))?;
    drop(file);
    std::fs::rename(&part, &path).map_err(|e| DownloadError::Io(e.to_string()))?;

    let path_string = path.to_string_lossy().into_owned();
    let format = extension.to_ascii_uppercase();
    sink.send(UiEvent::ItemCompleted {
        id: item.id,
        path: path_string.clone(),
        bytes,
        format: format.clone(),
    });
    sink.send(UiEvent::HistoryAppended(HistoryEntry {
        page_url: item.page_url.clone(),
        page_title: item.page_title.clone(),
        image_url: item.url.clone(),
        kind: item.kind,
        path: path_string.clone(),
        bytes,
        format: format.clone(),
        finished_at: chrono::Local::now().to_rfc3339(),
    }));

    Ok(TaskOutcome {
        path: path_string,
        bytes,
        format,
    })
}

async fn write_video(
    response: reqwest::Response,
    file: &mut std::fs::File,
    id: u64,
    total: u64,
    sink: &EventSink,
) -> Result<u64, DownloadError> {
    let mut stream = response.bytes_stream();
    let mut written: u64 = 0;
    let mut last_emit = Instant::now();
    loop {
        let next = tokio::time::timeout(Duration::from_secs(60), stream.next())
            .await
            .map_err(|_| DownloadError::Timeout)?;
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk.map_err(|e| DownloadError::Network(e.to_string()))?;
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

fn video_extension(url: &str, content_type: &str) -> &'static str {
    if content_type.contains("webm") {
        "webm"
    } else if content_type.contains("quicktime") {
        "mov"
    } else if content_type.contains("mp4") {
        "mp4"
    } else {
        crate::extract::video::extension_of(url).unwrap_or("mp4")
    }
}

async fn download(
    client: &reqwest::Client,
    item: &ImageItem,
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
