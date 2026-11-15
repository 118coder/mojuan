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
    ExportWord,
    ExportPdf,
    ExportPng,
    Settings,
    ClearRecent,
    ToggleSidebar,
    Replace,
    RevealInExplorer,
    Fullscreen,
    FindDoc,
    Print,
    InsertTime,
    GotoLine,
    SaveGbk,
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
        CmdItem { label: "导出为 Word（docx）", hint: "", cmd: Cmd::ExportWord },
        CmdItem { label: "导出为 PDF", hint: "", cmd: Cmd::ExportPdf },
        CmdItem { label: "导出为长图 PNG", hint: "", cmd: Cmd::ExportPng },
        CmdItem { label: "首选项", hint: "Ctrl+,", cmd: Cmd::Settings },
        CmdItem { label: "清空最近记录", hint: "", cmd: Cmd::ClearRecent },
        CmdItem { label: "查找替换（当前文档）", hint: "Ctrl+H", cmd: Cmd::Replace },
        CmdItem { label: "显示 / 隐藏侧栏", hint: "Ctrl+", cmd: Cmd::ToggleSidebar },
        CmdItem { label: "在资源管理器中显示", hint: "", cmd: Cmd::RevealInExplorer },
        CmdItem { label: "全屏切换", hint: "F11", cmd: Cmd::Fullscreen },
        CmdItem { label: "在文档中查找", hint: "Ctrl+F", cmd: Cmd::FindDoc },
        CmdItem { label: "打印…", hint: "Ctrl+P", cmd: Cmd::Print },
        CmdItem { label: "插入日期时间", hint: "F5", cmd: Cmd::InsertTime },
        CmdItem { label: "跳转到行", hint: "Ctrl+G", cmd: Cmd::GotoLine },
        CmdItem { label: "另存为 GBK 编码", hint: "", cmd: Cmd::SaveGbk },
    ]
}

#[derive(Clone)]
enum Pick {
    File(PathBuf),
    Cmd(Cmd),
}

