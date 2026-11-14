//! 弹窗：新建、重命名、删除确认、首选项、关于。

use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Key, Layout, Margin, RichText, Sense, TextEdit, Ui,
    Vec2,
};
use mojuan_core::Theme;

use crate::app::{App, Dialog};
use crate::theme::{alpha, Palette};
use crate::ui::widgets;

impl App {
    pub(crate) fn ui_dialog(&mut self, ctx: &egui::Context, now: f64) {
        let Some(dlg) = self.dialog.take() else { return };
        let pal = self.pal.clone();
        let mut close = false;
        let mut keep: Option<Dialog> = None;

        match dlg {
            Dialog::NewFile { dir, mut name } => {
                let mut submit = false;
                let mut cancel = false;
                widgets::modal(ctx, &pal, "new-file", 400.0, |ui| {
                    dialog_title(ui, &pal, "新建文件", "会在当前目录创建一个新的 Markdown 文件");
                    ui.add_space(10.0);
                    let resp = ui.add(
                        TextEdit::singleline(&mut name)
                            .desired_width(f32::INFINITY)
                            .margin(Margin::symmetric(8, 6)),
                    );
                    if !name.is_empty() {
                        resp.request_focus();
                    }
                    enter_pressed(ui, &resp, &mut submit);
                    ui.add_space(14.0);
                    buttons(ui, &pal, "创建", false, &mut submit, &mut cancel);
                });
                if submit {
                    self.create_file(&dir, &name, now);
                    close = true;
                } else if cancel {
                    close = true;
                } else {
                    keep = Some(Dialog::NewFile { dir, name });
                }
            }
            Dialog::NewFolder { dir, mut name } => {
                let mut submit = false;
                let mut cancel = false;
                widgets::modal(ctx, &pal, "new-folder", 400.0, |ui| {
                    dialog_title(ui, &pal, "新建文件夹", "");
                    ui.add_space(10.0);
                    let resp = ui.add(
                        TextEdit::singleline(&mut name)
                            .desired_width(f32::INFINITY)
                            .margin(Margin::symmetric(8, 6)),
                    );
                    if !name.is_empty() {
                        resp.request_focus();
                    }
                    enter_pressed(ui, &resp, &mut submit);
                    ui.add_space(14.0);
                    buttons(ui, &pal, "创建", false, &mut submit, &mut cancel);
                });
                if submit {
                    self.create_folder(&dir, &name, now);
                    close = true;
                } else if cancel {
                    close = true;
                } else {
                    keep = Some(Dialog::NewFolder { dir, name });
                }
            }
            Dialog::Rename { path, mut name } => {
                let mut submit = false;
                let mut cancel = false;
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();
                widgets::modal(ctx, &pal, "rename", 400.0, |ui| {
                    dialog_title(ui, &pal, "重命名", "");
                    ui.add_space(10.0);
                    let resp = ui.add(
                        TextEdit::singleline(&mut name)
                            .desired_width(f32::INFINITY)
                            .margin(Margin::symmetric(8, 6)),
                    );
                    if !name.is_empty() {
                        resp.request_focus();
                    }
                    enter_pressed(ui, &resp, &mut submit);
                    ui.add_space(14.0);
                    buttons(ui, &pal, "确定", false, &mut submit, &mut cancel);
                });
                let _ = ext;
                if submit {
                    self.rename(&path, &name, now);
                    close = true;
                } else if cancel {
                    close = true;
                } else {
                    keep = Some(Dialog::Rename { path, name });
                }
            }
            Dialog::Delete { path, is_dir } => {
                let mut cancel = false;
                let mut submit = false;
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                widgets::modal(ctx, &pal, "delete", 420.0, |ui| {
                    dialog_title(
                        ui,
                        &pal,
                        if is_dir { "删除文件夹" } else { "删除文件" },
                        "",
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!("「{name}」将被移入系统回收站，可以再恢复。"))
                            .size(12.5)
                            .color(pal.text_dim),
                    );
                    ui.add_space(16.0);
                    buttons(ui, &pal, "移到回收站", true, &mut submit, &mut cancel);
                });
                if submit {
                    self.delete(&path, now);
                    close = true;
                } else if cancel {
                    close = true;
                } else {
                    keep = Some(Dialog::Delete { path, is_dir });
                }
            }
            Dialog::Settings => {
                let mut cancel = false;
                let mut submit = false;
                let mut fs = self.store.settings.font_size;
                let mut measure = self.store.settings.measure;
                let mut lh = self.store.settings.line_spacing;
                let mut autosave = self.store.settings.autosave;
                let mut check_updates = self.store.settings.check_updates;
                let mut repo = self.store.settings.update_repo.clone();
                let mut do_check = false;
                let mut theme_idx = match self.store.settings.theme {
                    Theme::Dark => 0,
                    Theme::Light => 1,
                };
                widgets::modal(ctx, &pal, "settings", 460.0, |ui| {
                    dialog_title(ui, &pal, "首选项", "");
                    ui.add_space(12.0);
                    row_label(ui, &pal, "主题");
                    if widgets::segmented(ui, &pal, &mut theme_idx, &["暗色", "亮色"]) {
                        let t = if theme_idx == 0 { Theme::Dark } else { Theme::Light };
                        self.pending_theme = Some(t);
                    }
                    ui.add_space(12.0);
                    row_label(ui, &pal, "正文字号");
                    ui.add(
                        egui::Slider::new(&mut fs, 12.0..=26.0)
                            .suffix(" px")
                            .fixed_decimals(0),
                    );
                    ui.add_space(10.0);
                    row_label(ui, &pal, "阅读栏宽");
                    ui.add(
                        egui::Slider::new(&mut measure, 560.0..=980.0)
                            .suffix(" px")
                            .fixed_decimals(0),
                    );
                    ui.add_space(10.0);
                    row_label(ui, &pal, "行距");
                    ui.add(
                        egui::Slider::new(&mut lh, 1.4..=2.2)
                            .fixed_decimals(2)
                            .suffix(" 倍"),
                    );
                    ui.add_space(12.0);
                    ui.checkbox(&mut autosave, "编辑时自动保存");

                    ui.add_space(14.0);
                    widgets::hairline(ui, &pal);
                    ui.add_space(12.0);
                    row_label(ui, &pal, "更新");
                    ui.checkbox(&mut check_updates, "启动时检查新版本");
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add(
                            TextEdit::singleline(&mut repo)
                                .hint_text("更新源：owner/repo")
                                .desired_width(240.0)
                                .margin(Margin::symmetric(8, 5)),
                        );
                        if widgets::text_button(ui, &pal, "立即检查", false).clicked() {
                            do_check = true;
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("留空则使用程序内置的更新源")
                            .size(10.5)
                            .color(pal.text_faint),
                    );

                    ui.add_space(16.0);
                    buttons(ui, &pal, "完成", false, &mut submit, &mut cancel);
                });
                self.store.settings.font_size = fs;
                self.store.settings.measure = measure;
                self.store.settings.line_spacing = lh;
                self.store.settings.autosave = autosave;
                self.store.settings.check_updates = check_updates;
                self.store.settings.update_repo = repo.clone();
                let _ = self.store.save();
                if do_check {
                    let r = self.updater.repo(&repo);
                    self.updater.check(r, now);
                    self.updater.dialog_open = true;
                }
                if submit || cancel {
                    close = true;
                } else {
                    keep = Some(Dialog::Settings);
                }
            }
            Dialog::About => {
                let mut cancel = false;
                let mut submit = false;
                widgets::modal(ctx, &pal, "about", 460.0, |ui| {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(36.0, 36.0), Sense::hover());
                        ui.painter().rect_filled(r, CornerRadius::same(10), pal.accent);
                        let g = ui.painter().layout_no_wrap(
                            "墨".to_string(),
                            FontId::new(19.0, pal.bold()),
                            Color32::from_rgb(0xFD, 0xF8, 0xF3),
                        );
                        ui.painter().galley(r.center() - g.size() / 2.0, g, pal.text);
                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!("墨卷 v{}", mojuan_core::VERSION))
                                    .size(15.0)
                                    .family(pal.bold())
                                    .color(pal.text),
                            );
                            ui.label(
                                RichText::new("纯 Rust 的 Markdown 阅读与编辑器")
                                    .size(11.5)
                                    .color(pal.text_dim),
                            );
                        });
                    });
                    ui.add_space(14.0);
                    widgets::hairline(ui, &pal);
                    ui.add_space(10.0);
                    for (k, v) in [
                        ("Ctrl+O", "打开文件"),
                        ("Ctrl+Shift+O", "打开文件夹作为工作区"),
                        ("Ctrl+P", "快速打开 / 命令面板"),
                        ("Ctrl+S", "保存"),
                        ("Ctrl+E", "切换 阅读 / 分屏 / 编辑"),
                        ("Ctrl+B / Ctrl+I", "加粗 / 斜体"),
                        ("Ctrl+F", "侧栏搜索"),
                        ("Ctrl+W", "关闭当前标签"),
                        ("Ctrl+1 / 2 / 3", "字号增减"),
                    ] {
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [110.0, 18.0],
                                egui::Label::new(
                                    RichText::new(k)
                                        .size(11.0)
                                        .family(pal.mono())
                                        .color(pal.text_dim),
                                ),
                            );
                            ui.label(RichText::new(v).size(12.0).color(pal.text_dim));
                        });
                    }
                    ui.add_space(14.0);
                    buttons(ui, &pal, "关闭", false, &mut submit, &mut cancel);
                });
                if submit || cancel {
                    close = true;
                } else {
                    keep = Some(Dialog::About);
                }
            }
        }

        if !close {
            self.dialog = keep;
        }
        if let Some(t) = self.pending_theme.take() {
            self.set_theme(ctx, t);
        }
    }
}

