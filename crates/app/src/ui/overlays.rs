//! 浮层：命令面板（Ctrl+P）、右键菜单、提示条。

use std::path::PathBuf;
use std::time::Duration;

use eframe::egui::{
    self, Color32, CornerRadius, FontId, Id, Key, Margin, Pos2, RichText, Sense, Stroke, TextEdit,
    Vec2,
};

use crate::app::{App, Dialog};
use crate::icons::{self, Icon};
use crate::theme::alpha;
use crate::ui::widgets;

#[derive(Clone)]
enum Cmd {
    NewFile,
    OpenFolder,
    OpenFile,
    Save,
    Theme,
    Mode,
    ExportHtml,
    Settings,
    ClearRecent,
}

struct CmdItem {
    label: &'static str,
    hint: &'static str,
    cmd: Cmd,
}

fn commands() -> Vec<CmdItem> {
    vec![
        CmdItem { label: "新建文件", hint: "Ctrl+N", cmd: Cmd::NewFile },
        CmdItem { label: "打开文件夹", hint: "Ctrl+Shift+O", cmd: Cmd::OpenFolder },
        CmdItem { label: "打开文件", hint: "Ctrl+O", cmd: Cmd::OpenFile },
        CmdItem { label: "保存", hint: "Ctrl+S", cmd: Cmd::Save },
        CmdItem { label: "切换主题", hint: "", cmd: Cmd::Theme },
        CmdItem { label: "切换视图（阅读/分屏/编辑）", hint: "Ctrl+E", cmd: Cmd::Mode },
        CmdItem { label: "导出为 HTML", hint: "", cmd: Cmd::ExportHtml },
        CmdItem { label: "首选项", hint: "Ctrl+,", cmd: Cmd::Settings },
        CmdItem { label: "清空最近记录", hint: "", cmd: Cmd::ClearRecent },
    ]
}

#[derive(Clone)]
enum Pick {
    File(PathBuf),
    Cmd(Cmd),
}

