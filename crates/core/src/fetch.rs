use std::time::Duration;

use crate::error::FetchError;

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
pub const REFERER: &str = "https://mp.weixin.qq.com/";

pub fn build_client() -> Result<reqwest::Client, FetchError> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| FetchError::Network(e.to_string()))
}

pub fn is_verification_page(body: &str) -> bool {
    body.contains("环境异常")
        || body.contains("完成验证后即可继续访问")
        || body.contains("js_verify")
        || body.contains("weixin.qq.com/cgi-bin/verify")
}

pub async fn fetch_html(client: &reqwest::Client, url: &str) -> Result<String, FetchError> {
    let body = tokio::time::timeout(Duration::from_secs(30), async {
        let response = client
            .get(url)
            .header(reqwest::header::REFERER, REFERER)
            .send()
            .await
            .map_err(|e| FetchError::Network(e.to_string()))?;
        let status = response.status();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(FetchError::Verification);
        }
        if !status.is_success() {
            return Err(FetchError::Status(status.as_u16()));
        }
        response
            .text()
            .await
            .map_err(|e| FetchError::Network(e.to_string()))
    })
    .await
    .map_err(|_| FetchError::Network("请求超时".to_string()))??;
    if is_verification_page(&body) {
        return Err(FetchError::Verification);
    }
    Ok(body)
}
