//! 侧栏：文件树 / 大纲 / 搜索 / 最近打开。

use eframe::egui::{
    self, text::LayoutJob, Align, CornerRadius, CursorIcon, FontId, Layout, Pos2, Rect,
    RichText, ScrollArea, Sense, Stroke, Ui, Vec2,
};
use mojuan_core::files;

use crate::app::{App, Dialog, Row, SideTab};
use crate::icons::{self, Icon};
use crate::theme::{alpha, Palette};
use crate::ui::widgets;

enum RowAction {
    Open(std::path::PathBuf),
    Toggle(std::path::PathBuf),
    Menu(std::path::PathBuf, bool, Pos2),
}

impl App {
    pub(crate) fn ui_sidebar(&mut self, ui: &mut Ui, now: f64) {
        let pal = self.pal.clone();
        let mut idx = match self.side {
            SideTab::Files => 0,
            SideTab::Outline => 1,
            SideTab::Search => 2,
            SideTab::Recent => 3,
            SideTab::Favorite => 4,
        };
        ui.horizontal(|ui| {
            if widgets::segmented(ui, &pal, &mut idx, &["文件", "大纲", "搜索", "最近", "收藏"]) {
                self.side = match idx {
                    0 => SideTab::Files,
                    1 => SideTab::Outline,
                    2 => SideTab::Search,
                    3 => SideTab::Recent,
                    _ => SideTab::Favorite,
                };
                if self.side == SideTab::Search {
                    self.search.due = Some(now);
                }
            }
        });
        ui.add_space(8.0);

        match self.side {
            SideTab::Files => self.side_files(ui, &pal, now),
            SideTab::Outline => self.side_outline(ui, &pal),
            SideTab::Search => self.side_search(ui, &pal, now),
            SideTab::Recent => self.side_recent(ui, &pal),
            SideTab::Favorite => self.side_favorites(ui, &pal),
        }

        // 底部：工作区路径
        let root = self.ws.as_ref().map(|w| w.root.clone());
        if let Some(root) = root {
            ui.add_space(4.0);
            widgets::hairline(ui, &pal);
            ui.add_space(4.0);
            let text = root.to_string_lossy().replace('\\', "/");
            let short = shorten_middle(&text, 34);
            let resp = ui.add(
                egui::Label::new(RichText::new(short).size(10.5).color(pal.text_faint))
                    .truncate()
                    .sense(Sense::click()),
            );
            if resp.clicked() {
                files::reveal(&root);
            }
            resp.on_hover_text(text);
        }
    }

