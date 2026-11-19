use std::time::Duration;

use futures_util::StreamExt;
use url::Url;

use crate::fetch::REFERER;
use crate::imaging;
use crate::model::{ImageItem, UiEvent};

use super::EventSink;

const PREVIEW_MAX_BYTES: u64 = 12 * 1024 * 1024;
const FULL_MAX_BYTES: u64 = 64 * 1024 * 1024;
const PREVIEW_IDLE_TIMEOUT: Duration = Duration::from_secs(15);
const THUMB_SIDE: u32 = 256;
const FULL_SIDE: u32 = 1600;

pub async fn fetch(client: &reqwest::Client, item: &ImageItem, sink: &EventSink) {
    if item.kind.is_video() {
        return;
    }
    for url in candidates(&item.url) {
        let Some(bytes) = download(client, &url, PREVIEW_MAX_BYTES).await else {
            continue;
        };
        let Ok(Some((rgba, width, height))) =
            tokio::task::spawn_blocking(move || imaging::thumbnail(&bytes, THUMB_SIDE)).await
        else {
            continue;
        };
        sink.send(UiEvent::ThumbnailReady {
            id: item.id,
            rgba,
            width,
            height,
        });
        return;
    }
}

pub async fn full(
    client: &reqwest::Client,
    id: u64,
    url: &str,
    path: Option<String>,
    sink: &EventSink,
) {
    let local = match path {
        Some(path) => tokio::task::spawn_blocking(move || std::fs::read(path))
            .await
            .ok()
            .and_then(Result::ok),
        None => None,
    };
    let bytes = match local {
        Some(bytes) => Some(bytes),
        None => download(client, url, FULL_MAX_BYTES).await,
    };
    let Some(bytes) = bytes else {
        sink.send(UiEvent::FullImageFailed {
            id,
            message: "无法读取原图，可能已被删除或需要验证".to_string(),
        });
        return;
    };
    let decoded = tokio::task::spawn_blocking(move || imaging::thumbnail(&bytes, FULL_SIDE)).await;
    match decoded {
        Ok(Some((rgba, width, height))) => sink.send(UiEvent::FullImageReady {
            id,
            rgba,
            width,
            height,
        }),
        _ => sink.send(UiEvent::FullImageFailed {
            id,
            message: "图片解码失败，暂不支持该格式".to_string(),
        }),
    }
}

fn candidates(normalized: &str) -> Vec<String> {
    let Ok(url) = Url::parse(normalized) else {
        return Vec::new();
    };
    let host = url.host_str().unwrap_or_default();
    let size = if host.ends_with("qlogo.cn") {
        "132"
    } else if host.ends_with("qpic.cn") {
        "640"
    } else {
        return vec![normalized.to_string()];
    };
    let Some(mut segments) = url
        .path_segments()
        .map(|s| s.map(str::to_string).collect::<Vec<_>>())
    else {
        return vec![normalized.to_string()];
    };
    let Some(last) = segments.last_mut() else {
        return vec![normalized.to_string()];
    };
    if last.is_empty() || !last.chars().all(|c| c.is_ascii_digit()) {
        return vec![normalized.to_string()];
    }
    *last = size.to_string();
    let mut small = url.clone();
    small.set_path(&format!("/{}", segments.join("/")));
    vec![small.to_string(), normalized.to_string()]
}

async fn download(client: &reqwest::Client, url: &str, max_bytes: u64) -> Option<Vec<u8>> {
    let response = client
        .get(url)
        .header(reqwest::header::REFERER, REFERER)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let is_html = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/html"));
    if is_html {
        return None;
    }
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes)
    {
        return None;
    }
    let mut stream = response.bytes_stream();
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        let next = tokio::time::timeout(PREVIEW_IDLE_TIMEOUT, stream.next())
            .await
            .ok()?;
        let Some(chunk) = next else {
            break;
        };
        buffer.extend_from_slice(&chunk.ok()?);
        if buffer.len() as u64 > max_bytes {
            return None;
        }
    }
    if buffer.is_empty() {
        None
    } else {
        Some(buffer)
    }
}
