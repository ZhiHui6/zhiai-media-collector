use url::Url;

const IMAGE_HOSTS: [&str; 4] = [
    "mmbiz.qpic.cn",
    "wx.qlogo.cn",
    "thirdwx.qlogo.cn",
    "mmbiz.qlogo.cn",
];

pub fn unescape(raw: &str) -> String {
    raw.replace("\\/", "/")
        .replace("\\u0026", "&")
        .replace("\\u003D", "=")
        .replace("\\u003d", "=")
        .replace("&amp;", "&")
}

pub fn normalize_image_url(raw: &str) -> Option<String> {
    let unescaped = unescape(raw);
    let mut url = Url::parse(&unescaped).ok()?;
    let host = url.host_str()?.to_string();
    if !IMAGE_HOSTS.contains(&host.as_str()) {
        return None;
    }
    url.set_scheme("https").ok()?;
    let segments: Vec<String> = url
        .path_segments()
        .map(|s| s.map(str::to_string).collect())
        .unwrap_or_default();
    if segments.len() < 2 {
        return None;
    }
    let last = segments.last()?;
    if !last.is_empty() && last.chars().all(|c| c.is_ascii_digit()) {
        let mut rewritten = segments.clone();
        *rewritten.last_mut()? = "0".to_string();
        url.set_path(&format!("/{}", rewritten.join("/")));
    }
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
}

pub fn dedup_key(normalized: &str) -> Option<String> {
    let url = Url::parse(normalized).ok()?;
    Some(format!("{}{}", url.host_str()?, url.path()))
}

pub fn format_hint(raw: &str) -> Option<String> {
    let unescaped = unescape(raw);
    let url = Url::parse(&unescaped).ok()?;
    for (key, value) in url.query_pairs() {
        if key == "wx_fmt" && !value.is_empty() {
            return Some(canonical_format(&value));
        }
    }
    let first = url.path_segments()?.next()?;
    let suffix = first.rsplit("mmbiz_").next()?;
    if suffix == first {
        return None;
    }
    Some(canonical_format(suffix))
}

fn canonical_format(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "jpeg".to_string(),
        other => other.to_string(),
    }
}

pub fn media_hash(normalized: &str) -> Option<String> {
    let url = Url::parse(normalized).ok()?;
    let segments: Vec<&str> = url.path_segments()?.collect();
    let candidate = segments
        .iter()
        .rev()
        .find(|s| !s.is_empty() && !s.chars().all(|c| c.is_ascii_digit()))?;
    let cleaned: String = candidate
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    Some(cleaned.chars().take(12).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescapes_json_and_html() {
        assert_eq!(
            unescape("https:\\/\\/mmbiz.qpic.cn\\/mmbiz_jpg\\/abc\\/640?wx_fmt=jpeg\\u0026tp=webp"),
            "https://mmbiz.qpic.cn/mmbiz_jpg/abc/640?wx_fmt=jpeg&tp=webp"
        );
        assert_eq!(unescape("a=1&amp;b=2"), "a=1&b=2");
    }

    #[test]
    fn rewrites_size_to_original_and_strips_query() {
        let normalized = normalize_image_url(
            "https://mmbiz.qpic.cn/mmbiz_jpg/AbC123/640?wx_fmt=jpeg&tp=webp&wxfrom=5&wx_lazy=1",
        )
        .unwrap();
        assert_eq!(normalized, "https://mmbiz.qpic.cn/mmbiz_jpg/AbC123/0");
    }

    #[test]
    fn upgrades_http_and_handles_sz_prefix() {
        let normalized =
            normalize_image_url("http://mmbiz.qpic.cn/sz_mmbiz_png/XyZ/320?wx_fmt=png").unwrap();
        assert_eq!(normalized, "https://mmbiz.qpic.cn/sz_mmbiz_png/XyZ/0");
    }

    #[test]
    fn normalizes_qlogo_avatar() {
        let normalized = normalize_image_url("https://wx.qlogo.cn/mmopen/vi_32/AbC/132").unwrap();
        assert_eq!(normalized, "https://wx.qlogo.cn/mmopen/vi_32/AbC/0");
    }

    #[test]
    fn rejects_foreign_hosts() {
        assert!(normalize_image_url("https://res.wx.qq.com/mp/res/icon.png").is_none());
        assert!(normalize_image_url("https://example.com/a/640").is_none());
    }

    #[test]
    fn dedup_merges_size_variants() {
        let a = normalize_image_url("https://mmbiz.qpic.cn/mmbiz_jpg/AbC/640?wx_fmt=jpeg").unwrap();
        let b = normalize_image_url("https://mmbiz.qpic.cn/mmbiz_jpg/AbC/0").unwrap();
        assert_eq!(dedup_key(&a), dedup_key(&b));
    }

    #[test]
    fn format_hint_from_query_and_path() {
        assert_eq!(
            format_hint("https://mmbiz.qpic.cn/mmbiz/AbC/640?wx_fmt=jpeg").as_deref(),
            Some("jpeg")
        );
        assert_eq!(
            format_hint("https://mmbiz.qpic.cn/mmbiz_png/AbC/640").as_deref(),
            Some("png")
        );
        assert_eq!(
            format_hint("https://mmbiz.qpic.cn/sz_mmbiz_gif/AbC/640").as_deref(),
            Some("gif")
        );
        assert_eq!(
            format_hint("https://mmbiz.qpic.cn/mmbiz/AbC/640").as_deref(),
            None
        );
    }

    #[test]
    fn media_hash_skips_numeric_segments() {
        let normalized =
            normalize_image_url("https://mmbiz.qpic.cn/mmbiz_jpg/AbC123defGhi/640").unwrap();
        assert_eq!(media_hash(&normalized).as_deref(), Some("AbC123defGhi"));
    }
}