    fn side_files(&mut self, ui: &mut Ui, pal: &Palette, now: f64) {
        if self.ws.is_none() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("还没有打开工作区")
                        .size(12.5)
                        .color(pal.text_dim),
                );
                ui.add_space(10.0);
                if widgets::text_button(ui, pal, "打开文件夹", true).clicked() {
                    self.pick_workspace(now);
                }
                ui.add_space(6.0);
                if widgets::text_button(ui, pal, "打开单个文件", false).clicked() {
                    self.pick_open_file(now);
                }
            });
            return;
        }

        // 工具条
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let root = self.ws.as_ref().map(|w| w.root.clone()).unwrap_or_default();
            if widgets::icon_button(ui, pal, Icon::Plus, false, "新建文件").clicked() {
                self.dialog = Some(Dialog::NewFile {
                    dir: self
                        .tab()
                        .and_then(|t| t.path.parent().map(|p| p.to_path_buf()))
                        .unwrap_or(root),
                    name: "未命名.md".into(),
                });
            }
            if widgets::icon_button(ui, pal, Icon::Folder, false, "新建文件夹").clicked() {
                let dir = self
                    .tab()
                    .and_then(|t| t.path.parent().map(|p| p.to_path_buf()))
                    .unwrap_or_default();
                self.dialog = Some(Dialog::NewFolder {
                    dir,
                    name: "新建文件夹".into(),
                });
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button(ui, pal, Icon::Refresh, false, "刷新").clicked() {
                    if let Some(ws) = &mut self.ws {
                        ws.refresh_all();
                    }
                }
            });
        });
        ui.add_space(2.0);

        let rows = self.visible_rows();
        let active = self.active_path();
        let mut action: Option<RowAction> = None;
        let scroll_to_file = std::mem::take(&mut self.tree_scroll_req);
        let mut scrolled = false;

        ScrollArea::vertical()
            .id_salt("tree")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for row in rows {
                    let selected = active.as_deref() == Some(row.path.as_path());
                    let resp = file_row(ui, pal, &row, selected);
                    if scroll_to_file && selected && !scrolled {
                        ui.scroll_to_rect(resp.rect, Some(egui::Align::Center));
                        scrolled = true;
                    }
                    if resp.clicked() {
                        if row.is_dir {
                            action = Some(RowAction::Toggle(row.path.clone()));
                        } else {
                            action = Some(RowAction::Open(row.path.clone()));
                        }
                    }
                    if resp.secondary_clicked() {
                        if let Some(pos) = resp.interact_pointer_pos() {
                            action = Some(RowAction::Menu(row.path.clone(), row.is_dir, pos));
                        }
                    }
                }
                ui.add_space(12.0);
            });

        match action {
            Some(RowAction::Open(p)) => self.open_path(p),
            Some(RowAction::Toggle(p)) => {
                if let Some(ws) = &mut self.ws {
                    if ws.expanded.contains(&p) {
                        ws.expanded.remove(&p);
                    } else {
                        ws.expanded.insert(p.clone());
                        ws.ensure(&p);
                    }
                }
            }
            Some(RowAction::Menu(p, is_dir, pos)) => {
                self.menu = Some(crate::app::CtxMenu {
                    pos,
                    target: Some(p),
                    is_dir,
                });
            }
            None => {}
        }
    }

    fn side_outline(&mut self, ui: &mut Ui, pal: &Palette) {
        let Some(tab) = self.tab() else {
            widgets::empty_state(ui, pal, Icon::Outline, "没有打开的文档", "");
            return;
        };
        let mut toc = tab.doc.toc.clone();
        // 标题过滤（长文档快速定位）
        ui.add(
            egui::TextEdit::singleline(&mut self.outline_filter)
                .hint_text(RichText::new("过滤标题…").color(pal.text_faint))
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(8, 5)),
        );
        ui.add_space(4.0);
        let filter = self.outline_filter.trim().to_lowercase();
        if !filter.is_empty() {
            toc.retain(|e| e.text.to_lowercase().contains(&filter));
        }
        if toc.is_empty() {
            widgets::empty_state(ui, pal, Icon::Outline, "没有匹配的标题", "试试换个关键词");
            return;
        }
        let mut jump: Option<String> = None;
        ScrollArea::vertical()
            .id_salt("outline")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for entry in &toc {
                    let indent = (entry.level.saturating_sub(1) as f32) * 12.0;
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 25.0),
                        Sense::click(),
                    );
                    if resp.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(5),
                            alpha(pal.text, if pal.dark { 16 } else { 10 }),
                        );
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if entry.level > 1 {
                        let gx = rect.min.x + 6.0 + (entry.level.saturating_sub(2) as f32) * 12.0 + 4.0;
                        ui.painter().line_segment(
                            [
                                Pos2::new(gx, rect.min.y + 3.0),
                                Pos2::new(gx, rect.max.y - 3.0),
                            ],
                            Stroke::new(1.0, alpha(pal.text, 20)),
                        );
                    }
                    let strong = entry.level <= 2;
                    let color = if strong { pal.text } else { pal.text_dim };
                    let font = FontId::new(
                        if strong { 12.5 } else { 12.0 },
                        if strong { pal.bold() } else { egui::FontFamily::Proportional },
                    );
                    let mut job = LayoutJob::simple_singleline(entry.text.clone(), font, color);
                    job.wrap.max_width = rect.width() - indent - 12.0;
                    job.wrap.max_rows = 1;
                    job.wrap.overflow_character = Some('…');
                    let g = ui.painter().layout_job(job);
                    ui.painter().galley(
                        Pos2::new(rect.min.x + 6.0 + indent, rect.center().y - g.size().y / 2.0),
                        g,
                        color,
                    );
                    if resp.clicked() {
                        jump = Some(entry.id.clone());
                    }
                }
            });
        if let Some(id) = jump {
            self.jump_to_heading(id);
        }
    }

    fn side_search(&mut self, ui: &mut Ui, pal: &Palette, now: f64) {
        let has_ws = self.ws.is_some();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.search.query)
                .hint_text(RichText::new("搜索文件名与正文…").color(pal.text_faint))
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(8, 6)),
        );
        if resp.changed() {
            self.search.due = Some(now + 0.25);
            self.search.ran = false;
        }
        // Ctrl+F 自动聚焦
        if std::mem::take(&mut self.search_focus) {
            resp.request_focus();
        }
        // Enter 打开第一个结果（文件名命中优先，其次正文首条带行号）
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let target = self.search.result.names.first().map(|n| (n.path.clone(), None))
                .or_else(|| self.search.result.contents.first().map(|c| (c.path.clone(), Some(c.line_no))));
            if let Some((p, ln)) = target {
                self.open_path(p);
                if let Some(n) = ln {
                    self.pending_line = Some(n);
                }
                self.search.due = None;
            }
        }
        ui.add_space(6.0);

        if !has_ws {
            widgets::empty_state(ui, pal, Icon::Search, "先打开一个工作区", "搜索会在工作区内进行");
            return;
        }
        if !self.search.ran {
            ui.label(
                RichText::new(if self.search.query.trim().is_empty() {
                    "输入关键词开始搜索"
                } else {
                    "搜索中…"
                })
                .size(12.0)
                .color(pal.text_faint),
            );
            return;
        }

        let result = self.search.result.clone();
        let total = result.names.len() + result.contents.len();
        ui.label(
            RichText::new(format!(
                "{} 个结果{}",
                total,
                if result.truncated { "（已截断）" } else { "" }
            ))
            .size(11.0)
            .color(pal.text_faint),
        );
        ui.add_space(4.0);

        let mut open: Option<std::path::PathBuf> = None;
        let mut jump: Option<(std::path::PathBuf, usize)> = None;
        ScrollArea::vertical()
            .id_salt("search-results")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                if !result.names.is_empty() {
                    ui.label(RichText::new("文件名").size(10.5).color(pal.text_faint));
                    for hit in &result.names {
                        if result_row(ui, pal, &hit.name, &hit.rel, None).clicked() {
                            open = Some(hit.path.clone());
                        }
                    }
                    ui.add_space(6.0);
                }
                if !result.contents.is_empty() {
                    ui.label(RichText::new("正文").size(10.5).color(pal.text_faint));
                    for hit in &result.contents {
                        let label = format!("L{}", hit.line_no);
                        if result_row(ui, pal, &hit.line, &hit.name, Some(&label)).clicked() {
                            jump = Some((hit.path.clone(), hit.line_no));
                        }
                    }
                }
            });

        if let Some(p) = open {
            self.open_path(p);
        }
        if let Some((p, line)) = jump {
            self.jump_to_line(p, line);
        }
    }

    fn side_favorites(&mut self, ui: &mut Ui, pal: &Palette) {
        if self.store.favorites.is_empty() {
            widgets::empty_state(
                ui,
                pal,
                Icon::Check,
                "还没有收藏",
                "更多菜单 → 收藏当前文档",
            );
            return;
        }
        let mut open: Option<std::path::PathBuf> = None;
        let mut remove: Option<String> = None;
        ScrollArea::vertical()
            .id_salt("favorites")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for key in self.store.favorites.clone() {
                    let path = std::path::PathBuf::from(&key);
                    let name = files::display_name(&path);
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 25.0),
                        Sense::click(),
                    );
                    if resp.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(5),
                            alpha(pal.text, if pal.dark { 16 } else { 10 }),
                        );
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    let g = ui.painter().layout_no_wrap(
                        name.clone(),
                        egui::FontId::new(12.5, egui::FontFamily::Proportional),
                        pal.text,
                    );
                    ui.painter().galley(
                        Pos2::new(rect.min.x + 10.0, rect.center().y - g.size().y / 2.0),
                        g,
                        pal.text,
                    );
                    // 移除按钮（hover 显示）
                    let xr = Rect::from_center_size(
                        Pos2::new(rect.max.x - 12.0, rect.center().y),
                        Vec2::splat(16.0),
                    );
                    let xresp = ui.interact(
                        xr,
                        egui::Id::unique(("fav-x", &key)),
                        Sense::click(),
                    );
                    if xresp.hovered() {
                        icons::paint(
                            ui.painter(),
                            xr.center(),
                            10.0,
                            pal.text_dim,
                            Icon::Close,
                            pal.panel,
                        );
                        if xresp.clicked() {
                            remove = Some(key.clone());
                        }
                    }
                    if resp.clicked() {
                        open = Some(path);
                    }
                }
            });
        if let Some(p) = open {
            self.open_path(p);
        }
        if let Some(key) = remove {
            self.store.favorites.retain(|s| s != &key);
            let _ = self.store.save();
        }
    }

    fn side_recent(&mut self, ui: &mut Ui, pal: &Palette) {
        let items = self.store.recent.clone();
        if items.is_empty() {
            widgets::empty_state(ui, pal, Icon::Clock, "还没有最近记录", "打开过的文件会出现在这里");
            return;
        }
        let mut open: Option<std::path::PathBuf> = None;
        let mut forget: Option<std::path::PathBuf> = None;
        ScrollArea::vertical()
            .id_salt("recent")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for item in &items {
                    let path = std::path::PathBuf::from(&item.path);
                    let name = files::display_name(&path);
                    let parent = path
                        .parent()
                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                        .unwrap_or_default();
                    let resp = recent_row(ui, pal, &name, &parent, item.ts);
                    if resp.clicked() {
                        open = Some(path.clone());
                    }
                    if resp.secondary_clicked() {
                        forget = Some(path.clone());
                    }
                }
            });
        if let Some(p) = open {
            self.open_path(p);
        }
        if let Some(p) = forget {
            self.store.forget_file(&p);
            let _ = self.store.save();
        }
    }
}

