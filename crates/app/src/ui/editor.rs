//! 编辑区：等宽/中文正文字体 + Markdown 内联着色 + 居中排版。

use eframe::egui::{self, Frame, Margin, RichText, ScrollArea, TextEdit, Ui};

use crate::app::App;
use crate::icons::Icon;
use crate::ui::{md_highlight, widgets};

impl App {
    pub(crate) fn ui_editor(&mut self, ui: &mut Ui, split: bool, now: f64) {
        let pal = self.pal.clone();
        let fs = self.store.settings.font_size;
        let id = self.editor_id();

        ui.painter()
            .rect_filled(ui.available_rect_before_wrap(), 0, pal.edit_bg);

        if self.tabs.get(self.active).is_none() {
            widgets::empty_state(ui, &pal, Icon::FileText, "还没有打开文档", "左侧选择文件，或按 Ctrl+N 新建");
            return;
        }

        let measure = self.store.settings.measure;
        let avail_w = ui.available_width();
        let col_w = if split {
            (avail_w - 64.0).clamp(240.0, 900.0)
        } else {
            measure.min((avail_w - 160.0).max(320.0))
        };

        let tab = &mut self.tabs[self.active];
        let restore = tab.restore_edit.take();
        let mut area = ScrollArea::vertical()
            .id_salt(("edit-scroll", &tab.path))
            .auto_shrink([false, false]);
        if let Some(r) = restore {
            area = area.vertical_scroll_offset(r);
        }

        let mut changed = false;
        let out = area.show(ui, |ui| {
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                let pad = ((ui.available_width() - col_w) * 0.5).max(20.0);
                ui.add_space(pad);
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                        let job = md_highlight::job(buf.as_str(), &pal, fs, wrap);
                        ui.ctx().fonts_mut(|f| f.layout_job(job))
                    };
                    let resp = ui.add(
                        TextEdit::multiline(&mut tab.text)
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
            ui.add_space(120.0);
        });

        tab.scroll_edit = out.state.clamped_offset().y;

        if changed {
            tab.dirty = true;
            tab.needs_parse = true;
            tab.last_edit = now;
        }

        // 光标行列
        let ctx = ui.ctx().clone();
        if let Some(state) = egui::text_edit::TextEditState::load(&ctx, id) {
            if let Some(range) = state.cursor.char_range() {
                let idx = range.primary.index;
                let text = &self.tabs[self.active].text;
                let byte = text
                    .char_indices()
                    .nth(idx.0)
                    .map(|(i, _)| i)
                    .unwrap_or(text.len());
                let before = &text[..byte];
                let line = before.matches('\n').count() + 1;
                let col = before
                    .rsplit('\n')
                    .next()
                    .map(|s| s.chars().count())
                    .unwrap_or(0)
                    + 1;
                let tab = &mut self.tabs[self.active];
                tab.cursor_line = line;
                tab.cursor_col = col;
            }
        }

        // 跳转到指定行（搜索结果）
        if let Some(line) = self.pending_line.take() {
            let row_h = fs * 1.72;
            let target = (line.saturating_sub(4) as f32) * row_h;
            if let Some(tab) = self.tabs.get_mut(self.active) {
                tab.restore_edit = Some(target);
                tab.scroll_edit = target;
            }
            ctx.request_repaint();
        }

    }
}
