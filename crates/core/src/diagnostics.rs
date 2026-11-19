//! 数据驱动失败分类：先按稳定错误码，再按消息关键词匹配建议动作。
//!
//! 规则顺序即优先级，显式 code 永远优先于关键词，避免旧错误误命中新场景。

pub struct FailureRule {
    pub code: &'static str,
    pub contains: &'static [&'static str],
    pub action: &'static str,
}

const RULES: &[FailureRule] = &[
    FailureRule {
        code: "verification",
        contains: &["验证", "环境异常", "js_verify"],
        action: "请在浏览器打开链接完成验证后重试",
    },
    FailureRule {
        code: "disk_full",
        contains: &["磁盘空间", "os error 28", "os error 112"],
        action: "磁盘空间不足，请清理下载目录后重试",
    },
    FailureRule {
        code: "file_busy",
        contains: &["另一个程序", "os error 32", "os error 33"],
        action: "文件被其他程序占用，请关闭预览或相关程序后重试",
    },
    FailureRule {
        code: "permission_denied",
        contains: &["拒绝访问", "os error 5"],
        action: "没有写入权限，请在设置中更换保存目录",
    },
    FailureRule {
        code: "http_429",
        contains: &["429"],
        action: "请求过快，请在设置中降低并发或稍后重试",
    },
    FailureRule {
        code: "network_timeout",
        contains: &["超时"],
        action: "网络超时，请检查网络后重试",
    },
    FailureRule {
        code: "too_large",
        contains: &["超过", "上限"],
        action: "文件超过大小上限，无法下载该文件",
    },
    FailureRule {
        code: "http_auth",
        contains: &[],
        action: "链接可能已过期或需要验证，请在浏览器打开后重试",
    },
];

/// 返回可展示给用户的可执行建议；没有匹配规则时返回 None。
pub fn classify(code: Option<&str>, message: &str) -> Option<&'static str> {
    if let Some(code) = code.filter(|value| !value.is_empty())
        && let Some(rule) = RULES.iter().find(|rule| rule.code == code)
    {
        return Some(rule.action);
    }
    RULES
        .iter()
        .find(|rule| {
            !rule.contains.is_empty()
                && rule
                    .contains
                    .iter()
                    .any(|keyword| message.contains(keyword))
        })
        .map(|rule| rule.action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_explicit_code_first() {
        assert_eq!(
            classify(Some("verification"), "磁盘空间不足"),
            Some("请在浏览器打开链接完成验证后重试")
        );
        assert_eq!(
            classify(Some("http_429"), ""),
            Some("请求过快，请在设置中降低并发或稍后重试")
        );
    }

    #[test]
    fn falls_back_to_keywords() {
        assert_eq!(
            classify(None, "页面需要访问验证，请稍后重试"),
            Some("请在浏览器打开链接完成验证后重试")
        );
        assert_eq!(
            classify(None, "文件写入失败：os error 112"),
            Some("磁盘空间不足，请清理下载目录后重试")
        );
        assert_eq!(
            classify(None, "图片返回异常状态码 429"),
            Some("请求过快，请在设置中降低并发或稍后重试")
        );
    }

    #[test]
    fn unknown_failures_have_no_action() {
        assert_eq!(classify(None, "无法识别图片格式"), None);
        assert_eq!(classify(Some(""), ""), None);
    }
}
