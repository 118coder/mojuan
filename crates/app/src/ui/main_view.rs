//! 主区域：标签栏 + 阅读 / 分屏 / 编辑三种布局。

use eframe::egui::{
    self, Align, Color32, CornerRadius, CursorIcon, FontId, Layout, Pos2, Rect, RichText, ScrollArea,
    Sense, Stroke, Ui, Vec2,
};
use mojuan_core::ViewMode;

use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme::alpha;
use crate::ui::{preview, vpreview, widgets};

impl App {
    /// 极简模式顶条：文件名 + 展开完整界面
    pub(crate) fn ui_minimal_bar(&mut self, ui: &mut Ui, now: f64) {
        let pal = self.pal.clone();
        // 无边框窗口：极简条 = 标题栏拖拽区
        crate::ui::titlebar::drag_region(self, ui, "min");
        ui.horizontal_centered(|ui| {
            ui.set_min_height(34.0);
            ui.add_space(2.0);
            let title = self
                .tab()
                .map(|t| t.title.clone())
                .unwrap_or_else(|| "墨卷".into());
            // 限宽截断：长文件名不再把右侧按钮挤出窗口
            let max_w = (ui.available_width() - 200.0).max(80.0);
            ui.allocate_ui_with_layout(
                Vec2::new(max_w, 18.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.add(egui::Label::new(RichText::new(title).size(13.0).color(pal.text_dim)).truncate());
                },
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // 窗口三键在最右
                crate::ui::titlebar::window_buttons(self, ui, &pal);
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("完整界面").size(12.0).color(pal.text),
                        )
                        .frame(false),
                    )
                    .clicked()
                {
                    self.minimal = false;
                }
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("回顶").size(12.0).color(pal.text_dim),
                        )
                        .frame(false),
                    )
                    .clicked()
                {
                    if let Some(t) = self.tab_mut() {
                        t.restore_read = Some(0.0);
                    }
                }
                ui.label(
                    RichText::new("Esc")
                        .size(11.0)
                        .color(pal.text_faint),
                );
            });
        });
        widgets::hairline(ui, &pal);
        let _ = now;
    }

    /// 极简模式内容：md 直接渲染预览，txt 直接编辑
    pub(crate) fn ui_minimal_content(&mut self, ui: &mut Ui, now: f64) {
        if self.tabs.is_empty() {
            self.ui_welcome(ui, now);
            return;
        }
        match self.store.settings.mode {
            ViewMode::Edit => self.ui_editor(ui, false, now),
            _ => self.preview_area(ui, false, now),
        }
    }

    pub(crate) fn ui_main(&mut self, ui: &mut Ui, now: f64) {
        let pal = self.pal.clone();
        let w = ui.available_width();
        ui.allocate_ui_with_layout(
            Vec2::new(w, 38.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                self.ui_tabs(ui, now);
            },
        );
        widgets::hairline(ui, &pal);

        if self.tabs.is_empty() {
            self.ui_welcome(ui, now);
            return;
        }

        match self.store.settings.mode {
            ViewMode::Read => self.preview_area(ui, false, now),
            ViewMode::Edit => self.ui_editor(ui, false, now),
            ViewMode::Split => {
                let total = ui.available_width();
                let height = ui.available_height();
                let ratio = self.store.settings.split_ratio.clamp(0.25, 0.75);
                let ew = (total * ratio).max(220.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(ew, height),
                        Layout::top_down(Align::Min),
                        |ui| self.ui_editor(ui, true, now),
                    );
                    // 拖动分隔条
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(9.0, height), Sense::drag());
                    let hot = resp.hovered() || resp.dragged();
                    if hot {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                    }
                    let x = rect.center().x;
                    ui.painter().rect_filled(
                        Rect::from_min_size(Pos2::new(x - 3.0, rect.min.y), Vec2::new(6.0, rect.height())),
                        0,
                        pal.bg,
                    );
                    ui.painter().rect_filled(
                        Rect::from_min_size(Pos2::new(x - 0.5, rect.min.y), Vec2::new(1.0, rect.height())),
                        0,
                        if hot { pal.accent } else { alpha(pal.text, 26) },
                    );
                    if resp.dragged() {
                        let dx = resp.drag_delta().x;
                        self.store.settings.split_ratio =
                            ((ew + dx) / total).clamp(0.25, 0.75);
                    }
                    let rest = ui.available_width();
                    ui.allocate_ui_with_layout(
                        Vec2::new(rest, height),
                        Layout::top_down(Align::Min),
                        |ui| self.preview_area(ui, true, now),
                    );
                });
            }
        }
    }


    fn ui_welcome(&mut self, ui: &mut Ui, now: f64) {
        let pal = self.pal.clone();
        let w = ui.available_width();
        ui.add_space((ui.available_height() * 0.14).max(40.0));
        ui.vertical_centered(|ui| {
            // 标记
            let (rect, _) = ui.allocate_exact_size(Vec2::new(58.0, 58.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, CornerRadius::same(16), pal.accent);
            let g = ui.painter().layout_no_wrap(
                "墨".to_string(),
                FontId::new(30.0, pal.bold()),
                Color32::from_rgb(0xFD, 0xF8, 0xF3),
            );
            ui.painter().galley(rect.center() - g.size() / 2.0, g, pal.text);
            ui.add_space(18.0);
            ui.label(
                RichText::new("墨 卷")
                    .size(23.0)
                    .family(pal.bold())
                    .color(pal.text),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new("把注意力还给文字")
                    .size(12.5)
                    .color(pal.text_dim),
            );
            ui.add_space(26.0);

            // 精确测量两个按钮的宽度，才能让整行居中
            let w1 = ui
                .painter()
                .layout_no_wrap("打开文件夹".into(), FontId::new(13.0, pal.bold()), pal.text)
                .size()
                .x
                + 24.0;
            let w2 = ui
                .painter()
                .layout_no_wrap(
                    "打开文件".into(),
                    FontId::new(13.0, egui::FontFamily::Proportional),
                    pal.text,
                )
                .size()
                .x
                + 24.0;
            ui.allocate_ui_with_layout(
                Vec2::new(w1 + w2 + 6.0, 34.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    if widgets::text_button(ui, &pal, "打开文件夹", true).clicked() {
                        self.pick_workspace(now);
                    }
                    ui.add_space(2.0);
                    if widgets::text_button(ui, &pal, "打开文件", false).clicked() {
                        self.pick_open_file(now);
                    }
                },
            );
            ui.add_space(14.0);
            ui.label(
                RichText::new("Ctrl+Shift+O 打开工作区   ·   Ctrl+O 打开文件   ·   Ctrl+P 快速跳转")
                    .size(10.5)
                    .color(pal.text_faint),
            );

            // 最近打开
            let recent = self.store.recent.clone();
            if !recent.is_empty() {
                ui.add_space(30.0);
                ui.label(
                    RichText::new("最近打开")
                        .size(10.5)
                        .color(pal.text_faint),
                );
                ui.add_space(4.0);
                let list_w = (w * 0.36).clamp(220.0, 380.0);
                let mut open: Option<std::path::PathBuf> = None;
                ui.allocate_ui_with_layout(Vec2::new(list_w, 0.0), Layout::top_down(Align::Min), |ui| {
                    for item in recent.iter().take(5) {
                        let path = std::path::PathBuf::from(&item.path);
                        let name = mojuan_core::files::display_name(&path);
                        let (rect, resp) = ui.allocate_exact_size(
                            Vec2::new(list_w, 26.0),
                            Sense::click(),
                        );
                        if resp.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                CornerRadius::same(6),
                                alpha(pal.text, if pal.dark { 16 } else { 10 }),
                            );
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        icons::paint(
                            ui.painter(),
                            Pos2::new(rect.min.x + 14.0, rect.center().y),
                            13.0,
                            pal.text_faint,
                            Icon::FileText,
                            pal.read_bg,
                        );
                        let mut job = egui::text::LayoutJob::simple_singleline(
                            name,
                            FontId::new(12.0, egui::FontFamily::Proportional),
                            pal.text_dim,
                        );
                        job.wrap.max_width = list_w - 34.0;
                        job.wrap.max_rows = 1;
                        job.wrap.overflow_character = Some('…');
                        let g = ui.painter().layout_job(job);
                        ui.painter().galley(
                            Pos2::new(rect.min.x + 26.0, rect.center().y - g.size().y / 2.0),
                            g,
                            pal.text_dim,
                        );
                        if resp.clicked() {
                            open = Some(path);
                        }
                    }
                });
                if let Some(p) = open {
                    self.open_path(p);
                }
            }
        });
    }

    fn preview_area(&mut self, ui: &mut Ui, compact: bool, now: f64) {
        let pal = self.pal.clone();

        let idx = self.active;
        let Some(tab) = self.tabs.get_mut(idx) else {
            widgets::empty_state(
                ui,
                &pal,
                Icon::FileText,
                "选择一篇文档开始阅读",
                "Ctrl+O 打开文件 · Ctrl+Shift+O 打开工作区",
            );
            return;
        };

        let path = tab.path.clone();
        let plain = tab.plain;
        // 用 mem::take 拿到所有权，避免每帧克隆大字符串（千万字符时很关键）
        let text = std::mem::take(&mut tab.text);
        let doc = std::mem::take(&mut tab.doc);
        let mut vp = std::mem::take(&mut tab.vpreview);
        let rev = tab.rev;
        let path_id = path.to_string_lossy().to_string();
        let base = path.parent().map(|p| p.to_path_buf());
        let restore = tab.restore_read.take();

        let opts = preview::Opts {
            size: self.store.settings.font_size,
            line_height: self.store.settings.line_spacing,
            measure: self.store.settings.measure,
            compact,
        };
        let mut pending = self.pending_scroll.take();
        let mut pending_scroll_to: Option<f32> = None;
        let mut acts = Vec::new();
        let mut imgs = std::mem::take(&mut self.images);
        let dev = self.dev_scroll;

        let mut new_total = 0.0f32;
        let offset = if text.len() > vpreview::VIRTUAL_THRESHOLD {
            // 千万字路径：只解析、只布局视口内的块
            let (a, off, total) = vpreview::render(
                &mut vp,
                ui,
                &pal,
                &opts,
                &text,
                base.as_deref(),
                rev,
                &path_id,
                &mut pending,
                &mut imgs,
                restore,
                dev,
                plain,
            );
            acts = a;
            new_total = total;
            off
        } else {
            let outer = ui.available_rect_before_wrap();
            let mut area = ScrollArea::vertical()
                .id_salt(("read", &path))
                .auto_shrink([false, false]);
            if let Some(r) = dev.or(restore) {
                area = area.vertical_scroll_offset(r);
            }
            let out = area.show(ui, |ui| {
                ui.add_space(if compact { 18.0 } else { 30.0 });
                acts = preview::render(ui, &pal, &doc, &opts, &mut pending, &mut imgs);
                ui.add_space(140.0);
            });
            let off = out.state.clamped_offset().y;
            let total = out.content_size.y.max(outer.height());
            new_total = total;
            let bar_area = Rect::from_min_size(
                Pos2::new(outer.max.x - 6.0, outer.min.y),
                Vec2::new(6.0, outer.height()),
            );
            let bar_resp = ui.allocate_rect(bar_area, Sense::click() | Sense::drag());
            if let Some(pos) = bar_resp.interact_pointer_pos() {
                let f = ((pos.y - bar_area.min.y) / bar_area.height()).clamp(0.0, 1.0);
                if bar_resp.dragged() || bar_resp.clicked() {
                    pending_scroll_to = Some(f * (total - outer.height()));
                }
            }
            if total > outer.height() + 4.0 {
                let thumb = (outer.height() * outer.height() / total).max(26.0);
                let span = (outer.height() - thumb).max(1.0);
                let frac = (off / (total - outer.height()).max(1.0)).clamp(0.0, 1.0);
                ui.painter().rect_filled(
                    Rect::from_min_size(
                        Pos2::new(outer.max.x - 3.0, outer.min.y + span * frac),
                        Vec2::new(2.0, thumb),
                    ),
                    1,
                    alpha(pal.accent, 110),
                );
            }
            off
        };

        self.images = imgs;
        if let Some(tab) = self.tabs.get_mut(idx) {
            tab.text = text;
            tab.doc = doc;
            tab.vpreview = vp;
            tab.scroll_read = offset;
            tab.read_total = new_total;
            tab.read_view = ui.available_height().max(1.0);
            if let Some(y) = pending_scroll_to {
                tab.restore_read = Some(y);
            }
        }
        self.pending_scroll = pending;

        for a in acts {
            match a {
                preview::Action::Link(url) => self.handle_link(url, now),
                preview::Action::Copy(copied) => {
                    ui.ctx().copy_text(copied);
                    self.toast("已复制到剪贴板", now);
                }
                preview::Action::ViewImage(key) => self.viewer = Some(key),
                preview::Action::ScrollTo(y) => pending_scroll_to = Some(y),
            }
        }
    }

    pub(crate) fn handle_link(&mut self, url: String, now: f64) {
        let base = self
            .tab()
            .and_then(|t| t.path.parent().map(|p| p.to_path_buf()));
        if url.starts_with('#') {
            let id = url.trim_start_matches('#').to_string();
            self.pending_scroll = Some(id);
            return;
        }
        let (external, local) = preview::classify_link(base.as_deref(), &url);
        if external {
            preview::open_external(&url);
            return;
        }
        match local {
            Some(p) if p.exists() => {
                let ext = p
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if matches!(ext.as_str(), "md" | "markdown" | "mdown" | "mdx" | "txt") {
                    self.open_path(p);
                } else {
                    let s = p.to_string_lossy().to_string();
                    preview::open_external(&s);
                }
            }
            Some(p) => {
                self.toast(format!("找不到目标：{}", p.display()), now);
            }
            None => {
                self.toast("无法解析的链接", now);
            }
        }
    }

    // -- 标签栏 -----------------------------------------------------------

    fn ui_tabs(&mut self, ui: &mut Ui, now: f64) {
        let pal = self.pal.clone();
        ui.spacing_mut().item_spacing.x = 4.0;

        let mut activate: Option<usize> = None;
        let mut close: Option<usize> = None;
        let avail = ui.available_width() - 250.0;
        let mut used = 0.0f32;

        ScrollArea::horizontal()
            .id_salt("tabs")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for i in 0..self.tabs.len() {
                        let title = self.tabs[i].title.clone();
                        let dirty = self.tabs[i].dirty;
                        let active = i == self.active;
                        let title_w = ui
                            .painter()
                            .layout_no_wrap(
                                title.clone(),
                                FontId::new(12.5, egui::FontFamily::Proportional),
                                pal.text,
                            )
                            .size()
                            .x;
                        let tw = (title_w + 54.0).clamp(96.0, 200.0);
                        if used + tw > avail && !active && used > 0.0 {
                            break;
                        }
                        used += tw + 4.0;
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(tw, 28.0), Sense::click());
                        if resp.hovered() || active {
                            ui.painter().rect_filled(
                                rect,
                                CornerRadius::same(7),
                                if active {
                                    pal.elev
                                } else {
                                    alpha(pal.text, if pal.dark { 14 } else { 9 })
                                },
                            );
                            if active {
                                ui.painter().rect_stroke(
                                    rect,
                                    CornerRadius::same(7),
                                    Stroke::new(1.0, pal.border),
                                    egui::StrokeKind::Inside,
                                );
                            }
                        }
                        let color = if active { pal.text } else { pal.text_dim };
                        let g = ui.painter().layout_no_wrap(
                            title.clone(),
                            FontId::new(
                                12.5,
                                if active { pal.bold() } else { egui::FontFamily::Proportional },
                            ),
                            color,
                        );
                        let mut job_rect = rect;
                        job_rect.min.x += 12.0;
                        ui.painter().galley(
                            Pos2::new(job_rect.min.x, rect.center().y - g.size().y / 2.0),
                            g,
                            color,
                        );

                        // 关闭按钮 / 脏标记
                        let close_rect = Rect::from_center_size(
                            Pos2::new(rect.max.x - 15.0, rect.center().y),
                            Vec2::splat(18.0),
                        );
                        let close_resp = ui.interact(
                            close_rect,
                            egui::Id::unique(("tab-close", i, &self.tabs[i].path)),
                            Sense::click(),
                        );
                        if close_resp.hovered() {
                            ui.painter().rect_filled(
                                close_rect,
                                CornerRadius::same(4),
                                alpha(pal.text, 26),
                            );
                            icons::paint(
                                ui.painter(),
                                close_rect.center(),
                                11.0,
                                pal.text,
                                Icon::Close,
                                pal.panel,
                            );
                        } else if dirty {
                            let c = Pos2::new(close_rect.center().x, close_rect.center().y);
                            ui.painter().circle_filled(c, 3.0, pal.accent);
                        } else {
                            icons::paint(
                                ui.painter(),
                                close_rect.center(),
                                11.0,
                                alpha(pal.text_faint, 150),
                                Icon::Close,
                                pal.panel,
                            );
                        }
                        if close_resp.clicked() {
                            close = Some(i);
                        } else if resp.clicked_by(egui::PointerButton::Middle) {
                            close = Some(i);
                        } else if resp.secondary_clicked() {
                            if let Some(pos) = resp.interact_pointer_pos() {
                                self.tab_menu = Some((i, pos));
                                // 打开当帧的右键点击不能被当成「点外关闭」（同 content_menu_fresh）
                                self.tab_menu_fresh = true;
                            }
                        } else if resp.clicked() {
                            activate = Some(i);
                        }
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                    }
                    // 标签栏空白区双击 = 新建文件
                    let blank_w = ui.available_width().max(24.0);
                    let (_, blank) = ui.allocate_exact_size(Vec2::new(blank_w, 28.0), Sense::click());
                    if blank.double_clicked() {
                        let dir = self
                            .tab()
                            .and_then(|t| t.path.parent().map(|p| p.to_path_buf()))
                            .or_else(|| self.ws.as_ref().map(|w| w.root.clone()));
                        if let Some(dir) = dir {
                            self.dialog = Some(crate::app::Dialog::NewFile {
                                dir,
                                name: "未命名.md".into(),
                            });
                        }
                    }
                });
            });

        if let Some(i) = activate {
            if let Some(tab) = self.tabs.get_mut(i) {
                tab.restore_edit = Some(tab.scroll_edit);
                tab.restore_read = Some(tab.scroll_read);
            }
            self.active = i;
        }
        if let Some(i) = close {
            self.close_tab(i, now);
        }

        // 右侧：视图模式 + 新建
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let mut idx = match self.store.settings.mode {
                ViewMode::Read => 0,
                ViewMode::Split => 1,
                ViewMode::Edit => 2,
            };
            if widgets::segmented(ui, &pal, &mut idx, &["阅读", "分屏", "编辑"]) {
                self.set_mode(match idx {
                    0 => ViewMode::Read,
                    1 => ViewMode::Split,
                    _ => ViewMode::Edit,
                });
            }
            if widgets::icon_button(ui, &pal, Icon::Plus, false, "新建文件  Ctrl+N").clicked() {
                let dir = self
                    .tab()
                    .and_then(|t| t.path.parent().map(|p| p.to_path_buf()))
                    .or_else(|| self.ws.as_ref().map(|w| w.root.clone()));
                if let Some(dir) = dir {
                    self.dialog = Some(crate::app::Dialog::NewFile {
                        dir,
                        name: "未命名.md".into(),
                    });
                }
            }
        });
    }
}