impl App {
    pub(crate) fn ui_overlays(&mut self, ctx: &egui::Context, now: f64) {
        // 文档内查找浮条
        if self.find.is_some() {
            self.ui_find_bar(ctx, now);
        }
        // 图片全屏查看器
        if let Some(key) = self.viewer.clone() {
            let pal = self.pal.clone();
            let wheel = ctx.input(|i| i.smooth_scroll_delta.y);
            if wheel != 0.0 {
                self.viewer_scale = (self.viewer_scale * if wheel > 0.0 { 1.1 } else { 1.0 / 1.1 })
                    .clamp(0.2, 8.0);
            }
            let close = ctx.input(|i| i.pointer.any_click() || i.key_pressed(egui::Key::Escape));
            egui::Area::new(egui::Id::unique("image-viewer"))
                .order(egui::Order::Tooltip)
                .fixed_pos(ctx.input(|i| i.viewport().outer_rect.unwrap_or(egui::Rect::NOTHING).min))
                .show(ctx, |ui| {
                    let screen = ui.max_rect();
                    ui.painter()
                        .rect_filled(screen, 0, Color32::from_black_alpha(216));
                    let tex = if key.starts_with("http://") || key.starts_with("https://") {
                        self.images.get_url(ctx, &key)
                    } else {
                        let p = std::path::PathBuf::from(&key);
                        self.images.get(ctx, &p)
                    };
                    if let Some(h) = tex {
                        let natural = h.size_vec2();
                        let scale = (screen.width() * 0.9 / natural.x)
                            .min(screen.height() * 0.9 / natural.y)
                            * self.viewer_scale;
                        let draw = natural * scale;
                        let (rect, _) = ui.allocate_exact_size(draw, egui::Sense::hover());
                        let top_left = screen.center() - draw / 2.0;
                        ui.painter().image(
                            h.id(),
                            egui::Rect::from_min_size(top_left, draw),
                            egui::Rect::from_min_size(egui::Pos2::ZERO, natural),
                            pal.text,
                        );
                        let _ = rect;
                    } else {
                        ui.label(
                            RichText::new("图片加载中…")
                                .size(15.0)
                                .color(pal.text_dim),
                        );
                    }
                    // 关闭提示
                    let hint = ui.painter().layout_no_wrap(
                        "点击任意处或 Esc 关闭".to_string(),
                        egui::FontId::new(12.0, egui::FontFamily::Proportional),
                        pal.text_dim,
                    );
                    ui.painter().galley(
                        Pos2::new(screen.center().x - hint.size().x / 2.0, screen.max.y - 36.0),
                        hint,
                        pal.text_dim,
                    );
                });
            if close {
                self.viewer = None;
                self.viewer_scale = 1.0;
            }
        }

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

        // 无条件调用：关闭态在函数内清 rect/fresh（is_some 门住会让状态永远清不掉）
        self.ui_ctx_menu(ctx, now);
        // 标签右键菜单：曾只置状态从未渲染（死代码，右键标签条毫无反应），
        // 这里补上渲染调用
        self.ui_tab_menu(ctx, now);
        if self.dialog.is_some() {
            self.ui_dialog(ctx, now);
        }
        if self.updater.dialog_open {
            self.ui_update_dialog(ctx, now);
        } else {
            self.ui_update_banner(ctx, now);
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
            Cmd::ExportWord => self.export_docx(now),
            Cmd::ExportPdf => self.export_pdf(now),
            Cmd::ExportPng => self.export_png(now),
            Cmd::ToggleSidebar => {
                let open = self.store.settings.sidebar_open;
                self.store.settings.sidebar_open = !open;
            }
            Cmd::Replace => {
                self.dialog = Some(Dialog::Replace {
                    find: String::new(),
                    replace: String::new(),
                });
            }
            Cmd::RevealInExplorer => {
                if let Some(p) = self.active_path() {
                    let _ = std::process::Command::new("explorer")
                        .arg("/select,")
                        .arg(&p)
                        .spawn();
                } else {
                    self.toast("没有打开的文件", now);
                }
            }
            Cmd::Fullscreen => {
                self.fullscreen = !self.fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
            Cmd::FindDoc => self.find_open(ctx),
            Cmd::Print => self.print_doc(now),
            Cmd::InsertTime => {
                let ts = crate::win::local_time_str();
                self.editor_insert_text(ctx, now, &ts);
            }
            Cmd::GotoLine => self.dialog = Some(Dialog::Goto { input: String::new() }),
            Cmd::SaveGbk => self.save_as_gbk(now),
            Cmd::Settings => self.dialog = Some(Dialog::Settings),
            Cmd::ClearRecent => {
                self.store.recent.clear();
                let _ = self.store.save();
                self.toast("已清空最近记录", now);
            }
        }
    }

    /// 内容区右键菜单（阅读 / 编辑两种）
    pub(crate) fn ui_content_menu(&mut self, ctx: &egui::Context, now: f64) {
        let mut frame_resp: Option<egui::InnerResponse<()>> = None;
        let Some((is_editor, pos)) = self.content_menu else {
            self.content_menu_rect = None;
            self.content_menu_fresh = false;
            return;
        };
        let pal = self.pal.clone();
        let mut action: Option<u8> = None;
        let mut export: Option<u8> = None;
        let big = self
            .tab()
            .map(|t| t.text.len() > crate::ui::vpreview::VIRTUAL_THRESHOLD)
            .unwrap_or(false);
        egui::Area::new(egui::Id::unique("content-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                let fr = widgets::menu_frame(&pal).show(ui, |ui| {
                    // 无头自测标记控件：selftest.rs 用 read_response 断言菜单真实渲染
                    let _ = ui.interact(
                        egui::Rect::from_min_size(
                            ui.min_rect().min,
                            egui::Vec2::splat(2.0),
                        ),
                        egui::Id::unique(crate::selftest::MENU_MARKER),
                        egui::Sense::hover(),
                    );
                    ui.set_width(200.0);
                    if is_editor {
                        // 编辑区菜单（大文档窗口不支持剪切/粘贴/全选——它们作用于窗口文本）
                        if !big {
                            if widgets::menu_item(ui, &pal, Icon::Copy, "剪切  Ctrl+X", false)
                                .clicked()
                            {
                                action = Some(1);
                            }
                            if widgets::menu_item(ui, &pal, Icon::Copy, "复制  Ctrl+C", false)
                                .clicked()
                            {
                                action = Some(2);
                            }
                            if widgets::menu_item(ui, &pal, Icon::FileText, "粘贴  Ctrl+V", false)
                                .clicked()
                            {
                                action = Some(3);
                            }
                            if widgets::menu_item(ui, &pal, Icon::Outline, "全选  Ctrl+A", false)
                                .clicked()
                            {
                                action = Some(4);
                            }
                            widgets::hairline(ui, &pal);
                        } else {
                            ui.label(
                                RichText::new("超大文档：请在窗口内选择后用 Ctrl+C 复制")
                                    .size(10.5)
                                    .color(pal.text_faint),
                            );
                            widgets::hairline(ui, &pal);
                        }
                        if widgets::menu_item(ui, &pal, Icon::Clock, "插入日期时间  F5", false)
                            .clicked()
                        {
                            action = Some(5);
                        }
                        if widgets::menu_item(ui, &pal, Icon::Search, "在文档中查找  Ctrl+F", false)
                            .clicked()
                        {
                            action = Some(6);
                        }
                        if widgets::menu_item(ui, &pal, Icon::Outline, "跳转到行  Ctrl+G", false)
                            .clicked()
                        {
                            action = Some(7);
                        }
                    } else {
                        // 阅读区菜单
                        ui.add(
                            egui::Label::new(
                                RichText::new("选中文字后按 Ctrl+C 复制")
                                    .size(10.5)
                                    .color(pal.text_faint),
                            )
                            .selectable(false), // 菜单文字不可选中（商业软件惯例）
                        );
                        if widgets::menu_item(ui, &pal, Icon::Copy, "复制全文（Markdown）", false)
                            .clicked()
                        {
                            action = Some(8);
                        }
                        if widgets::menu_item(ui, &pal, Icon::Search, "在文档中查找  Ctrl+F", false)
                            .clicked()
                        {
                            action = Some(6);
                        }
                        if widgets::menu_item(ui, &pal, Icon::Outline, "跳转到行  Ctrl+G", false)
                            .clicked()
                        {
                            action = Some(7);
                        }
                        if widgets::menu_item(ui, &pal, Icon::ChevronRight, "回到顶部", false)
                            .clicked()
                        {
                            action = Some(9);
                        }
                        widgets::hairline(ui, &pal);
                        let fav = self
                            .active_path()
                            .map(|p| self.store.favorites.contains(&p.to_string_lossy().to_string()))
                            .unwrap_or(false);
                        let fav_label = if fav { "取消收藏当前文档" } else { "收藏当前文档" };
                        if widgets::menu_item(ui, &pal, Icon::Check, fav_label, false).clicked() {
                            action = Some(10);
                        }
                        if widgets::menu_item(ui, &pal, Icon::Refresh, "打印…  Ctrl+P", false)
                            .clicked()
                        {
                            action = Some(11);
                        }
                        widgets::hairline(ui, &pal);
                        if widgets::menu_item(ui, &pal, Icon::FileText, "导出为 HTML", false)
                            .clicked()
                        {
                            export = Some(1);
                        }
                        if widgets::menu_item(ui, &pal, Icon::FileText, "导出为 Word", false)
                            .clicked()
                        {
                            export = Some(2);
                        }
                        if widgets::menu_item(ui, &pal, Icon::FileText, "导出为 PDF", false)
                            .clicked()
                        {
                            export = Some(3);
                        }
                        if widgets::menu_item(ui, &pal, Icon::Image, "导出为长图 PNG", false)
                            .clicked()
                        {
                            export = Some(4);
                        }
                    }
                });
                frame_resp = Some(fr);
            });
        // 记下菜单矩形：判断「点击是否落在菜单上」用
        // （不能用 is_pointer_over_egui：整个窗口都是 egui，恒为真，
        //   曾导致点菜单外关不掉、菜单一直停留）
        if let Some(fr) = frame_resp {
            self.content_menu_rect = Some(fr.response.rect);
        }
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        let any_click = ctx.input(|i| i.pointer.any_click());
        // hit 判定加 2pt 容差：egui 渲染 Area 时把位置取整到物理像素，
        // 指针坐标不取整——松开在 x.5/y.5+ 的位置时菜单 rect 边界可能落在
        // 指针外侧零点几 pt，此时「打开菜单的那次点击」会被误判成菜单外点击
        let inside_menu = match (self.content_menu_rect, pointer) {
            (Some(r), Some(p)) => r.expand(2.0).contains(p),
            _ => false,
        };
        // fresh：打开当帧的 any_click 就是打开菜单的那次点击，不是关闭手势
        if any_click && !inside_menu && !self.content_menu_fresh {
            self.content_menu = None;
        }
        self.content_menu_fresh = false;
        // 滚轮滚动时关闭（Win11 行为）
        if ctx.input(|i| i.smooth_scroll_delta.y.abs() > 0.0) {
            self.content_menu = None;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.content_menu = None;
        }
        if let Some(a) = action {
            self.content_menu = None;
            match a {
                1 => self.editor_cut(ctx, now),
                2 => self.editor_copy_sel(ctx, now),
                3 => self.editor_paste(ctx, now),
                4 => self.editor_select_all(ctx),
                5 => {
                    let ts = crate::win::local_time_str();
                    self.editor_insert_text(ctx, now, &ts);
                }
                6 => self.find_open(ctx),
                7 => self.dialog = Some(Dialog::Goto { input: String::new() }),
                8 => self.copy_all_md(now),
                9 => {
                    if let Some(t) = self.tab_mut() {
                        t.restore_read = Some(0.0);
                    }
                }
                10 => self.toggle_favorite(now),
                11 => self.print_doc(now),
                _ => {}
            }
        }
        if let Some(e) = export {
            self.content_menu = None;
            match e {
                1 => self.export_html(now),
                2 => self.export_docx(now),
                3 => self.export_pdf(now),
                4 => self.export_png(now),
                _ => {}
            }
        }
    }

    /// 标签右键菜单：关闭 / 关闭其他 / 关闭右侧
    pub(crate) fn ui_tab_menu(&mut self, ctx: &egui::Context, now: f64) {
        let Some((idx, pos)) = self.tab_menu else {
            self.tab_menu_rect = None;
            self.tab_menu_fresh = false;
            return;
        };
        let pal = self.pal.clone();
        let mut action: Option<u8> = None;
        let mut frame_rect: Option<egui::Rect> = None;
        egui::Area::new(egui::Id::new("tab-menu"))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let fr = widgets::menu_frame(&pal).show(ui, |ui| {
                    ui.set_width(170.0);
                    if widgets::menu_item(ui, &pal, Icon::Close, "关闭  Ctrl+W", false).clicked() {
                        action = Some(1);
                    }
                    if widgets::menu_item(ui, &pal, Icon::Close, "关闭其他", false).clicked() {
                        action = Some(2);
                    }
                    if widgets::menu_item(ui, &pal, Icon::Close, "关闭右侧", false).clicked() {
                        action = Some(3);
                    }
                });
                frame_rect = Some(fr.response.rect);
            });
        self.tab_menu_rect = frame_rect;
        // 点外关闭：rect 判定 + fresh 跳过打开当帧 + 2pt 容差（同 ui_content_menu）
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        let inside = match (self.tab_menu_rect, pointer) {
            (Some(r), Some(p)) => r.expand(2.0).contains(p),
            _ => false,
        };
        if ctx.input(|i| i.pointer.any_click()) && !inside && !self.tab_menu_fresh {
            self.tab_menu = None;
        }
        self.tab_menu_fresh = false;
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.tab_menu = None;
        }
        match action {
            Some(1) => {
                self.tab_menu = None;
                self.close_tab(idx, now);
            }
            Some(2) => {
                self.tab_menu = None;
                for i in (0..self.tabs.len()).rev() {
                    if i != idx {
                        self.close_tab(i, now);
                    }
                }
            }
            Some(3) => {
                self.tab_menu = None;
                for i in ((idx + 1)..self.tabs.len()).rev() {
                    self.close_tab(i, now);
                }
            }
            _ => {}
        }
    }

    /// 文档内查找浮条（记事本式，右上角）
    fn ui_find_bar(&mut self, ctx: &egui::Context, now: f64) {
        let Some(mut f) = self.find.clone() else { return };
        let pal = self.pal.clone();
        let mut action: Option<u8> = None; // 1=下一个 2=上一个 3=关闭 4=切换大小写
        let screen = ctx.input(|i| i.viewport().outer_rect.unwrap_or(egui::Rect::NOTHING));
        egui::Area::new(egui::Id::unique("find-bar"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::Pos2::new(
                (screen.max.x - 500.0).max(screen.min.x + 12.0),
                screen.min.y + 52.0,
            ))
            .show(ctx, |ui| {
                widgets::menu_frame(&pal).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if widgets::text_button(ui, &pal, if f.case_sensitive { "Aa" } else { "aa" }, f.case_sensitive)
                            .on_hover_text("区分大小写")
                            .clicked()
                        {
                            action = Some(4);
                        }
                        let resp = ui.add(
                            TextEdit::singleline(&mut f.query)
                                .hint_text(RichText::new("在文档中查找…").color(pal.text_faint))
                                .desired_width(200.0)
                                .margin(Margin::symmetric(8, 5)),
                        );
                        if resp.changed() {
                            f.idx = 0;
                        }
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            action = Some(1);
                        }
                        let total = f.matches.len();
                        let cur = if total == 0 { 0 } else { f.idx + 1 };
                        ui.label(
                            RichText::new(format!("{cur}/{total}"))
                                .size(12.0)
                                .family(pal.mono())
                                .color(if total == 0 { pal.text_faint } else { pal.accent }),
                        );
                        if widgets::text_button(ui, &pal, "↑", false)
                            .on_hover_text("上一个  Shift+F3")
                            .clicked()
                        {
                            action = Some(2);
                        }
                        if widgets::text_button(ui, &pal, "↓", false)
                            .on_hover_text("下一个  F3 / Enter")
                            .clicked()
                        {
                            action = Some(1);
                        }
                        if widgets::text_button(ui, &pal, "✕", false).clicked() {
                            action = Some(3);
                        }
                    });
                });
            });
        // Esc 关闭
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            action = Some(3);
        }
        match action {
            Some(1) => {
                self.find = Some(f);
                self.find_move(ctx, 1);
            }
            Some(2) => {
                self.find = Some(f);
                self.find_move(ctx, -1);
            }
            Some(3) => self.find = None,
            Some(4) => {
                f.case_sensitive = !f.case_sensitive;
                self.find = Some(f);
            }
            _ => {
                self.find = Some(f);
            }
        }
        let _ = now;
    }

    // -- 右键菜单 ---------------------------------------------------------

    fn ui_ctx_menu(&mut self, ctx: &egui::Context, now: f64) {
        let pal = self.pal.clone();
        let Some(menu) = self.menu.take() else {
            self.ctx_menu_rect = None;
            self.ctx_menu_fresh = false;
            return;
        };
        let mut keep = true;
        let mut action: Option<&'static str> = None;
        let mut frame_rect: Option<egui::Rect> = None;

        egui::Area::new(Id::unique("ctx-menu"))
            .order(egui::Order::Tooltip)
            .fixed_pos(menu.pos)
            .show(ctx, |ui| {
                let fr = widgets::menu_frame(&pal).show(ui, |ui| {
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
                frame_rect = Some(fr.response.rect);
            });
        self.ctx_menu_rect = frame_rect;

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            keep = false;
        }
        // 点外关闭：rect 判定（is_pointer_over_egui 整窗恒真，曾导致永远关不掉），
        // fresh 跳过打开当帧，expand 2pt 吸收渲染取整偏移
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        let inside = match (self.ctx_menu_rect, pointer) {
            (Some(r), Some(p)) => r.expand(2.0).contains(p),
            _ => false,
        };
        if ctx.input(|i| i.pointer.any_click()) && !inside && !self.ctx_menu_fresh {
            keep = false;
        }
        self.ctx_menu_fresh = false;

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

