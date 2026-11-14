//! 更新提示对话框。

use eframe::egui::{
    self, Align, Color32, CornerRadius, CursorIcon, FontId, Id, Layout, Margin, Pos2, Rect,
    RichText, ScrollArea, Sense, Stroke, Vec2,
};

use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme::alpha;
use crate::ui::widgets;
use crate::update::{self, Status};

impl App {
    pub(crate) fn ui_update_dialog(&mut self, ctx: &egui::Context, now: f64) {        if !self.updater.dialog_open {
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
                    clean_notes(&notes)
                };
                egui::Frame::new()
                    .fill(alpha(pal.text, if pal.dark { 12 } else { 9 }))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(13, 11))
                    .show(ui, |ui| {
                        ui.set_width(486.0);
                        // 让滚动条在静止时也隐约可见，提示"下面还有内容"
                        ui.style_mut().spacing.scroll.dormant_handle_opacity = 0.28;
                        ui.style_mut().spacing.scroll.dormant_background_opacity = 0.05;
                        ScrollArea::vertical()
                            .max_height(198.0)
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                            )
                            .id_salt("update-notes")
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(body)
                                            .size(12.0)
                                            .line_height(Some(19.0))
                                            .color(alpha(pal.text, 205)),
                                    )
                                    .wrap(),
                                );
                            });
                    });
                if !has_asset {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("这个版本没有提供 exe 附件，可以到发布页手动下载。")
                            .size(11.0)
                            .color(pal.text_faint),
                    );
                }
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
                    // 没有 exe 附件时，不给一个永远点不动的按钮
                    let show_primary = state == "ready" || (state == "found" && has_asset);
                    let enabled = show_primary;
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
                    if show_primary && ui.add_enabled(enabled, btn).clicked() {
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

/// 把 GitHub 的 release 说明（Markdown）压成适合弹窗阅读的纯文本。
///
/// 只做"降噪"不做完整渲染：去掉 HTML 注释、代码围栏、图片、链接地址、
/// 标题井号与强调星号，保留换行和缩进，让说明能直接读。
fn clean_notes(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_comment = false;
    let mut in_code = false;

    for raw in src.lines() {
        let line = raw.trim_end();

        // 跨行的 HTML 注释
        if in_comment {
            if let Some(p) = line.find("-->") {
                in_comment = false;
                let rest = line[p + 3..].trim();
                if !rest.is_empty() {
                    out.push_str(rest);
                    out.push('\n');
                }
            }
            continue;
        }
        if line.trim_start().starts_with("<!--") {
            if !line.contains("-->") {
                in_comment = true;
            }
            continue;
        }

        // 代码围栏：只保留中间的内容
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }

        let mut s = line.to_string();
        if !in_code {
            s = strip_links(&s);
            s = s.replace("**", "").replace("__", "");
            // 行首标题井号
            let t = s.trim_start();
            let hashes = t.len() - t.trim_start_matches('#').len();
            if hashes > 0 && t.chars().nth(hashes) == Some(' ') {
                s = t[hashes..].trim_start().to_string();
            }
            // 列表符号统一成 ·
            let t = s.trim_start();
            for marker in ["- [ ] ", "- [x] ", "- ", "* ", "+ "] {
                if let Some(rest) = t.strip_prefix(marker) {
                    let indent = s.len() - t.len();
                    s = format!("{}{} {}", " ".repeat(indent), "·", rest);
                    break;
                }
            }
            s = s.replace('`', "");
        }
        out.push_str(s.trim_end());
        out.push('\n');
    }

    // 压掉连续空行
    let mut cleaned = String::with_capacity(out.len());
    let mut blank = 0;
    for line in out.lines() {
        if line.trim().is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        cleaned.push_str(line);
        cleaned.push('\n');
    }
    cleaned.trim().to_string()
}

