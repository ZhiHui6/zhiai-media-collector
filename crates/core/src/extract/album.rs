use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

use crate::error::FetchError;
use crate::fetch;
use crate::model::ParseOutcome;

const MAX_PAGES: usize = 50;
const PAGE_SIZE: usize = 10;

static BIZ_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"__biz\s*=\s*["']([^"']+)["']"#).unwrap());
static ALBUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)album_?id\s*=\s*["']?(\d{5,})["']?"#).unwrap());
static ARTICLE_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"https?:(?:\\?/){2}mp\.weixin\.qq\.com\\?/(?:s\\?/[^"'\s<>()\[\]]*|s\?[^"'\s<>()\[\]]*)"#,
    )
    .unwrap()
});

pub async fn parse(client: &reqwest::Client, url: &Url, raw_url: &str) -> ParseOutcome {
    let html = match fetch::fetch_html(client, url.as_str()).await {
        Ok(html) => html,
        Err(err) => {
            return ParseOutcome::Failed {
                url: raw_url.to_string(),
                message: err.to_string(),
            };
        }
    };
    let title = album_title(&html);
    let biz = query_value(url, "__biz").or_else(|| capture(&BIZ_RE, &html));
    let album_id = query_value(url, "album_id").or_else(|| capture(&ALBUM_RE, &html));

    let mut links = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut api_error = None;
    if let (Some(biz), Some(album_id)) = (biz, album_id) {
        match collect_articles(client, url, &biz, &album_id).await {
            Ok(articles) => {
                for link in articles {
                    if seen.insert(link.clone()) {
                        links.push(link);
                    }
                }
            }
            Err(err) => api_error = Some(err),
        }
    }

    if links.is_empty() {
        for link in html_article_links(&html) {
            if seen.insert(link.clone()) {
                links.push(link);
            }
        }
    }

    if links.is_empty() {
        let message = match api_error {
            Some(err) => format!("合集接口受限（{err}），请在浏览器打开合集页手动复制文章链接"),
            None => "未能在合集中找到文章链接，请打开合集页手动复制".to_string(),
        };
        return ParseOutcome::Failed {
            url: raw_url.to_string(),
            message,
        };
    }
    ParseOutcome::ArticleLinks {
        page_url: raw_url.to_string(),
        title,
        links,
    }
}

async fn collect_articles(
    client: &reqwest::Client,
    base: &Url,
    biz: &str,
    album_id: &str,
) -> Result<Vec<String>, FetchError> {
    let mut articles = Vec::new();
    let mut begin_msgid: Option<String> = None;
    let mut begin_itemidx: Option<String> = None;
    for page in 0..MAX_PAGES {
        let mut request = base.clone();
        request.set_path("/mp/appmsgalbum");
        request.set_fragment(None);
        request
            .query_pairs_mut()
            .clear()
            .append_pair("action", "getalbum")
            .append_pair("__biz", biz)
            .append_pair("album_id", album_id)
            .append_pair("count", &PAGE_SIZE.to_string())
            .append_pair("is_reverse", "1")
            .append_pair("f", "json");
        if let Some(msgid) = &begin_msgid {
            request.query_pairs_mut().append_pair("begin_msgid", msgid);
        }
        if let Some(itemidx) = &begin_itemidx {
            request
                .query_pairs_mut()
                .append_pair("begin_itemidx", itemidx);
        }
        let body = fetch::fetch_html(client, request.as_str()).await?;
        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| FetchError::Network(format!("合集响应不是有效 JSON：{e}")))?;
        let ret = value
            .get("ret")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        if ret != 0 {
            let message = value
                .get("errmsg")
                .or_else(|| value.get("msg"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("接口返回错误");
            return Err(FetchError::Network(format!("{message} (ret={ret})")));
        }
        let response = value.get("getalbum_resp").unwrap_or(&value);
        let list = response
            .get("article_list")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if list.is_empty() {
            break;
        }
        for item in &list {
            let link = item
                .get("url")
                .or_else(|| item.get("link"))
                .or_else(|| item.get("content_url"))
                .and_then(serde_json::Value::as_str);
            if let Some(link) = link {
                articles.push(link.to_string());
            }
        }
        let last = list.last().expect("list is not empty");
        begin_msgid = json_string(last.get("msgid"));
        begin_itemidx = json_string(last.get("itemidx"));
        let continue_flag = response
            .get("continue_flag")
            .and_then(|value| json_string(Some(value)))
            .is_some_and(|flag| flag == "1");
        if !continue_flag || begin_msgid.is_none() {
            break;
        }
        if page + 1 < MAX_PAGES {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    Ok(articles)
}

fn json_string(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_i64().map(|number| number.to_string()))
        .or_else(|| value.as_u64().map(|number| number.to_string()))
}

fn query_value(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
}

fn capture(pattern: &Regex, haystack: &str) -> Option<String> {
    pattern
        .captures(haystack)
        .map(|captures| captures[1].to_string())
}

fn album_title(html: &str) -> String {
    let document = Html::parse_document(html);
    if let Ok(selector) = Selector::parse(r#"meta[property="og:title"]"#) {
        for element in document.select(&selector) {
            if let Some(content) = element.value().attr("content")
                && !content.trim().is_empty()
            {
                return content.trim().to_string();
            }
        }
    }
    "合集".to_string()
}

fn html_article_links(html: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let document = Html::parse_document(html);
    if let Ok(selector) = Selector::parse("a[href*='mp.weixin.qq.com/s']") {
        for element in document.select(&selector) {
            if let Some(href) = element.value().attr("href")
                && let Some(link) = normalize_article_link(href)
                && seen.insert(link.clone())
            {
                links.push(link);
            }
        }
    }
    for matched in ARTICLE_URL_RE.find_iter(html) {
        if let Some(link) = normalize_article_link(matched.as_str())
            && seen.insert(link.clone())
        {
            links.push(link);
        }
    }
    links
}

fn normalize_article_link(raw: &str) -> Option<String> {
    let cleaned = super::normalize::unescape(raw.trim_end_matches('\\'));
    let parsed = Url::parse(&cleaned).ok()?;
    if parsed.host_str() != Some("mp.weixin.qq.com") {
        return None;
    }
    if !parsed.path().starts_with("/s/") && parsed.path() != "/s" {
        return None;
    }
    Some(cleaned)
}
