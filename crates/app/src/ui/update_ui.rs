//! 更新提示对话框。

use eframe::egui::{
    self, Align, Color32, CornerRadius, Layout, Margin, RichText, ScrollArea, Vec2,
};

use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme::alpha;
use crate::ui::widgets;
use crate::update::{self, Status};

impl App {
    pub(crate) fn ui_update_dialog(&mut self, ctx: &egui::Context, now: f64) {
        if !self.updater.dialog_open {
            return;
        }
        let pal = self.pal.clone();
        let current = mojuan_core::VERSION;

        // 先把要显示的数据取出来，避免闭包里同时借用 self
        let (version, notes, html_url, has_asset, state) = match &self.updater.status {
            Status::Found(r) => (
                r.version(),
                r.body.clone(),
                r.html_url.clone(),
                r.exe_asset().is_some(),
                "found",
            ),
            Status::Downloading => (String::new(), String::new(), String::new(), false, "downloading"),
            Status::Ready(p) => (
                String::new(),
                String::new(),
                String::new(),
                true,
                if p.exists() { "ready" } else { "broken" },
            ),
            Status::Failed(e) => (String::new(), e.clone(), String::new(), false, "failed"),
            _ => (String::new(), String::new(), String::new(), false, "idle"),
        };

        let mut close = false;
        let mut start_download = false;
        let mut restart = false;
        let mut open_page = false;

        widgets::modal(ctx, &pal, "update-dialog", 520.0, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::new(30.0, 30.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(r, CornerRadius::same(9), pal.accent_soft);
                icons::paint(ui.painter(), r.center(), 16.0, pal.accent, Icon::Refresh, pal.elev);
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    let title = match state {
                        "found" => format!("发现新版本 v{version}"),
                        "downloading" => "正在下载更新…".to_string(),
                        "ready" => "更新已就绪".to_string(),
                        "failed" => "检查更新失败".to_string(),
                        _ => "检查更新".to_string(),
                    };
                    ui.label(
                        RichText::new(title)
                            .size(15.0)
                            .family(pal.bold())
                            .color(pal.text),
                    );
                    let sub = match state {
                        "found" => format!("当前 v{current} → 最新 v{version}"),
                        "ready" => "重启后即可使用新版本".to_string(),
                        _ => format!("当前版本 v{current}"),
                    };
                    ui.label(RichText::new(sub).size(11.5).color(pal.text_dim));
                });
            });

            ui.add_space(12.0);

            if state == "found" {
                let body = if notes.trim().is_empty() {
                    "（这个版本没有填写更新说明）".to_string()
                } else {
                    notes.clone()
                };
                egui::Frame::new()
                    .fill(alpha(pal.text, if pal.dark { 10 } else { 8 }))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_width(486.0);
                        ScrollArea::vertical()
                            .max_height(190.0)
                            .id_salt("update-notes")
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(body)
                                        .size(11.5)
                                        .color(pal.text_dim),
                                );
                            });
                    });
            } else if state == "failed" {
                ui.label(
                    RichText::new(notes.clone())
                        .size(12.0)
                        .color(pal.text_dim),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("可以稍后再试，或在首选项里设置更新源。")
                        .size(11.0)
                        .color(pal.text_faint),
                );
            } else if state == "downloading" {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        RichText::new("正在从 GitHub 下载，请保持网络畅通…")
                            .size(12.0)
                            .color(pal.text_dim),
                    );
                });
            } else if state == "ready" {
                ui.label(
                    RichText::new("下载完成。点击「立即重启」会自动替换并重新打开，大约需要 2 秒。")
                        .size(12.0)
                        .color(pal.text_dim),
                );
            }

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let primary = match state {
                        "ready" => "立即重启",
                        _ => "下载并更新",
                    };
                    let enabled = (state == "found" && has_asset) || state == "ready";
                    let btn = egui::Button::new(
                        RichText::new(primary)
                            .size(12.5)
                            .family(pal.bold())
                            .color(if enabled {
                                Color32::from_rgb(0xFD, 0xF7, 0xF2)
                            } else {
                                pal.text_faint
                            }),
                    )
                    .fill(if enabled { pal.accent } else { alpha(pal.text, 14) })
                    .corner_radius(CornerRadius::same(6))
                    .min_size(Vec2::new(96.0, 30.0));
                    if ui.add_enabled(enabled, btn).clicked() {
                        match state {
                            "ready" => restart = true,
                            _ => start_download = true,
                        }
                    }
                    if widgets::text_button(ui, &pal, "稍后", false).clicked() {
                        close = true;
                    }
                    if !html_url.is_empty() && widgets::text_button(ui, &pal, "发布页", false).clicked()
                    {
                        open_page = true;
                    }
                });
            });
        });

        if start_download {
            if let Status::Found(release) = &self.updater.status {
                let release = release.clone();
                match self.updater.download(&release) {
                    Ok(()) => {}
                    Err(e) => self.toast(e, now),
                }
            }
        }
        if restart {
            if let Status::Ready(path) = &self.updater.status {
                let path = path.clone();
                if let Err(e) = update::install_and_restart(&path) {
                    self.toast(e, now);
                }
            }
        }
        if open_page && !html_url.is_empty() {
            crate::ui::preview::open_external(&html_url);
        }
        if close {
            self.updater.dialog_open = false;
        }
    }
}
