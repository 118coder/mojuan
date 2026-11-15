//! 大文档编辑窗口：只把屏幕附近的几百行交给 egui 排版。
//!
//! egui 的 `TextEdit` 会把整篇文本排成一个 Galley；千万字符会产生上千万字形，
//! 内存和耗时都不可接受。这里用「滑动窗口」把 TextEdit 绑定到一小段文本：
//!
//! - `line_starts` 缓存每行起始字节（文本变动时重建一次，线性扫描）；
//! - 根据滚动偏移算出该显示哪一段，前后各留缓冲行；
//! - 编辑时只把窗口内容写回大文本的对应区间，是全量语法着色的 1/1000 开销；
//! - 窗口随滚动自动换页，编辑体验与整篇编辑一致（IME、撤销、选区都在窗口内可用）。

use eframe::egui::{self, Frame, Margin, RichText, ScrollArea, TextEdit, Ui};

use crate::app::App;
use crate::ui::md_highlight;

const BACK_LINES: usize = 48;
const WINDOW_LINES: usize = 260;

fn line_starts(text: &str) -> Vec<usize> {
    let mut v = Vec::with_capacity(text.len() / 48 + 1);
    v.push(0);
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            v.push(i + 1);
        }
    }
    v
}

/// 把窗口里编辑过的内容写回整篇文本，并保持窗口区间有效。
fn flush(tab: &mut crate::app::Tab) -> bool {
    if !tab.win_valid {
        return false;
    }
    let start = tab.win_start.min(tab.text.len());
    let end = tab.win_end.min(tab.text.len()).max(start);
    // 边界必须落在 char boundary 上
    let start = floor_char(&tab.text, start);
    let end = floor_char(&tab.text, end);
    if tab.text.get(start..end) == Some(tab.win_text.as_str()) {
        return false;
    }
    tab.text.replace_range(start..end, &tab.win_text);
    tab.win_end = start + tab.win_text.len();
    true
}

fn floor_char(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

pub(crate) fn render(app: &mut App, ui: &mut Ui, split: bool, now: f64) {
    let pal = app.pal.clone();
    let fs = app.store.settings.font_size;
    let id = app.editor_id();
    let measure = app.store.settings.measure;
    let avail_w = ui.available_width();
    let col_w = if split {
        (avail_w - 64.0).clamp(240.0, 900.0)
    } else {
        measure.min((avail_w - 160.0).max(320.0))
    };
    let line_h = fs * 1.72;

    let wrap_on = app.store.settings.editor_wrap;
    let idx = app.active;
    let tab = &mut app.tabs[idx];

    // 1) 行索引（只在文本修订后重建）
    if tab.line_starts_rev != tab.rev || tab.line_starts.is_empty() {
        tab.line_starts = line_starts(&tab.text);
        tab.line_starts_rev = tab.rev;
    }
    let total_lines = tab.line_starts.len().max(1);

    // 2) 根据滚动位置决定是否换页
    let scroll_line = (tab.scroll_edit / line_h) as usize;
    let need_move = !tab.win_valid
        || scroll_line < tab.win_first_line.saturating_add(BACK_LINES / 2)
        || scroll_line > tab.win_first_line + WINDOW_LINES.saturating_sub(BACK_LINES);
    if need_move {
        flush(tab);
        let first = scroll_line.saturating_sub(BACK_LINES).min(total_lines - 1);
        let last = (first + WINDOW_LINES).min(total_lines);
        let start = tab.line_starts[first];
        let end = if last < total_lines {
            tab.line_starts[last]
        } else {
            tab.text.len()
        };
        tab.win_start = start;
        tab.win_end = end;
        tab.win_text = tab.text.get(start..end).unwrap_or("").to_string();
        tab.win_first_line = first;
        tab.win_valid = true;
    }

    let restore = tab.restore_edit.take();
    let mut area = ScrollArea::vertical()
        .id_salt(("edit-big", &tab.path))
        .auto_shrink([false, false]);
    if let Some(r) = restore {
        area = area.vertical_scroll_offset(r);
    }

    let mut changed = false;
    let win_first_line = tab.win_first_line;
    let out = area.show_viewport(ui, |ui, _viewport| {
        ui.set_height(total_lines as f32 * line_h + 140.0);
        ui.set_width(avail_w);
        ui.add_space(18.0 + win_first_line as f32 * line_h);

        ui.horizontal_top(|ui| {
            let pad = ((ui.available_width() - col_w) * 0.5).max(20.0);
            ui.add_space(pad);
            ui.vertical(|ui| {
                ui.set_width(col_w);
                let plain = tab.plain;
                let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                    // txt 保持纯文本：不做 Markdown 着色
                    let mut job = if plain {
                        let mut j = egui::text::LayoutJob::default();
                        j.append(
                            buf.as_str(),
                            0.0,
                            egui::TextFormat::simple(egui::FontId::monospace(fs), pal.text),
                        );
                        j.wrap.max_width = wrap;
                        j
                    } else {
                        md_highlight::job(buf.as_str(), &pal, fs, wrap)
                    };
                    if !wrap_on {
                        job.wrap.max_width = f32::INFINITY;
                    }
                    ui.ctx().fonts_mut(|f| f.layout_job(job))
                };
                let resp = ui.add(
                    TextEdit::multiline(&mut tab.win_text)
                        .id(id)
                        .frame(Frame::NONE)
                        .desired_width(col_w)
                        .margin(Margin::same(0))
                        .hint_text(RichText::new("开始写点什么…").color(pal.text_faint))
                        .layouter(&mut layouter),
                );
                changed = resp.changed();
            });
        });
        // 底部占位由 set_height 保证，滚动条长度才正确
    });

    tab.scroll_edit = out.state.clamped_offset().y;

    if changed {
        if flush(tab) {
            tab.rev = tab.rev.wrapping_add(1);
            tab.line_starts_rev = 0;
        }
        tab.dirty = true;
        tab.needs_parse = true;
        tab.last_edit = now;
    }

    // 3) 光标行列（窗口内偏移 + 窗口首行）
    let ctx = ui.ctx().clone();
    if let Some(state) = egui::text_edit::TextEditState::load(&ctx, id) {
        if let Some(range) = state.cursor.char_range() {
            let ci = range.primary.index.0;
            let byte = tab
                .win_text
                .char_indices()
                .nth(ci)
                .map(|(i, _)| i)
                .unwrap_or(tab.win_text.len());
            let before = &tab.win_text[..byte];
            let line = win_first_line + before.matches('\n').count() + 1;
            let col = before.rsplit('\n').next().map(|s| s.chars().count()).unwrap_or(0) + 1;
            tab.cursor_line = line;
            tab.cursor_col = col;
        }
    }

    // 4) 搜索跳转
    if let Some(line) = app.pending_line.take() {
        let target = (line.saturating_sub(4) as f32) * line_h;
        let t = &mut app.tabs[idx];
        t.restore_edit = Some(target);
        t.scroll_edit = target;
        t.win_valid = false;
        ctx.request_repaint();
    }
}
