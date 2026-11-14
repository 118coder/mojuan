//! 顶栏：品牌、工作区、搜索入口、主题与设置。

use eframe::egui::{
    self, Align, Color32, CornerRadius, CursorIcon, FontId, Layout, Pos2, Rect, RichText, Sense,
    Stroke, Ui, Vec2,
};
use mojuan_core::Theme;

use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme::{alpha, Palette, R_SM};
use crate::ui::widgets;

impl App {
    pub(crate) fn ui_topbar(&mut self, ui: &mut Ui, now: f64) {
        let pal = self.pal.clone();
        let h = ui.available_height();
        let mut more_rect: Option<egui::Rect> = None;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            let open = self.store.settings.sidebar_open;
            if widgets::icon_button(ui, &pal, Icon::Sidebar, open, "显示/隐藏侧栏  Ctrl+B").clicked() {
                self.store.settings.sidebar_open = !open;
            }

            // 品牌标识
            let (rect, _) = ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, CornerRadius::same(7), pal.accent);
            let g = ui.painter().layout_no_wrap(
                "墨".to_string(),
                FontId::new(13.0, pal.bold()),
                Color32::from_rgb(0xFD, 0xF8, 0xF3),
            );
            ui.painter().galley(rect.center() - g.size() / 2.0, g, pal.text);

            ui.add_space(2.0);
            ui.label(
                RichText::new("墨卷")
                    .size(14.5)
                    .family(pal.bold())
                    .color(pal.text),
            );

            ui.add_space(6.0);
            // 分隔
            let (sep, _) = ui.allocate_exact_size(Vec2::new(1.0, 16.0), Sense::hover());
            ui.painter()
                .rect_filled(sep, 0, pal.border);
            ui.add_space(6.0);

