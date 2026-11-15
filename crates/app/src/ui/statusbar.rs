//! 底部状态栏：字数、阅读时长、保存状态、光标位置。

use eframe::egui::{self, Align, Layout, Pos2, RichText, Ui};

use crate::app::App;
use crate::theme::alpha;

impl App {
    pub(crate) fn ui_statusbar(&mut self, ui: &mut Ui, now: f64) {
        let pal = self.pal.clone();
        let font_size = self.store.settings.font_size;
        let mut toggle_eol = false;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            if let Some(tab) = self.tab_mut() {
                let stats = tab.doc.stats.clone();
                let dot = |ui: &mut Ui| {
                    let (r, _) = ui.allocate_exact_size(egui::Vec2::new(3.0, 3.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 1.5, pal.text_faint);
                };
                ui.label(
                    RichText::new(format!("{} 字", group(stats.chars)))
                        .size(11.0)
                        .color(pal.text_dim),
                );
                dot(ui);
                ui.label(
                    RichText::new(format!("{} 词", group(stats.words)))
                        .size(11.0)
                        .color(pal.text_dim),
                );
                dot(ui);
                ui.label(
                    RichText::new(format!("约 {} 分钟", stats.minutes))
                        .size(11.0)
                        .color(pal.text_dim),
                );

                // 大文档模式标识：让用户知道当前走的是虚拟滚动
                let big = tab.text.len() > crate::ui::vpreview::VIRTUAL_THRESHOLD;
                let blocks = tab
                    .vpreview
                    .index
                    .as_ref()
                    .map(|i| i.blocks.len())
                    .unwrap_or(0);
                if big {
                    dot(ui);
                    ui.label(
                        RichText::new(format!("虚拟滚动 · {} 块", group(blocks)))
                            .size(10.5)
                            .color(pal.accent),
                    );
                }

                // 阅读进度百分比
                if tab.read_total > tab.read_view + 50.0 && tab.read_view > 1.0 {
                    let frac =
                        (tab.scroll_read / (tab.read_total - tab.read_view)).clamp(0.0, 1.0);
                    dot(ui);
                    ui.label(
                        RichText::new(format!("已读 {:.0}%", frac * 100.0))
                            .size(10.5)
                            .color(pal.text_dim),
                    );
                }

                // 缩放百分比 + 总行数
                let zoom = ((font_size / 16.0) * 100.0).round() as i32;
                let lines = if tab.line_count == 0 {
                    tab.line_count = tab.text.matches('\n').count() + 1;
                    tab.line_count
                } else {
                    tab.line_count
                };
                let dirty = tab.dirty;
                let line = tab.cursor_line;
                let col = tab.cursor_col;
                let path = tab.path.clone();
                let eol = if tab.crlf { "CRLF" } else { "LF" };

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    ui.label(
                        RichText::new("UTF-8")
                            .size(10.5)
                            .family(pal.mono())
                            .color(pal.text_faint),
                    );
                    // 换行符：点击切换 CRLF / LF
                    let eol_resp = ui
                        .label(
                            RichText::new(eol)
                                .size(10.5)
                                .family(pal.mono())
                                .color(pal.text_faint),
                        )
                        .interact(egui::Sense::click());
                    if eol_resp.clicked() {
                        if let Some(t) = self.tab_mut() {
                            let new_crlf = !t.crlf;
                            t.text = if new_crlf {
                                t.text.replace('\n', "\r\n")
                            } else {
                                t.text.replace("\r\n", "\n")
                            };
                            t.crlf = new_crlf;
                            t.rev = t.rev.wrapping_add(1);
                            t.win_valid = false;
                            t.dirty = true;
                            t.needs_parse = true;
                            t.line_count = 0;
                            self.toast("已切换换行符，保存时按新格式写入", now);
                        }
                    }
                    eol_resp.on_hover_text("点击切换换行符 CRLF / LF");
                    // 缩放百分比
                    ui.label(
                        RichText::new(format!("{}%", zoom))
                            .size(10.5)
                            .family(pal.mono())
                            .color(pal.text_dim),
                    )
                    .on_hover_text("字号缩放 · Ctrl+滚轮 / Ctrl+= / Ctrl+- / Ctrl+0");
                    // 总行数
                    ui.label(
                        RichText::new(format!("{} 行", group(lines)))
                            .size(10.5)
                            .color(pal.text_faint),
                    );
                    ui.label(
                        RichText::new(format!("行 {line}, 列 {col}"))
                            .size(10.5)
                            .color(pal.text_faint),
                    );
                    let (text, color) = if dirty {
                        ("未保存", pal.accent)
                    } else {
                        ("已保存", pal.text_faint)
                    };
                    let _ = now;
                    let path_tip = path.to_string_lossy().replace('\\', "/");
                    let (r, _) = ui.allocate_exact_size(
                        egui::Vec2::new(7.0, 7.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().circle_filled(
                        Pos2::new(r.center().x, r.center().y),
                        3.0,
                        if dirty { color } else { alpha(pal.check, 200) },
                    );
                    ui.label(RichText::new(text).size(11.0).color(color))
                        .on_hover_text(path_tip);

                    // 文档类型
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_ascii_uppercase();
                    if !ext.is_empty() {
                        ui.label(
                            RichText::new(ext)
                                .size(10.0)
                                .family(pal.mono_bold())
                                .color(alpha(pal.text_faint, 200)),
                        );
                    }
                });
            } else {
                ui.label(
                    RichText::new("就绪")
                        .size(11.0)
                        .color(pal.text_faint),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |_ui| {});
            }
        });
    if toggle_eol {
        if let Some(t) = self.tab_mut() {
            let new_crlf = !t.crlf;
            t.text = if new_crlf {
                t.text.replace('\n', "\r\n")
            } else {
                t.text.replace("\r\n", "\n")
            };
            t.crlf = new_crlf;
            t.rev = t.rev.wrapping_add(1);
            t.win_valid = false;
            t.dirty = true;
            t.needs_parse = true;
            t.line_count = 0;
            self.toast("已切换换行符，保存时按新格式写入", now);
        }
    }
    }
}


fn group(n: usize) -> String {
    let s = n.to_string();
    let bytes: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for (i, c) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c);
    }
    out
}
