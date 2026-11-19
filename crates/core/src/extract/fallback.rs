use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::normalize::{dedup_key, format_hint, normalize_image_url};

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"https?:(?:\\?/){2}(?:mmbiz\.qpic\.cn|(?:wx|thirdwx|mmbiz)\.qlogo\.cn)[^"'\s<>()\[\]]*"#,
    )
    .unwrap()
});

pub struct RawImage {
    pub url: String,
    pub hint: Option<String>,
}

pub fn scan(html: &str) -> Vec<RawImage> {
    let mut seen = HashSet::new();
    let mut images = Vec::new();
    for matched in URL_RE.find_iter(html) {
        let raw = matched.as_str().trim_end_matches('\\');
        let Some(url) = normalize_image_url(raw) else {
            continue;
        };
        let Some(key) = dedup_key(&url) else {
            continue;
        };
        if seen.insert(key) {
            images.push(RawImage {
                url,
                hint: format_hint(raw),
            });
        }
    }
    images
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_plain_and_escaped_urls() {
        let html = r#"
            <img data-src="https://mmbiz.qpic.cn/mmbiz_jpg/PlainHash/640?wx_fmt=jpeg">
            <script>var pic = "https:\/\/mmbiz.qpic.cn\/mmbiz_png\/EscapedHash\/640?wx_fmt=png\u0026tp=webp";</script>
        "#;
        let images = scan(html);
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].url, "https://mmbiz.qpic.cn/mmbiz_jpg/PlainHash/0");
        assert_eq!(
            images[1].url,
            "https://mmbiz.qpic.cn/mmbiz_png/EscapedHash/0"
        );
        assert_eq!(images[1].hint.as_deref(), Some("png"));
    }

    #[test]
    fn deduplicates_size_variants_and_covers_qlogo() {
        let html = r#"
            https://mmbiz.qpic.cn/mmbiz_jpg/SameHash/640?wx_fmt=jpeg
            https://mmbiz.qpic.cn/mmbiz_jpg/SameHash/0
            https://wx.qlogo.cn/mmopen/AvatarHash/132
            https://res.wx.qq.com/mp/res/icon.png
        "#;
        let images = scan(html);
        assert_eq!(images.len(), 2);
        assert!(images[1].url.contains("wx.qlogo.cn"));
    }
}
