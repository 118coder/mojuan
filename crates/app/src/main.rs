//! 墨卷 —— 纯 Rust 的 Markdown 阅读与编辑器。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod art;
mod devshot;
mod export;
mod fonts;
mod img;
mod icons;
mod selftest;
mod theme;
mod ui;
mod update;
mod win;

use eframe::egui;
use mojuan_core::ViewMode;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    // 无头自测模式：不开窗口跑真实布局代码，验证右键菜单等交互，exit code 表示结果
    if std::env::args().nth(1).as_deref() == Some("--selftest") {
        let code = selftest::run_full_selftest();
        std::process::exit(code);
    }
    // 更新替换后，清掉上次留下的旧版本
    update::cleanup_old_exe();
    // 命令行打开文件：双击 .md 关联 / 手动传参（忽略 - 开头的选项）
    let open_file: Option<PathBuf> = std::env::args()
        .nth(1)
        .filter(|s| !s.starts_with('-'))
        .map(PathBuf::from)
        .filter(|p| p.is_file());
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1340.0, 880.0])
            .with_min_inner_size([960.0, 620.0])
            .with_title(format!(
            "墨卷 v{} · 构建于 {}",
            mojuan_core::VERSION,
            crate::win::exe_build_time_str().unwrap_or_else(|| "未知".into())
        ))
            .with_icon(std::sync::Arc::new(art::app_icon())),
        ..Default::default()
    };
    eframe::run_native(
        "mojuan",
        opts,
        Box::new(move |cc| {
            let mut app = app::App::new(cc);
            if let Some(p) = open_file {
                app.open_path(p);
                app.restore_last_scroll();
                // 双击/命令行打开：默认极简模式，打开即见内容。
                // md 先出渲染页（上次会话可能把全局模式留在编辑）；txt 已在 open_path 里设为编辑
                if app.store.settings.minimal_open {
                    app.minimal = true;
                    if !app.tab().map(|t| t.plain).unwrap_or(false) {
                        app.store.settings.mode = ViewMode::Read;
                    }
                }
            }
            Ok(Box::new(app))
        }),
    )
}