impl App {
    pub(crate) fn ui_overlays(&mut self, ctx: &egui::Context, now: f64) {
        let pal = self.pal.clone();

        if let Some(t) = self.toast.clone() {
            let age = now - t.born;
            if age > 2.6 {
                self.toast = None;
            } else {
                let fade = (((2.6 - age) / 0.35).clamp(0.0, 1.0)) as f32;
                widgets::toast(ctx, &pal, &t.text, fade);
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }

        if self.menu.is_some() {
            self.ui_ctx_menu(ctx, now);
        }
        if self.dialog.is_some() {
            self.ui_dialog(ctx, now);
        }
        if self.updater.dialog_open {
            self.ui_update_dialog(ctx, now);
        }
        if self.palette.open {
            self.ui_palette(ctx, now);
        }
    }

    // -- 命令面板 ---------------------------------------------------------

    fn ui_palette(&mut self, ctx: &egui::Context, now: f64) {
        let pal = self.pal.clone();
        let screen = ctx.viewport_rect();
        let mut close = false;
        let mut pick: Option<Pick> = None;

        egui::Area::new(Id::unique("palette-dim"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    0,
                    Color32::from_black_alpha(if pal.dark { 110 } else { 60 }),
                );
                if resp.clicked() {
                    close = true;
                }
            });

        let card_w = 560.0f32;
        let card_pos = Pos2::new(screen.center().x - card_w / 2.0, screen.min.y + 96.0);
        egui::Area::new(Id::unique("palette-card"))
            .order(egui::Order::Tooltip)
            .fixed_pos(card_pos)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(pal.elev)
                    .stroke(Stroke::new(1.0, pal.border))
                    .corner_radius(CornerRadius::same(12))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 16],
                        blur: 40,
                        spread: 0,
                        color: Color32::from_black_alpha(if pal.dark { 140 } else { 50 }),
                    })
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(card_w - 20.0);

                        // 搜索框
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::hover());
                            icons::paint(
                                ui.painter(),
                                r.center(),
                                14.0,
                                pal.text_faint,
                                Icon::Search,
                                pal.elev,
                            );
                            let resp = ui.add(
                                TextEdit::singleline(&mut self.palette.query)
                                    .frame(egui::Frame::NONE)
                                    .hint_text(
                                        RichText::new("搜索文件，或输入 > 执行命令")
                                            .size(14.0)
                                            .color(pal.text_faint),
                                    )
                                    .desired_width(ui.available_width() - 30.0)
                                    .margin(Margin::symmetric(0, 8)),
                            );
                            if !self.palette.focused {
                                resp.request_focus();
                                self.palette.focused = true;
                            }
                            ui.label(
                                RichText::new("Esc")
                                    .size(10.0)
                                    .family(pal.mono())
                                    .color(pal.text_faint),
                            );
                        });
                        widgets::hairline(ui, &pal);

                        // 结果
                        let q = self.palette.query.trim().to_string();
                        let mut items: Vec<(String, String, Pick)> = Vec::new();
                        if q.starts_with('>') {
                            let needle = q.trim_start_matches('>').trim().to_string();
                            for c in commands() {
                                if needle.is_empty()
                                    || mojuan_core::search::fuzzy_score(&needle, c.label).is_some()
                                {
                                    items.push((
                                        c.label.to_string(),
                                        c.hint.to_string(),
                                        Pick::Cmd(c.cmd),
                                    ));
                                }
                            }
                        } else {
                            let files = self.palette_files.clone();
                            let mut scored: Vec<(i32, PathBuf, String)> = Vec::new();
                            for (path, rel) in files {
                                if let Some(score) =
                                    mojuan_core::search::fuzzy_score(&q, &rel)
                                {
                                    scored.push((score, path, rel));
                                }
                            }
                            scored.sort_by(|a, b| b.0.cmp(&a.0));
                            for (_, path, rel) in scored.into_iter().take(9) {
                                items.push((rel.clone(), String::new(), Pick::File(path)));
                            }
                        }

                        if items.is_empty() {
                            ui.add_space(14.0);
                            ui.label(
                                RichText::new("没有匹配项")
                                    .size(12.0)
                                    .color(pal.text_faint),
                            );
                            ui.add_space(10.0);
                        } else {
                            self.palette.sel = self.palette.sel.min(items.len() - 1);
                            if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                                self.palette.sel = (self.palette.sel + 1) % items.len();
                            }
                            if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                                self.palette.sel =
                                    (self.palette.sel + items.len() - 1) % items.len();
                            }
                            if ui.input(|i| i.key_pressed(Key::Enter)) {
                                pick = Some(items[self.palette.sel].2.clone());
                            }
                            let sel = self.palette.sel;
                            for (i, (title, hint, target)) in items.iter().enumerate() {
                                let (rect, resp) = ui.allocate_exact_size(
                                    Vec2::new(ui.available_width(), 34.0),
                                    Sense::click(),
                                );
                                if i == sel || resp.hovered() {
                                    ui.painter().rect_filled(
                                        rect,
                                        CornerRadius::same(7),
                                        if i == sel {
                                            pal.accent_soft
                                        } else {
                                            alpha(pal.text, if pal.dark { 14 } else { 9 })
                                        },
                                    );
                                }
                                let icon = match target {
                                    Pick::File(_) => Icon::FileText,
                                    Pick::Cmd(_) => Icon::Layout,
                                };
                                icons::paint(
                                    ui.painter(),
                                    Pos2::new(rect.min.x + 16.0, rect.center().y),
                                    14.0,
                                    if i == sel { pal.accent } else { pal.text_dim },
                                    icon,
                                    pal.elev,
                                );
                                let mut job = egui::text::LayoutJob::simple_singleline(
                                    title.clone(),
                                    FontId::new(13.0, egui::FontFamily::Proportional),
                                    if i == sel { pal.text } else { pal.text_dim },
                                );
                                job.wrap.max_width = rect.width() - 100.0;
                                job.wrap.max_rows = 1;
                                job.wrap.overflow_character = Some('…');
                                let g = ui.painter().layout_job(job);
                                ui.painter().galley(
                                    Pos2::new(rect.min.x + 28.0, rect.center().y - g.size().y / 2.0),
                                    g,
                                    pal.text,
                                );
                                if !hint.is_empty() {
                                    let hg = ui.painter().layout_no_wrap(
                                        hint.clone(),
                                        FontId::new(10.0, egui::FontFamily::Monospace),
                                        pal.text_faint,
                                    );
                                    ui.painter().galley(
                                        Pos2::new(
                                            rect.max.x - hg.size().x - 10.0,
                                            rect.center().y - hg.size().y / 2.0,
                                        ),
                                        hg,
                                        pal.text_faint,
                                    );
                                }
                                if resp.clicked() {
                                    pick = Some((*target).clone());
                                }
                                if resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                            }
                        }
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("↑↓ 选择 · Enter 确认")
                                    .size(10.0)
                                    .color(pal.text_faint),
                            );
                        });
                    });
            });

        if !self.palette.open {
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            close = true;
        }

        if let Some(p) = pick {
            close = true;
            match p {
                Pick::File(path) => self.open_path(path),
                Pick::Cmd(cmd) => self.run_command(cmd, ctx, now),
            }
        }

        if close {
            self.palette.open = false;
            self.palette.focused = false;
            self.palette.query.clear();
            self.palette.sel = 0;
        }
    }

    fn run_command(&mut self, cmd: Cmd, ctx: &egui::Context, now: f64) {
        match cmd {
            Cmd::NewFile => {
                let dir = self
                    .tab()
                    .and_then(|t| t.path.parent().map(|p| p.to_path_buf()))
                    .or_else(|| self.ws.as_ref().map(|w| w.root.clone()));
                if let Some(dir) = dir {
                    self.dialog = Some(Dialog::NewFile {
                        dir,
                        name: "未命名.md".into(),
                    });
                }
            }
            Cmd::OpenFolder => self.pick_workspace(now),
            Cmd::OpenFile => self.pick_open_file(now),
            Cmd::Save => {
                self.save_active(now);
            }
            Cmd::Theme => {
                let next = match self.store.settings.theme {
                    mojuan_core::Theme::Dark => mojuan_core::Theme::Light,
                    mojuan_core::Theme::Light => mojuan_core::Theme::Dark,
                };
                self.set_theme(ctx, next);
            }
            Cmd::Mode => {
                let next = match self.store.settings.mode {
                    mojuan_core::ViewMode::Read => mojuan_core::ViewMode::Split,
                    mojuan_core::ViewMode::Split => mojuan_core::ViewMode::Edit,
                    mojuan_core::ViewMode::Edit => mojuan_core::ViewMode::Read,
                };
                self.set_mode(next);
            }
            Cmd::ExportHtml => self.export_html(now),
            Cmd::Settings => self.dialog = Some(Dialog::Settings),
            Cmd::ClearRecent => {
                self.store.recent.clear();
                let _ = self.store.save();
                self.toast("已清空最近记录", now);
            }
        }
    }

    // -- 右键菜单 ---------------------------------------------------------

    fn ui_ctx_menu(&mut self, ctx: &egui::Context, now: f64) {
        let pal = self.pal.clone();
        let Some(menu) = self.menu.take() else { return };
        let mut keep = true;
        let mut action: Option<&'static str> = None;

        egui::Area::new(Id::unique("ctx-menu"))
            .order(egui::Order::Tooltip)
            .fixed_pos(menu.pos)
            .show(ctx, |ui| {
                widgets::menu_frame(&pal).show(ui, |ui| {
                    ui.set_width(190.0);
                    let target = menu.target.clone();
                    if let Some(p) = &target {
                        let name = p
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        ui.label(
                            RichText::new(name)
                                .size(11.0)
                                .color(pal.text_faint),
                        );
                        widgets::hairline(ui, &pal);
                        if menu.is_dir {
                            if widgets::menu_item(ui, &pal, Icon::Plus, "新建文件", false).clicked() {
                                action = Some("new-file");
                            }
                            if widgets::menu_item(ui, &pal, Icon::Folder, "新建文件夹", false)
                                .clicked()
                            {
                                action = Some("new-folder");
                            }
                            if widgets::menu_item(ui, &pal, Icon::Folder, "设为工作区", false)
                                .clicked()
                            {
                                action = Some("set-workspace");
                            }
                        } else if widgets::menu_item(ui, &pal, Icon::FileText, "打开", false).clicked()
                        {
                            action = Some("open");
                        }
                        if widgets::menu_item(ui, &pal, Icon::Pencil, "重命名", false).clicked() {
                            action = Some("rename");
                        }
                        if widgets::menu_item(ui, &pal, Icon::External, "在资源管理器中显示", false)
                            .clicked()
                        {
                            action = Some("reveal");
                        }
                        widgets::hairline(ui, &pal);
                        if widgets::menu_item(ui, &pal, Icon::Trash, "删除", true).clicked() {
                            action = Some("delete");
                        }
                    }
                });
            });

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            keep = false;
        }
        if ctx.input(|i| i.pointer.any_click()) && !ctx.is_pointer_over_egui() {
            keep = false;
        }

        if let (Some(act), Some(target)) = (action, menu.target.clone()) {
            keep = false;
            match act {
                "open" => self.open_path(target),
                "new-file" => {
                    let dir = if menu.is_dir {
                        target.clone()
                    } else {
                        target.parent().map(|p| p.to_path_buf()).unwrap_or_default()
                    };
                    self.dialog = Some(Dialog::NewFile {
                        dir,
                        name: "未命名.md".into(),
                    });
                }
                "new-folder" => {
                    let dir = if menu.is_dir {
                        target.clone()
                    } else {
                        target.parent().map(|p| p.to_path_buf()).unwrap_or_default()
                    };
                    self.dialog = Some(Dialog::NewFolder {
                        dir,
                        name: "新建文件夹".into(),
                    });
                }
                "set-workspace" => self.open_workspace(target),
                "rename" => {
                    let name = target
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    self.dialog = Some(Dialog::Rename { path: target, name });
                }
                "reveal" => mojuan_core::files::reveal(&target),
                "delete" => {
                    self.dialog = Some(Dialog::Delete {
                        path: target,
                        is_dir: menu.is_dir,
                    });
                }
                _ => {}
            }
        }

        if keep {
            self.menu = Some(menu);
        } else {
            let _ = now;
        }
    }
}