/// `[文字](链接)` → `文字`，`![说明](图)` → `说明`
fn strip_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        let is_img = bytes[i] == '!' && i + 1 < bytes.len() && bytes[i + 1] == '[';
        if bytes[i] == '[' || is_img {
            let open = if is_img { i + 1 } else { i };
            if let Some(close) = find_char(&bytes, open + 1, ']') {
                if close + 1 < bytes.len() && bytes[close + 1] == '(' {
                    if let Some(end) = find_char(&bytes, close + 2, ')') {
                        for c in &bytes[open + 1..close] {
                            out.push(*c);
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

fn find_char(chars: &[char], from: usize, target: char) -> Option<usize> {
    (from..chars.len()).find(|&i| chars[i] == target)
}

impl App {
    /// 右下角的小浮窗：只提醒，不抢焦点。点它才打开完整对话框。
    pub(crate) fn ui_update_banner(&mut self, ctx: &egui::Context, now: f64) {
        if !self.updater.banner || self.updater.dialog_open {
            return;
        }
        // 只有真的"发现新版本"才显示；一旦状态变化就收起
        let Status::Found(release) = &self.updater.status else {
            self.updater.banner = false;
            return;
        };
        let version = release.version();
        let has_asset = release.exe_asset().is_some();
        let pal = self.pal.clone();
        let current = mojuan_core::VERSION;

        let screen = ctx.viewport_rect();
        let w = 274.0;
        let h = 62.0;
        // 抬到状态栏之上，别压在字数统计上
        let pos = Pos2::new(screen.max.x - w - 18.0, screen.max.y - h - 26.0 - 12.0);

        let mut open_dialog = false;
        let mut dismiss = false;

        egui::Area::new(Id::unique("update-banner"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .interactable(true)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
                let hot = resp.hovered();
                if hot {
                    ctx.set_cursor_icon(CursorIcon::PointingHand);
                }

                // 阴影 + 卡片
                ui.painter().rect_filled(
                    rect.translate(Vec2::new(0.0, 3.0)),
                    CornerRadius::same(11),
                    Color32::from_black_alpha(if pal.dark { 70 } else { 26 }),
                );
                ui.painter().rect(
                    rect,
                    CornerRadius::same(11),
                    if pal.dark {
                        pal.elev
                    } else {
                        Color32::from_rgb(0xFF, 0xFD, 0xFA)
                    },
                    Stroke::new(1.0, if hot { alpha(pal.accent, 120) } else { pal.border_soft }),
                    egui::StrokeKind::Inside,
                );

                // 左侧图标
                let icon_c = Pos2::new(rect.min.x + 25.0, rect.center().y);
                ui.painter()
                    .circle_filled(icon_c, 14.0, alpha(pal.accent, 26));
                icons::paint(
                    ui.painter(),
                    icon_c,
                    15.0,
                    pal.accent,
                    Icon::ChevronDown,
                    pal.elev,
                );

                // 文案
                let g1 = ui.painter().layout_no_wrap(
                    format!("有新版本 v{version}"),
                    FontId::new(12.5, pal.bold()),
                    pal.text,
                );
                let sub = if has_asset {
                    format!("当前 v{current} · 点击查看")
                } else {
                    format!("当前 v{current} · 需手动下载")
                };
                let g2 = ui.painter().layout_no_wrap(
                    sub,
                    FontId::new(10.5, egui::FontFamily::Proportional),
                    pal.text_faint,
                );
                let tx = rect.min.x + 46.0;
                // 文案区单独裁剪，长版本号不会顶到关闭按钮上
                let text_clip = Rect::from_min_max(
                    Pos2::new(tx, rect.min.y + 4.0),
                    Pos2::new(rect.max.x - 32.0, rect.max.y - 4.0),
                );
                let tp = ui.painter().with_clip_rect(text_clip);
                tp.galley(
                    Pos2::new(tx, rect.center().y - g1.size().y - 2.0),
                    g1,
                    pal.text,
                );
                tp.galley(
                    Pos2::new(tx, rect.center().y + 3.0),
                    g2,
                    pal.text_faint,
                );

                // 右上角关闭
                let close_c = Pos2::new(rect.max.x - 16.0, rect.min.y + 16.0);
                let close_rect = Rect::from_center_size(close_c, Vec2::new(18.0, 18.0));
                let close_resp = ui.interact(
                    close_rect,
                    Id::unique("update-banner-close"),
                    Sense::click(),
                );
                let col = if close_resp.hovered() {
                    pal.text
                } else {
                    pal.text_faint
                };
                icons::paint(ui.painter(), close_c, 11.0, col, Icon::Close, pal.elev);
                if close_resp.clicked() {
                    dismiss = true;
                }

                if resp.clicked() {
                    open_dialog = true;
                }
            });

        if dismiss {
            self.updater.banner = false;
            return;
        }
        if open_dialog {
            self.updater.dialog_open = true;
            self.updater.banner = false;
            let _ = now;
        }
    }
}
