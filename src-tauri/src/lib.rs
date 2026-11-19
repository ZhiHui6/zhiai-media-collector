mod bridge;
mod commands;
mod media;
mod state;

use tauri::Manager;
use zhiai_core::download::{EventSink, queue};
use zhiai_core::model::ThemeMode;
use zhiai_core::storage;

use crate::media::MediaKind;
use crate::state::AppState;

fn not_found() -> tauri::http::Response<std::borrow::Cow<'static, [u8]>> {
    tauri::http::Response::builder()
        .status(404)
        .body(std::borrow::Cow::Owned(Vec::new()))
        .unwrap_or_else(|_| tauri::http::Response::new(std::borrow::Cow::Owned(Vec::new())))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// convertFileSrc 会对整段路径做百分号编码（含 `/`），这里先还原再解析。
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (
                hex_value(bytes[index + 1]),
                hex_value(bytes[index + 2]),
            ) {
                out.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("media", |ctx, request| {
            let path = percent_decode(request.uri().path().trim_start_matches('/'));
            let mut parts = path.split('/');
            let head = parts.next().unwrap_or_default();
            let id = parts
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            let state = ctx.app_handle().state::<AppState>();
            if head == "lib" {
                let file = state
                    .library_path(id)
                    .and_then(|target| media::read_local(&target));
                return match file {
                    Some((bytes, mime)) => tauri::http::Response::builder()
                        .header("Content-Type", mime)
                        .header("Cache-Control", "no-store")
                        .body(std::borrow::Cow::Owned(bytes))
                        .unwrap_or_else(|_| {
                            tauri::http::Response::new(std::borrow::Cow::Owned(Vec::new()))
                        }),
                    None => not_found(),
                };
            }
            let kind = if head == "full" {
                MediaKind::Full
            } else {
                MediaKind::Thumb
            };
            match state.media.get(kind, id) {
                Some(bytes) => tauri::http::Response::builder()
                    .header("Content-Type", "image/png")
                    .header("Cache-Control", "no-store")
                    .body(std::borrow::Cow::Owned(bytes.to_vec()))
                    .unwrap_or_else(|_| {
                        tauri::http::Response::new(std::borrow::Cow::Owned(Vec::new()))
                    }),
                None => not_found(),
            }
        })
        .setup(|app| {
            storage::migrate_legacy_config();
            let settings = storage::load_settings();
            let history = storage::load_history();
            let (event_tx, event_rx) = std::sync::mpsc::channel();
            let sink = EventSink::new(event_tx, std::sync::Arc::new(|| {}));
            let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
            tauri::async_runtime::spawn(queue::run(command_rx, sink, settings.clone()));
            app.manage(AppState::new(command_tx, settings.clone(), history));
            bridge::spawn(app.handle().clone(), event_rx);
            if let Some(window) = app.get_webview_window("main") {
                let title = format!("智爱媒体素材采集器 v{}", env!("CARGO_PKG_VERSION"));
                let _ = window.set_title(&title);
                let theme = match settings.theme {
                    ThemeMode::Light => Some(tauri::Theme::Light),
                    ThemeMode::Dark => Some(tauri::Theme::Dark),
                    ThemeMode::System => None,
                };
                let _ = window.set_theme(theme);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::parse_text,
            commands::parse_album,
            commands::enqueue,
            commands::open_preview,
            commands::cancel_item,
            commands::retry_item,
            commands::retry_all_failed,
            commands::pause_all,
            commands::resume_all,
            commands::set_concurrency,
            commands::clear_finished,
            commands::update_settings,
            commands::get_bootstrap,
            commands::set_download_root,
            commands::reveal_path,
            commands::open_directory,
            commands::apply_theme,
            commands::library_prepare,
            commands::library_delete,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
                handle.state::<AppState>().save_history();
            }
        });
}
