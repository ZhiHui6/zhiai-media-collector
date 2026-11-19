use std::collections::HashSet;

use crate::model::{ImageKind, PageKind};

use super::normalize::dedup_key;

pub fn fallback_kind(url: &str, page_kind: PageKind, cover_keys: &HashSet<String>) -> ImageKind {
    if let Some(key) = dedup_key(url)
        && cover_keys.contains(&key)
    {
        return ImageKind::Cover;
    }
    if let Ok(parsed) = url::Url::parse(url)
        && parsed
            .host_str()
            .is_some_and(|host| host.ends_with("qlogo.cn"))
    {
        return ImageKind::Avatar;
    }
    match page_kind {
        PageKind::PicturePost => ImageKind::Album,
        _ => ImageKind::Article,
    }
}