fn file_row(ui: &mut Ui, pal: &Palette, row: &Row, selected: bool) -> egui::Response {
    let h = 27.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
    let hovered = resp.hovered();
    if selected {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(0.0, 1.0)),
            CornerRadius::same(6),
            pal.accent_soft,
        );
        ui.painter().rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.min.x + 1.0, rect.center().y - 7.0),
                Vec2::new(2.0, 14.0),
            ),
            1,
            pal.accent,
        );
    } else if hovered {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(0.0, 1.0)),
            CornerRadius::same(6),
            alpha(pal.text, if pal.dark { 14 } else { 9 }),
        );
    }

    let indent = 6.0 + row.depth as f32 * 13.0;
    let mut x = rect.min.x + indent;

    if row.is_dir {
        icons::paint(
            ui.painter(),
            Pos2::new(x + 6.0, rect.center().y),
            12.0,
            pal.text_faint,
            if row.expanded {
                Icon::ChevronDown
            } else {
                Icon::ChevronRight
            },
            pal.panel,
        );
    }
    x += 16.0;

    let ext = row
        .path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let icon_color = if row.is_dir {
        if selected { pal.accent } else { pal.text_dim }
    } else {
        match ext.as_str() {
            "md" | "markdown" | "mdx" => pal.accent,
            _ => pal.text_faint,
        }
    };
    icons::paint(
        ui.painter(),
        Pos2::new(x + 6.0, rect.center().y),
        15.0,
        icon_color,
        if row.is_dir { Icon::Folder } else { Icon::FileText },
        pal.panel,
    );
    x += 18.0;

    let color = if selected {
        pal.text
    } else if row.is_dir {
        pal.text
    } else {
        pal.text_dim
    };
    let font = FontId::new(
        13.0,
        if selected { pal.bold() } else { egui::FontFamily::Proportional },
    );
    let mut job = LayoutJob::simple_singleline(row.name.clone(), font, color);
    job.wrap.max_width = (rect.max.x - x - 8.0).max(20.0);
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let g = ui.painter().layout_job(job);
    ui.painter()
        .galley(Pos2::new(x, rect.center().y - g.size().y / 2.0), g, color);

    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp
}

