use url::Url;

use crate::error::LinkError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    Article,
    Album,
}

pub fn extract_urls(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut urls = Vec::new();
    for token in text.split(|c: char| {
        c.is_whitespace() || matches!(c, ',' | ';' | '，' | '；' | '、' | '"' | '\'' | '<' | '>')
    }) {
        let token = token.trim();
        if token.is_empty() || !token.contains("mp.weixin.qq.com") {
            continue;
        }
        let candidate = if token.starts_with("http://") || token.starts_with("https://") {
            token.to_string()
        } else {
            format!("https://{token}")
        };
        let Ok(parsed) = Url::parse(&candidate) else {
            continue;
        };
        if parsed.host_str() != Some("mp.weixin.qq.com") {
            continue;
        }
        if seen.insert(candidate.clone()) {
            urls.push(candidate);
        }
    }
    urls
}

pub fn classify(raw: &str) -> Result<(Url, LinkKind), LinkError> {
    let url = Url::parse(raw).map_err(|e| LinkError::Invalid(e.to_string()))?;
    if url.host_str() != Some("mp.weixin.qq.com") {
        return Err(LinkError::Unsupported);
    }
    let path = url.path();
    let kind = if path.starts_with("/s/") || path == "/s" {
        LinkKind::Article
    } else if path == "/mp/appmsgalbum" {
        LinkKind::Album
    } else {
        return Err(LinkError::Unsupported);
    };
    Ok((url, kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_multiline_mixed_input() {
        let text = "https://mp.weixin.qq.com/s/AbCdEf123\n看看这个 https://mp.weixin.qq.com/s?__biz=MzA1&mid=224748&idx=1&sn=abc，还有\nhttps://mp.weixin.qq.com/mp/appmsgalbum?__biz=MzA1&album_id=123";
        let urls = extract_urls(text);
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://mp.weixin.qq.com/s/AbCdEf123");
    }

    #[test]
    fn deduplicates_and_adds_scheme() {
        let text = "mp.weixin.qq.com/s/AbCdEf123 https://mp.weixin.qq.com/s/AbCdEf123";
        assert_eq!(extract_urls(text).len(), 1);
        assert_eq!(
            extract_urls(text)[0],
            "https://mp.weixin.qq.com/s/AbCdEf123"
        );
    }

    #[test]
    fn ignores_unrelated_text() {
        assert!(extract_urls("https://example.com/mp.weixin.qq.com/s/x 纯文本").is_empty());
    }

    #[test]
    fn classifies_article_short_and_long() {
        let (_, kind) = classify("https://mp.weixin.qq.com/s/AbCdEf123").unwrap();
        assert_eq!(kind, LinkKind::Article);
        let (_, kind) = classify("https://mp.weixin.qq.com/s?__biz=MzA1&mid=1&idx=1&sn=x").unwrap();
        assert_eq!(kind, LinkKind::Article);
    }

    #[test]
    fn classifies_album() {
        let (_, kind) =
            classify("https://mp.weixin.qq.com/mp/appmsgalbum?__biz=MzA1&album_id=123").unwrap();
        assert_eq!(kind, LinkKind::Album);
    }

    #[test]
    fn rejects_other_hosts_and_paths() {
        assert!(matches!(
            classify("https://example.com/s/x"),
            Err(LinkError::Unsupported)
        ));
        assert!(matches!(
            classify("https://mp.weixin.qq.com/cgi-bin/home"),
            Err(LinkError::Unsupported)
        ));
        assert!(matches!(classify("not a url"), Err(LinkError::Invalid(_))));
    }
}
