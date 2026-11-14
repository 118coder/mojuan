//! 墨卷 —— 纯 Rust 的 Markdown 阅读与编辑器。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod art;
mod devshot;
mod fonts;
mod img;
mod icons;
mod theme;
mod ui;
mod update;
mod win;

use eframe::egui;

fn main() -> eframe::Result<()> {
    // 更新替换后，清掉上次留下的旧版本
    update::cleanup_old_exe();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1340.0, 880.0])
            .with_min_inner_size([960.0, 620.0])
            .with_title("墨卷 · Mojuan")
            .with_icon(std::sync::Arc::new(art::app_icon())),
        ..Default::default()
    };
    eframe::run_native(
        "mojuan",
        opts,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