fn result_row(
    ui: &mut Ui,
    pal: &Palette,
    title: &str,
    subtitle: &str,
    badge: Option<&str>,
) -> egui::Response {
    let h = 42.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(0.0, 1.0)),
            CornerRadius::same(6),
            alpha(pal.text, if pal.dark { 14 } else { 9 }),
        );
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let mut text_w = rect.width() - 16.0;
    if let Some(b) = badge {
        let g = ui.painter().layout_no_wrap(
            b.to_string(),
            FontId::new(10.0, pal.mono()),
            pal.text_faint,
        );
        ui.painter().galley(
            Pos2::new(rect.max.x - g.size().x - 8.0, rect.min.y + 6.0),
            g,
            pal.text_faint,
        );
        text_w -= 40.0;
    }
    let mut job = LayoutJob::simple_singleline(
        title.to_string(),
        FontId::new(12.5, egui::FontFamily::Proportional),
        pal.text,
    );
    job.wrap.max_width = text_w;
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let g = ui.painter().layout_job(job);
    ui.painter()
        .galley(Pos2::new(rect.min.x + 8.0, rect.min.y + 6.0), g, pal.text);

    let mut job2 = LayoutJob::simple_singleline(
        subtitle.to_string(),
        FontId::new(10.5, egui::FontFamily::Proportional),
        pal.text_faint,
    );
    job2.wrap.max_width = text_w;
    job2.wrap.max_rows = 1;
    job2.wrap.overflow_character = Some('…');
    let g2 = ui.painter().layout_job(job2);
    ui.painter().galley(
        Pos2::new(rect.min.x + 8.0, rect.min.y + h - g2.size().y - 6.0),
        g2,
        pal.text_faint,
    );
    resp
}

