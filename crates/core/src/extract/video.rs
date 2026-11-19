use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

use super::normalize::{dedup_key, unescape};

static URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https?:(?:\\?/){2}[^"'\s<>()\[\]]+"#).unwrap());

const VIDEO_EXTS: [&str; 4] = ["mp4", "m4v", "mov", "webm"];

pub struct RawVideo {
    pub url: String,
    pub hint: Option<String>,
}

/// 从页面 DOM 与原始 HTML 中提取视频直链（mp4 / mov / webm）。
pub fn scan(doc: &Html, html: &str) -> Vec<RawVideo> {
    let mut seen = HashSet::new();
    let mut videos = Vec::new();

    if let Ok(selector) = Selector::parse("video, video source, mpvideo, source") {
        for element in doc.select(&selector) {
            for attr in ["src", "data-src"] {
                let Some(value) = element.value().attr(attr) else {
                    continue;
                };
                push(&mut seen, &mut videos, value);
            }
        }
    }

    for matched in URL_RE.find_iter(html) {
        push(&mut seen, &mut videos, matched.as_str());
    }

    videos
}

/// 仅基于 HTML 文本扫描视频直链（用于内嵌播放页等二次请求结果）。
pub fn scan_html(html: &str) -> Vec<RawVideo> {
    let mut seen = HashSet::new();
    let mut videos = Vec::new();
    for matched in URL_RE.find_iter(html) {
        push(&mut seen, &mut videos, matched.as_str());
    }
    videos
}

/// 公众号内嵌视频播放页（iframe）地址，需二次请求才能拿到真实视频直链。
pub fn iframe_sources(doc: &Html) -> Vec<String> {
    let Ok(selector) = Selector::parse("iframe") else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    let mut sources = Vec::new();
    for element in doc.select(&selector) {
        let Some(value) = element.value().attr("data-src") else {
            continue;
        };
        if !(value.contains("videoplayer") || value.contains("get_mp_video_play_url")) {
            continue;
        }
        let unescaped = unescape(value);
        if seen.insert(unescaped.clone()) {
            sources.push(unescaped);
        }
    }
    sources
}

fn push(seen: &mut HashSet<String>, videos: &mut Vec<RawVideo>, raw: &str) {
    let trimmed = raw
        .trim()
        .trim_end_matches(['\\', ',', ';', '.', ')', ']', '\'']);
    let unescaped = unescape(trimmed);
    let Ok(url) = Url::parse(&unescaped) else {
        return;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return;
    }
    let Some(host) = url.host_str() else {
        return;
    };
    if host == "mmbiz.qpic.cn" || host.ends_with("qlogo.cn") {
        return;
    }
    let path = url.path().to_ascii_lowercase();
    let extension = VIDEO_EXTS
        .iter()
        .find(|ext| path.ends_with(&format!(".{ext}")));
    let contains_ext = VIDEO_EXTS
        .iter()
        .find(|ext| path.contains(&format!(".{ext}")));
    let looks_like_video = extension.is_some()
        || contains_ext.is_some()
        || host.contains("video")
        || path.contains("/mpvideo/")
        || path.contains("/videoplayer");
    if !looks_like_video || has_image_extension(&path) {
        return;
    }
    let normalized = url.to_string();
    let Some(key) = dedup_key(&normalized) else {
        return;
    };
    if seen.insert(key) {
        videos.push(RawVideo {
            url: normalized,
            hint: extension.or(contains_ext).map(|ext| (*ext).to_string()),
        });
    }
}

fn has_image_extension(path: &str) -> bool {
    [".jpg", ".jpeg", ".png", ".gif", ".webp", ".svg", ".ico"]
        .iter()
        .any(|ext| path.ends_with(ext))
}

pub fn extension_of(url: &str) -> Option<&'static str> {
    let path = Url::parse(url).ok()?.path().to_ascii_lowercase();
    VIDEO_EXTS
        .iter()
        .find(|ext| path.ends_with(&format!(".{ext}")))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_plain_and_escaped_mp4() {
        let html = r#"
            <video src="https://mpvideo.qpic.cn/abc_f10002.mp4?dis_k=1&amp;dis_t=2"></video>
            <script>var v = "https:\/\/mpvideo.qpic.cn\/def_f10002.mp4?dis_k=3\u0026dis_t=4";</script>
        "#;
        let doc = Html::parse_document(html);
        let videos = scan(&doc, html);
        assert_eq!(videos.len(), 2);
        assert!(videos[0].url.contains("abc_f10002.mp4"));
        assert_eq!(videos[1].hint.as_deref(), Some("mp4"));
    }

    #[test]
    fn ignores_images_and_pages() {
        let html = r#"
            <img src="https://mmbiz.qpic.cn/mmbiz_jpg/AbC/640?wx_fmt=jpeg">
            <video poster="https://mmbiz.qpic.cn/mmbiz_jpg/Cover/0"></video>
            <a href="https://mp.weixin.qq.com/s/abcdef">link</a>
        "#;
        let doc = Html::parse_document(html);
        assert!(scan(&doc, html).is_empty());
    }

    #[test]
    fn finds_player_iframes() {
        let html = r#"<iframe class="video_iframe" data-src="https://mp.weixin.qq.com/mp/videoplayer?action=get_mp_video_play_url&amp;vid=wxv_1"></iframe>"#;
        let doc = Html::parse_document(html);
        let sources = iframe_sources(&doc);
        assert_eq!(sources.len(), 1);
        assert!(sources[0].ends_with("vid=wxv_1"));
    }
}
