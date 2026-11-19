pub mod album;
pub mod article;
pub mod classify;
pub mod fallback;
pub mod normalize;
pub mod video;

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use scraper::{Html, Selector};

use crate::error::ExtractError;
use crate::fetch;
use crate::link::{self, LinkKind};
use crate::model::{ImageItem, ImageKind, PageKind, ParseBatch, ParseOutcome};

static NEXT_ITEM_ID: AtomicU64 = AtomicU64::new(1);

/// 恢复历史任务后抬高 ID 游标，避免重启后新解析的条目与旧任务撞号。
pub fn ensure_id_above(id: u64) {
    NEXT_ITEM_ID.fetch_max(id.saturating_add(1), Ordering::Relaxed);
}

#[derive(Clone)]
pub struct ClassifiedImage {
    pub url: String,
    pub hint: Option<String>,
    pub kind: ImageKind,
}

pub struct ExtractedPage {
    pub title: String,
    pub account_name: String,
    pub images: Vec<ClassifiedImage>,
    pub cover_keys: HashSet<String>,
}

pub fn detect_page_kind(doc: &Html, raw: &str) -> PageKind {
    if let Ok(selector) = Selector::parse("#js_content")
        && doc.select(&selector).next().is_some()
    {
        return PageKind::Article;
    }
    if [
        "picture_page_info",
        "js_picture",
        "share_picture",
        "msg_picture",
    ]
    .iter()
    .any(|marker| raw.contains(marker))
    {
        return PageKind::PicturePost;
    }
    if raw.contains("appmsgalbum") || raw.contains("album_id") {
        return PageKind::AlbumPage;
    }
    PageKind::Unknown
}

struct Analysis {
    page_kind: PageKind,
    structured: ExtractedPage,
    images: Vec<ClassifiedImage>,
    videos: Vec<video::RawVideo>,
    iframes: Vec<String>,
    warnings: Vec<String>,
    seen: HashSet<String>,
}

/// 同步完成所有 DOM 解析，避免把 `Html` 带进 async 状态机（其内部非 Send）。
fn analyze(html: &str) -> Analysis {
    let doc = Html::parse_document(html);
    let page_kind = detect_page_kind(&doc, html);
    let mut warnings = Vec::new();
    let structured = article::extract(&doc, html);
    let mut images = structured.images.clone();
    let mut seen: HashSet<String> = images
        .iter()
        .filter_map(|image| normalize::dedup_key(&image.url))
        .collect();

    let needs_fallback = images.is_empty() || page_kind != PageKind::Article;
    if needs_fallback {
        let scanned = fallback::scan(html);
        if !scanned.is_empty() {
            if page_kind == PageKind::Article {
                warnings.push("未识别到正文结构，已使用全页扫描，顺序可能不准确".to_string());
            } else if page_kind == PageKind::PicturePost {
                warnings.push("图片消息暂按全页扫描提取，顺序可能不准确".to_string());
            }
            for raw in scanned {
                let Some(key) = normalize::dedup_key(&raw.url) else {
                    continue;
                };
                if !seen.insert(key) {
                    continue;
                }
                let kind = classify::fallback_kind(&raw.url, page_kind, &structured.cover_keys);
                images.push(ClassifiedImage {
                    url: raw.url,
                    hint: raw.hint,
                    kind,
                });
            }
        }
    }

    let videos = video::scan(&doc, html);
    let mut iframes = video::iframe_sources(&doc);
    iframes.retain(|iframe| !videos.iter().any(|item| item.url == *iframe));

    Analysis {
        page_kind,
        structured,
        images,
        videos,
        iframes,
        warnings,
        seen,
    }
}

pub async fn parse_link(client: &reqwest::Client, raw_url: &str) -> ParseOutcome {
    let (url, kind) = match link::classify(raw_url) {
        Ok(parsed) => parsed,
        Err(err) => {
            return ParseOutcome::Failed {
                url: raw_url.to_string(),
                message: err.to_string(),
            };
        }
    };
    if kind == LinkKind::Album {
        return album::parse(client, &url, raw_url).await;
    }
    let html = match fetch::fetch_html(client, url.as_str()).await {
        Ok(html) => html,
        Err(err) => {
            return ParseOutcome::Failed {
                url: raw_url.to_string(),
                message: err.to_string(),
            };
        }
    };

    let mut analysis = analyze(&html);
    for iframe in analysis.iframes.iter().take(3) {
        let Ok(player) = fetch::fetch_html(client, iframe).await else {
            continue;
        };
        analysis.videos.extend(video::scan_html(&player));
    }
    if analysis.videos.is_empty() && !analysis.iframes.is_empty() {
        analysis
            .warnings
            .push("页面含内嵌视频，但未解析到可下载地址".to_string());
    }
    for raw in analysis.videos {
        let Some(key) = normalize::dedup_key(&raw.url) else {
            continue;
        };
        if !analysis.seen.insert(key) {
            continue;
        }
        analysis.images.push(ClassifiedImage {
            url: raw.url,
            hint: raw.hint,
            kind: ImageKind::Video,
        });
    }

    if analysis.images.is_empty() {
        return ParseOutcome::Failed {
            url: raw_url.to_string(),
            message: ExtractError::NoImages.to_string(),
        };
    }
    let items = analysis
        .images
        .into_iter()
        .enumerate()
        .map(|(index, image)| ImageItem {
            id: NEXT_ITEM_ID.fetch_add(1, Ordering::Relaxed),
            url: image.url,
            kind: image.kind,
            seq: index + 1,
            page_url: raw_url.to_string(),
            page_title: analysis.structured.title.clone(),
            format_hint: image.hint,
            selected: true,
        })
        .collect();
    ParseOutcome::Images(ParseBatch {
        page_url: raw_url.to_string(),
        page_title: analysis.structured.title,
        account_name: analysis.structured.account_name,
        page_kind: analysis.page_kind,
        items,
        warnings: analysis.warnings,
    })
}