            // 工作区切换
            let name = self.workspace_name();
            let chip = workspace_chip(ui, &pal, &name, self.ws.is_some());
            if chip.clicked() {
                self.ws_menu = !self.ws_menu;
            }
            if chip.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let more = widgets::icon_button(ui, &pal, Icon::More, self.more_menu, "更多");
                more_rect = Some(more.rect);
                if more.clicked() {
                    self.more_menu = !self.more_menu;
                }
                let theme_icon = match self.store.settings.theme {
                    Theme::Dark => Icon::Sun,
                    Theme::Light => Icon::Moon,
                };
                if widgets::icon_button(ui, &pal, theme_icon, false, "切换主题").clicked() {
                    let next = match self.store.settings.theme {
                        Theme::Dark => Theme::Light,
                        Theme::Light => Theme::Dark,
                    };
                    let ctx = ui.ctx().clone();
                    self.set_theme(&ctx, next);
                }
                if widgets::icon_button(ui, &pal, Icon::Save, false, "保存  Ctrl+S").clicked() {
                    self.save_active(now);
                }
                // 搜索入口
                let search = search_button(ui, &pal);
                if search.clicked() {
                    self.palette.open = true;
                    self.palette.query.clear();
                    self.palette.sel = 0;
                }
                if search.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
            });
        });

        // 工作区下拉
        if self.ws_menu {
            self.ui_ws_menu(ui.ctx(), now);
        }
        if self.more_menu {
            if let Some(rect) = more_rect {
                self.ui_more_menu(ui.ctx(), rect, now);
            }
        }
        let _ = h;
    }


    fn ui_more_menu(&mut self, ctx: &egui::Context, anchor: egui::Rect, now: f64) {
        let pal = self.pal.clone();
        let mut close = false;
        let mut action: Option<&'static str> = None;
        let version_line = format!("v{}", mojuan_core::VERSION);
        let status_hint = match &self.updater.status {
            crate::update::Status::Found(r) => Some(format!("新版本 v{}", r.version())),
            crate::update::Status::Checking => Some("检查中…".to_string()),
            crate::update::Status::Latest(v) => Some(format!("已是最新 v{v}")),
            _ => None,
        };

        egui::Area::new(egui::Id::unique("more-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(anchor.right() - 208.0, anchor.bottom() + 6.0))
            .show(ctx, |ui| {
                widgets::menu_frame(&pal).show(ui, |ui| {
                    ui.set_width(200.0);
                    let label = match &status_hint {
                        Some(s) => format!("检查更新（{s}）"),
                        None => "检查更新".to_string(),
                    };
                    if widgets::menu_item(ui, &pal, Icon::Refresh, &label, false).clicked() {
                        action = Some("update");
                    }
                    if widgets::menu_item(ui, &pal, Icon::FileText, "打开示例文档", false).clicked() {
                        action = Some("sample");
                    }
                    if widgets::menu_item(ui, &pal, Icon::Layout, "首选项", false).clicked() {
                        action = Some("settings");
                    }
                    widgets::hairline(ui, &pal);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("墨卷")
                                .size(11.0)
                                .color(pal.text_faint),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(version_line)
                                    .size(10.5)
                                    .family(pal.mono())
                                    .color(pal.text_faint),
                            );
                        });
                    });
                    let _ = ui.add(
                        egui::Label::new(
                            RichText::new("关于 / 快捷键").size(11.5).color(pal.text_dim),
                        )
                        .sense(Sense::click()),
                    );
                });
            });

        // 关于：点击底部那一行
        if ctx.input(|i| i.pointer.any_click()) && !ctx.is_pointer_over_egui() {
            close = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }

        if let Some(a) = action {
            close = true;
            match a {
                "update" => {
                    let repo = self.updater.repo(&self.store.settings.update_repo);
                    self.updater.check(repo, now);
                    self.updater.dialog_open = true;
                }
                "sample" => self.open_sample(now),
                "settings" => self.dialog = Some(crate::app::Dialog::Settings),
                _ => {}
            }
        }
        if close {
            self.more_menu = false;
        }
    }

    fn ui_ws_menu(&mut self, ctx: &egui::Context, now: f64) {
        let pal = self.pal.clone();
        let mut close = false;
        egui::Area::new(egui::Id::unique("ws-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(160.0, 44.0))
            .show(ctx, |ui| {
                widgets::menu_frame(&pal).show(ui, |ui| {
                    ui.set_width(240.0);
                    if widgets::menu_item(ui, &pal, Icon::Folder, "打开文件夹…", false).clicked() {
                        self.pick_workspace(now);
                        close = true;
                    }
                    if widgets::menu_item(ui, &pal, Icon::FileText, "打开文件…", false).clicked() {
                        self.pick_open_file(now);
                        close = true;
                    }
                    widgets::hairline(ui, &pal);
                    ui.label(
                        RichText::new("最近的工作区")
                            .size(11.0)
                            .color(pal.text_faint),
                    );
                    let items: Vec<String> = self.store.workspaces.clone();
                    for w in items.iter().take(5) {
                        let short = w.rsplit(['\\', '/']).next().unwrap_or(w).to_string();
                        if widgets::menu_item(ui, &pal, Icon::Folder, &short, false).clicked() {
                            self.open_workspace(std::path::PathBuf::from(w));
                            close = true;
                        }
                    }
                });
            });
        // 点击别处关闭
        if ctx.input(|i| i.pointer.any_click()) {
            if !ctx.is_pointer_over_egui() {
                close = true;
            }
        }
        if close {
            self.ws_menu = false;
        }
    }
}

fn workspace_chip(ui: &mut Ui, pal: &Palette, name: &str, has_ws: bool) -> egui::Response {
    let font = FontId::new(12.5, egui::FontFamily::Proportional);
    let galley = ui.painter().layout_no_wrap(name.to_string(), font.clone(), pal.text);
    let w = galley.size().x + 44.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w.min(240.0), 26.0), Sense::click());
    let hovered = resp.hovered();
    if hovered {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(R_SM),
            alpha(pal.text, if pal.dark { 16 } else { 10 }),
        );
    }
    let col = if has_ws { pal.text_dim } else { pal.text_faint };
    icons::paint(
        ui.painter(),
        Pos2::new(rect.min.x + 14.0, rect.center().y),
        15.0,
        col,
        Icon::Folder,
        pal.panel,
    );
    let mut g = galley;
    let max_text_w = rect.width() - 40.0;
    if g.size().x > max_text_w {
        g = ui.painter().layout(
            name.to_string(),
            font,
            pal.text_dim,
            max_text_w,
        );
    }
    ui.painter().galley(
        Pos2::new(rect.min.x + 26.0, rect.center().y - g.size().y / 2.0),
        g,
        col,
    );
    icons::paint(
        ui.painter(),
        Pos2::new(rect.max.x - 12.0, rect.center().y),
        12.0,
        pal.text_faint,
        Icon::ChevronDown,
        pal.panel,
    );
    resp
}

fn search_button(ui: &mut Ui, pal: &Palette) -> egui::Response {
    let text = "搜索文件";
    let font = FontId::new(12.5, egui::FontFamily::Proportional);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, pal.text_faint);
    let w = galley.size().x + 84.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::click());
    let hovered = resp.hovered();
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(R_SM),
        if hovered {
            alpha(pal.text, if pal.dark { 18 } else { 12 })
        } else {
            alpha(pal.text, if pal.dark { 10 } else { 8 })
        },
    );
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(R_SM),
        Stroke::new(1.0, if hovered { pal.border } else { Color32::TRANSPARENT }),
        egui::StrokeKind::Inside,
    );
    icons::paint(
        ui.painter(),
        Pos2::new(rect.min.x + 15.0, rect.center().y),
        13.0,
        pal.text_faint,
        Icon::Search,
        pal.panel,
    );
    ui.painter().galley(
        Pos2::new(rect.min.x + 26.0, rect.center().y - galley.size().y / 2.0),
        galley,
        pal.text_faint,
    );
    // Ctrl+P 提示
    let hint = "Ctrl+P";
    let g2 = ui.painter().layout_no_wrap(
        hint.to_string(),
        FontId::new(10.0, egui::FontFamily::Monospace),
        pal.text_faint,
    );
    let hr = Rect::from_min_size(
        Pos2::new(rect.max.x - g2.size().x - 18.0, rect.center().y - 8.0),
        Vec2::new(g2.size().x + 12.0, 16.0),
    );
    ui.painter()
        .rect_filled(hr, CornerRadius::same(4), alpha(pal.text, 12));
    ui.painter()
        .galley(hr.center() - g2.size() / 2.0, g2, pal.text_faint);
    resp
}

