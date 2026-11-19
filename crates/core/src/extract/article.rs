use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use scraper::{Html, Selector};

use crate::model::ImageKind;

use super::normalize::{dedup_key, format_hint, normalize_image_url};
use super::{ClassifiedImage, ExtractedPage};

static MSG_CDN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"msg_cdn_url\s*=\s*["']([^"']+)["']"#).unwrap());
static MSG_TITLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"msg_title\s*=\s*["']([^"']*)["']"#).unwrap());
static NICKNAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"nickname\s*=\s*["']([^"']*)["']"#).unwrap());
static BG_IMG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"background-image:\s*url\(\s*['"]?([^'")]+)"#).unwrap());

pub fn extract(doc: &Html, raw_html: &str) -> ExtractedPage {
    let title = extract_title(doc, raw_html);
    let account_name = extract_account(doc, raw_html);
    let mut images = Vec::new();
    let mut seen = HashSet::new();
    let mut cover_keys = HashSet::new();

    if let Ok(selector) = Selector::parse(r#"meta[property="og:image"]"#) {
        for element in doc.select(&selector) {
            if let Some(content) = element.value().attr("content")
                && let Some(key) = push_image(&mut images, &mut seen, content, ImageKind::Cover)
            {
                cover_keys.insert(key);
            }
        }
    }
    for captures in MSG_CDN_RE.captures_iter(raw_html) {
        if let Some(key) = push_image(&mut images, &mut seen, &captures[1], ImageKind::Cover) {
            cover_keys.insert(key);
        }
    }
    let mut body_ids = HashSet::new();
    if let Ok(selector) = Selector::parse("#js_content img") {
        for element in doc.select(&selector) {
            body_ids.insert(element.id());
            let attrs = element.value();
            let raw = attrs
                .attr("data-src")
                .or_else(|| attrs.attr("data-croporisrc"))
                .or_else(|| attrs.attr("src"));
            if let Some(raw) = raw {
                push_image(&mut images, &mut seen, raw, ImageKind::Article);
            }
        }
    }
    if let Ok(selector) = Selector::parse("#js_content [style]") {
        for element in doc.select(&selector) {
            if let Some(style) = element.value().attr("style") {
                for captures in BG_IMG_RE.captures_iter(style) {
                    push_image(&mut images, &mut seen, &captures[1], ImageKind::Article);
                }
            }
        }
    }

    if let Ok(selector) = Selector::parse("img") {
        for element in doc.select(&selector) {
            if body_ids.contains(&element.id()) {
                continue;
            }
            let attrs = element.value();
            let raw = attrs.attr("data-src").or_else(|| attrs.attr("src"));
            let Some(raw) = raw else {
                continue;
            };
            let Some(normalized) = normalize_image_url(raw) else {
                continue;
            };
            let is_avatar = url::Url::parse(&normalized)
                .ok()
                .and_then(|u| u.host_str().map(str::to_string))
                .is_some_and(|host| host.ends_with("qlogo.cn"));
            if is_avatar {
                push_image(&mut images, &mut seen, raw, ImageKind::Avatar);
            }
        }
    }

    for selector_src in [
        "#js_pc_qr_code img",
        "#js_profile_qrcode img",
        ".qr_code img",
        "img[src*='/mp/qrcode']",
    ] {
        if let Ok(selector) = Selector::parse(selector_src) {
            for element in doc.select(&selector) {
                let attrs = element.value();
                if let Some(raw) = attrs.attr("data-src").or_else(|| attrs.attr("src")) {
                    push_image(&mut images, &mut seen, raw, ImageKind::QrCode);
                }
            }
        }
    }

    ExtractedPage {
        title,
        account_name,
        images,
        cover_keys,
    }
}

fn push_image(
    images: &mut Vec<ClassifiedImage>,
    seen: &mut HashSet<String>,
    raw: &str,
    kind: ImageKind,
) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with("data:") {
        return None;
    }
    let is_qrcode_endpoint = trimmed.contains("mp.weixin.qq.com/mp/qrcode");
    let url = if is_qrcode_endpoint {
        trimmed.to_string()
    } else {
        normalize_image_url(trimmed)?
    };
    let key = if is_qrcode_endpoint {
        url.clone()
    } else {
        dedup_key(&url)?
    };
    if !seen.insert(key.clone()) {
        return None;
    }
    images.push(ClassifiedImage {
        url,
        hint: format_hint(trimmed),
        kind,
    });
    Some(key)
}

fn extract_title(doc: &Html, raw_html: &str) -> String {
    if let Ok(selector) = Selector::parse(r#"meta[property="og:title"]"#) {
        for element in doc.select(&selector) {
            if let Some(content) = element.value().attr("content") {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return decode_entities(trimmed);
                }
            }
        }
    }
    if let Ok(selector) = Selector::parse("#activity-name") {
        for element in doc.select(&selector) {
            let text: String = element.text().collect();
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    if let Some(captures) = MSG_TITLE_RE.captures(raw_html) {
        let decoded = decode_entities(&captures[1]);
        if !decoded.is_empty() {
            return decoded;
        }
    }
    "未命名文章".to_string()
}

fn extract_account(doc: &Html, raw_html: &str) -> String {
    if let Ok(selector) = Selector::parse("#js_name") {
        for element in doc.select(&selector) {
            let text: String = element.text().collect();
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    if let Some(captures) = NICKNAME_RE.captures(raw_html) {
        return decode_entities(&captures[1]);
    }
    String::new()
}

fn decode_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        <html><head>
        <meta property="og:title" content="测试文章标题">
        <meta property="og:image" content="https://mmbiz.qpic.cn/mmbiz_jpg/CoverHash/640?wx_fmt=jpeg">
        </head><body>
        <a id="js_name">测试公众号</a>
        <img src="https://wx.qlogo.cn/mmopen/AvatarHash/132">
        <div id="js_content">
            <p><img data-src="https://mmbiz.qpic.cn/mmbiz_png/BodyHash1/640?wx_fmt=png" src="data:image/png;base64,placeholder"></p>
            <section style="background-image: url('https://mmbiz.qpic.cn/mmbiz_jpg/BodyHash2/640?wx_fmt=jpeg')"></section>
            <img src="https://res.wx.qq.com/mp/res/icon.png">
        </div>
        <div id="js_pc_qr_code"><img src="https://mp.weixin.qq.com/mp/qrcode?scene=10000004&size=102&__biz=MzA1"></div>
        <script>var msg_cdn_url = "https://mmbiz.qpic.cn/mmbiz_jpg/CoverHash/0";</script>
        </body></html>
    "#;

    #[test]
    fn extracts_all_roles_with_dedup() {
        let doc = Html::parse_document(SAMPLE);
        let page = extract(&doc, SAMPLE);
        assert_eq!(page.title, "测试文章标题");
        assert_eq!(page.account_name, "测试公众号");
        assert_eq!(page.images.len(), 5);
        assert_eq!(page.images[0].kind, ImageKind::Cover);
        assert_eq!(page.images[1].kind, ImageKind::Article);
        assert_eq!(
            page.images[1].url,
            "https://mmbiz.qpic.cn/mmbiz_png/BodyHash1/0"
        );
        assert_eq!(page.images[2].kind, ImageKind::Article);
        assert_eq!(page.images[3].kind, ImageKind::Avatar);
        assert_eq!(page.images[4].kind, ImageKind::QrCode);
    }

    #[test]
    fn falls_back_to_js_title_and_nickname() {
        let html = r#"<script>var msg_title = "脚本标题"; var nickname = "脚本账号";</script>"#;
        let doc = Html::parse_document(html);
        let page = extract(&doc, html);
        assert_eq!(page.title, "脚本标题");
        assert_eq!(page.account_name, "脚本账号");
        assert!(page.images.is_empty());
    }
}
