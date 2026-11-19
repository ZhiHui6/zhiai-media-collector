use zhiai_core::extract::fallback;
use zhiai_core::fetch;

fn main() {
    let Some(url) = std::env::args().nth(1) else {
        eprintln!("用法: cargo run --example probe_page -- <公众号链接>");
        return;
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build runtime");
    let client = fetch::build_client().expect("failed to build http client");
    let html = match runtime.block_on(fetch::fetch_html(&client, &url)) {
        Ok(html) => html,
        Err(err) => {
            eprintln!("抓取失败: {err}");
            return;
        }
    };
    println!("响应长度: {}", html.len());
    for marker in [
        "js_content",
        "picture_page_info",
        "js_picture",
        "share_picture",
        "msg_picture",
        "picture_list",
        "pic_list",
        "album_id",
        "appmsgalbum",
        "msg_cdn_url",
        "og:image",
    ] {
        println!("标记 {marker}: {}", html.contains(marker));
    }

    let images = fallback::scan(&html);
    println!("可识别图片: {}", images.len());
    for image in images.iter().take(12) {
        println!("  {}  hint={:?}", image.url, image.hint);
    }

    println!("可能的结构变量:");
    let mut printed = 0;
    for line in html.lines() {
        let trimmed = line.trim();
        let looks_like_script = trimmed.starts_with("var ") || trimmed.starts_with("window.");
        let relevant = trimmed.contains("picture") || trimmed.contains("album");
        if looks_like_script && relevant {
            println!("  {}", trimmed.chars().take(240).collect::<String>());
            printed += 1;
            if printed >= 20 {
                break;
            }
        }
    }
}