fn dialog_title(ui: &mut Ui, pal: &Palette, title: &str, subtitle: &str) {
    ui.label(
        RichText::new(title)
            .size(15.5)
            .family(pal.bold())
            .color(pal.text),
    );
    if !subtitle.is_empty() {
        ui.add_space(3.0);
        ui.label(RichText::new(subtitle).size(11.5).color(pal.text_faint));
    }
}

fn row_label(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.label(RichText::new(text).size(12.0).color(pal.text_dim));
}

fn enter_pressed(ui: &Ui, resp: &egui::Response, submit: &mut bool) {
    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
        *submit = true;
    }
}

fn buttons(
    ui: &mut Ui,
    pal: &Palette,
    ok_label: &str,
    danger: bool,
    submit: &mut bool,
    cancel: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let ok_text = RichText::new(ok_label)
                .size(12.5)
                .family(pal.bold())
                .color(Color32::from_rgb(0xFD, 0xF7, 0xF2));
            let fill = if danger { pal.danger } else { pal.accent };
            let btn = egui::Button::new(ok_text)
                .fill(fill)
                .corner_radius(CornerRadius::same(6))
                .min_size(Vec2::new(72.0, 30.0));
            if ui.add(btn).clicked() {
                *submit = true;
            }
            if widgets::text_button(ui, pal, "取消", false).clicked() {
                *cancel = true;
            }
            let _ = alpha(pal.text, 1);
        });
    });
}