fn recent_row(ui: &mut Ui, pal: &Palette, name: &str, parent: &str, ts: i64) -> egui::Response {
    let h = 38.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(0.0, 1.0)),
            CornerRadius::same(6),
            alpha(pal.text, if pal.dark { 14 } else { 9 }),
        );
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    icons::paint(
        ui.painter(),
        Pos2::new(rect.min.x + 14.0, rect.center().y),
        14.0,
        pal.accent,
        Icon::FileText,
        pal.panel,
    );
    let mut job = LayoutJob::simple_singleline(
        name.to_string(),
        FontId::new(12.5, egui::FontFamily::Proportional),
        pal.text,
    );
    job.wrap.max_width = rect.width() - 60.0;
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let g = ui.painter().layout_job(job);
    ui.painter()
        .galley(Pos2::new(rect.min.x + 26.0, rect.min.y + 4.0), g, pal.text);

    let mut job2 = LayoutJob::simple_singleline(
        format!("{} · {}", parent, human_time(ts)),
        FontId::new(10.5, egui::FontFamily::Proportional),
        pal.text_faint,
    );
    job2.wrap.max_width = rect.width() - 34.0;
    job2.wrap.max_rows = 1;
    job2.wrap.overflow_character = Some('…');
    let g2 = ui.painter().layout_job(job2);
    ui.painter().galley(
        Pos2::new(rect.min.x + 26.0, rect.min.y + h - g2.size().y - 5.0),
        g2,
        pal.text_faint,
    );
    resp
}

fn human_time(ts: i64) -> String {
    let now = mojuan_core::store::now_ts();
    let d = (now - ts).max(0);
    if d < 60 {
        "刚刚".to_string()
    } else if d < 3600 {
        format!("{} 分钟前", d / 60)
    } else if d < 86400 {
        format!("{} 小时前", d / 3600)
    } else if d < 86400 * 30 {
        format!("{} 天前", d / 86400)
    } else {
        format!("{} 个月前", d / (86400 * 30))
    }
}

fn shorten_middle(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    let head: String = chars[..max / 3].iter().collect();
    let tail: String = chars[chars.len() - max / 3..].iter().collect();
    format!("{head}…{tail}")
}

