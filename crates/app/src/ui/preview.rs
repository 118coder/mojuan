//! 阅读区渲染：把 core 的文档模型画成排版良好的文章。
//!
//! 设计目标：中文行距舒适、标题层级清晰、代码块可复制、表格自适应、
//! 链接可点（支持相对路径跳转到工作区里的其它文档）。

use std::path::PathBuf;

use eframe::egui::{
    self, text::LayoutJob, Align, Color32, CornerRadius, CursorIcon, FontId, Frame, Label, Layout,
    Margin, Pos2, Rect, RichText, Sense, Stroke, TextFormat, Ui, Vec2,
};
use mojuan_core::{Align as TAlign, Block, Doc, ListItem, Run};

use crate::icons::{self, Icon};
use crate::theme::{alpha, Palette, R_MD, R_SM};

pub enum Action {
    Link(String),
    Copy(String),
}

pub struct Opts {
    pub size: f32,
    pub line_height: f32,
    pub measure: f32,
    pub compact: bool,
}

struct Ctx<'a> {
    pal: &'a Palette,
    opts: &'a Opts,
    acts: Vec<Action>,
    imgs: &'a mut crate::img::Loader,
}

pub fn render(
    ui: &mut Ui,
    pal: &Palette,
    doc: &Doc,
    opts: &Opts,
    pending: &mut Option<String>,
    imgs: &mut crate::img::Loader,
) -> Vec<Action> {
    let total_w = ui.available_width();
    let col_w = if opts.compact {
        (total_w - 48.0).max(240.0)
    } else {
        opts.measure.min((total_w - 48.0).max(280.0))
    };
    let pad = ((total_w - col_w) * 0.5).max(24.0);

    let mut cx = Ctx {
        pal,
        opts,
        acts: Vec::new(),
        imgs,
    };

    ui.horizontal_top(|ui| {
        ui.add_space(pad);
        ui.vertical(|ui| {
            ui.set_width(col_w);
            if doc.blocks.is_empty() {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("空白文档")
                        .size(13.0)
                        .color(pal.text_faint),
                );
            }
            blocks(ui, &mut cx, &doc.blocks, pending, 0, false);
        });
        ui.add_space(pad);
    });

    cx.acts
}

fn blocks(
    ui: &mut Ui,
    cx: &mut Ctx,
    blocks: &[Block],
    pending: &mut Option<String>,
    depth: usize,
    dim: bool,
) {
    for b in blocks {
        block(ui, cx, b, pending, depth, dim);
    }
}

fn block(
    ui: &mut Ui,
    cx: &mut Ctx,
    b: &Block,
    pending: &mut Option<String>,
    depth: usize,
    dim: bool,
) {
    let pal = cx.pal;
    let size = cx.opts.size;
    let lh = cx.opts.line_height;

    match b {
        Block::Heading {
            level,
            runs,
            id,
            ..
        } => {
            let (hsize, top, bottom) = match level {
                1 => (size * 1.92, 10.0, 14.0),
                2 => (size * 1.42, 30.0, 10.0),
                3 => (size * 1.18, 24.0, 8.0),
                4 => (size * 1.04, 20.0, 6.0),
                5 => (size * 0.98, 16.0, 4.0),
                _ => (size * 0.94, 14.0, 4.0),
            };
            ui.add_space(top);
            let color = if *level >= 5 { pal.text_dim } else { pal.text };
            let resp = inline(
                ui,
                cx,
                runs,
                hsize,
                1.32,
                color,
                true,
                *level == 1,
            );
            if *level <= 2 {
                let rect = resp.rect;
                let y = rect.max.y + if *level == 1 { 10.0 } else { 7.0 };
                let w = if *level == 1 { rect.width() } else { (rect.width() * 0.32).min(72.0) };
                let col = if *level == 1 {
                    alpha(pal.text, if pal.dark { 54 } else { 34 })
                } else {
                    alpha(pal.accent, 90)
                };
                ui.painter().rect_filled(
                    Rect::from_min_size(Pos2::new(rect.min.x, y), Vec2::new(w, 1.0)),
                    0,
                    col,
                );
                ui.add_space(if *level == 1 { 14.0 } else { 10.0 });
            }
            if pending.as_deref() == Some(id.as_str()) {
                let scroll = Rect::from_min_size(
                    rect_top_left(&resp),
                    Vec2::new(resp.rect.width(), resp.rect.height()),
                );
                ui.scroll_to_rect(scroll.expand2(Vec2::new(0.0, 36.0)), Some(Align::TOP));
                *pending = None;
            }
            ui.add_space(bottom);
        }
        Block::Para(runs) => {
            inline(ui, cx, runs, size, lh, pal.text, false, false);
            ui.add_space(size * 0.72);
        }
        Block::Code { lang, lines } => {
            code_block(ui, cx, lang, lines);
            ui.add_space(size * 0.7);
        }
        Block::Quote(inner) => {
            let frame = Frame::new().inner_margin(Margin {
                left: 16,
                right: 0,
                top: 2,
                bottom: 2,
            });
            let resp = frame.show(ui, |ui| {
                ui.set_width(ui.available_width() - 16.0);
                blocks(ui, cx, inner, pending, depth + 1, true);
            });
            let rect = resp.response.rect;
            ui.painter().rect_filled(
                Rect::from_min_size(
                    Pos2::new(rect.min.x, rect.min.y + 2.0),
                    Vec2::new(2.5, (rect.height() - 4.0).max(4.0)),
                ),
                2,
                alpha(pal.quote_bar, if pal.dark { 200 } else { 160 }),
            );
            ui.add_space(size * 0.7);
        }
        Block::List {
            ordered,
            start,
            items,
        } => {
            list(ui, cx, *ordered, *start, items, pending, depth, dim);
            ui.add_space(size * 0.5);
        }
        Block::Table { aligns, head, rows } => {
            table(ui, cx, aligns, head, rows);
            ui.add_space(size * 0.8);
        }
        Block::Rule => {
            ui.add_space(size * 0.6);
            let (rect, _) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), 1.0),
                Sense::hover(),
            );
            ui.painter().rect_filled(rect, 0, pal.border);
            ui.add_space(size * 0.9);
        }
        Block::Image {
            path,
            src,
            alt,
            title,
        } => {
            ui.add_space(size * 0.3);
            image_block(ui, cx, path.as_deref(), src, alt);
            if !title.is_empty() {
                ui.add_space(6.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(title)
                            .size(size * 0.78)
                            .color(pal.text_faint),
                    );
                });
            }
            ui.add_space(size * 0.8);
        }
    }
}

fn image_block(
    ui: &mut Ui,
    cx: &mut Ctx,
    path: Option<&std::path::Path>,
    src: &str,
    alt: &str,
) {
    let pal = cx.pal;
    let size = cx.opts.size;
    let max_w = ui.available_width();
    let max_h = (ui.ctx().viewport_rect().height() * 0.72).max(220.0);

    let Some(path) = path else {
        // 远程图片：给个可点的提示
        image_notice(ui, cx, Icon::External, if alt.is_empty() { src } else { alt });
        return;
    };

    let tex = cx
        .imgs
        .get(ui.ctx(), path)
        .map(|t| (t.id(), t.size_vec2(), t.size_vec2()));

    match tex {
        Some((id, natural, _)) => {
            let scale = (max_w / natural.x)
                .min(max_h / natural.y)
                .min(1.0)
                .max(0.05);
            let draw = Vec2::new(natural.x * scale, natural.y * scale);
            ui.vertical_centered(|ui| {
                let img = egui::Image::new(egui::load::SizedTexture::new(id, natural))
                    .fit_to_exact_size(draw)
                    .corner_radius(CornerRadius::same(R_MD));
                let resp = ui.add(img);
                ui.painter().rect_stroke(
                    resp.rect,
                    CornerRadius::same(R_MD),
                    Stroke::new(1.0, alpha(pal.text, 22)),
                    egui::StrokeKind::Inside,
                );
            });
        }
        None => {
            if cx.imgs.is_pending(path) {
                let h = (max_w * 0.3).clamp(80.0, 220.0);
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(max_w.min(max_w), h),
                    Sense::hover(),
                );
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::same(R_SM),
                    alpha(pal.text, 8),
                );
                let g = ui.painter().layout_no_wrap(
                    "图片加载中…".to_string(),
                    FontId::new(size * 0.78, egui::FontFamily::Proportional),
                    pal.text_faint,
                );
                ui.painter()
                    .galley(rect.center() - g.size() / 2.0, g, pal.text_faint);
            } else {
                let msg = cx
                    .imgs
                    .failure(path)
                    .cloned()
                    .unwrap_or_else(|| "无法显示这张图片".to_string());
                image_notice(ui, cx, Icon::Image, &format!("{}（{}）", msg, src));
            }
        }
    }
}

fn image_notice(ui: &mut Ui, cx: &Ctx, icon: Icon, text: &str) {
    let pal = cx.pal;
    let w = ui.available_width().min(460.0);
    Frame::new()
        .fill(alpha(pal.text, 8))
        .stroke(Stroke::new(1.0, pal.border_soft))
        .corner_radius(CornerRadius::same(R_SM))
        .inner_margin(Margin::symmetric(14, 14))
        .show(ui, |ui| {
            ui.set_width(w - 28.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::hover());
                icons::paint(ui.painter(), r.center(), 15.0, pal.text_faint, icon, pal.panel);
                ui.add_space(2.0);
                ui.label(
                    RichText::new(text)
                        .size(cx.opts.size * 0.78)
                        .color(pal.text_faint),
                );
            });
        });
}

fn rect_top_left(resp: &egui::Response) -> Pos2 {
    resp.rect.min
}

// ---------------------------------------------------------------------------
// 行内文本
// ---------------------------------------------------------------------------

struct LinkSpan {
    start: usize,
    end: usize,
    url: String,
}

fn build_job(
    cx: &Ctx,
    runs: &[Run],
    size: f32,
    line_height: f32,
    base_color: Color32,
    wrap: f32,
) -> (LayoutJob, Vec<LinkSpan>) {
    let pal = cx.pal;
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let mut links: Vec<LinkSpan> = Vec::new();
    let mut cc = 0usize;

    for run in runs {
        let mut fmt = TextFormat {
            font_id: FontId::new(size, egui::FontFamily::Proportional),
            color: base_color,
            ..Default::default()
        };
        if run.bold {
            fmt.font_id = FontId::new(size, pal.bold());
        }
        if run.italic {
            fmt.italics = true;
        }
        if run.strike {
            fmt.strikethrough = Stroke::new(1.0, alpha(base_color, 180));
        }
        if run.code {
            fmt.font_id = FontId::new(size * 0.92, pal.mono());
            fmt.background = pal.inline_code_bg;
            fmt.color = if pal.dark { pal.link } else { pal.accent };
        }
        if run.link.is_some() && run.image.is_none() {
            fmt.color = pal.link;
            fmt.underline = Stroke::new(1.0, alpha(pal.link, 110));
        }
        if run.image.is_some() {
            fmt.color = pal.text_dim;
            fmt.italics = true;
        }
        fmt.line_height = Some(size * line_height);
        job.append(&run.text, 0.0, fmt);

        let n = run.text.chars().count();
        if let Some(url) = &run.link {
            links.push(LinkSpan {
                start: cc,
                end: cc + n,
                url: url.clone(),
            });
        }
        cc += n;
    }

    (job, links)
}

#[allow(clippy::too_many_arguments)]
fn inline(
    ui: &mut Ui,
    cx: &mut Ctx,
    runs: &[Run],
    size: f32,
    line_height: f32,
    color: Color32,
    force_bold: bool,
    wide: bool,
) -> egui::Response {
    let wrap = ui.available_width();
    let effective: Vec<Run> = if force_bold {
        runs.iter()
            .map(|r| Run {
                bold: true,
                ..r.clone()
            })
            .collect()
    } else {
        runs.to_vec()
    };

    let (job, links) = build_job(cx, &effective, size, line_height, color, wrap);
    let resp = ui.add(Label::new(job).selectable(true));

    if !links.is_empty() {
        let hit = |pos: Pos2| -> Option<String> {
            let (job2, _) = build_job(cx, &effective, size, line_height, color, wrap);
            let galley = ui.ctx().fonts_mut(|f| f.layout_job(job2));
            let cur = galley.cursor_from_pos(pos - resp.rect.min);
            let idx = cur.index.0;
            links
                .iter()
                .find(|l| idx >= l.start && idx < l.end)
                .map(|l| l.url.clone())
        };
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                if let Some(url) = hit(pos) {
                    cx.acts.push(Action::Link(url));
                }
            }
        } else if resp.hovered() {
            if let Some(pos) = ui.ctx().pointer_hover_pos() {
                if hit(pos).is_some() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
            }
        }
    }

    let _ = wide;
    resp
}

// ---------------------------------------------------------------------------
// 代码块
// ---------------------------------------------------------------------------

fn code_block(ui: &mut Ui, cx: &mut Ctx, lang: &str, lines: &[Vec<mojuan_core::highlight::Token>]) {
    let pal = cx.pal;
    let size = cx.opts.size;
    let avail = ui.available_width();

    let mut plain = String::new();
    for line in lines {
        for t in line {
            plain.push_str(&t.text);
        }
    }

    let inner_w = avail - 32.0;
    // 估算最长行宽，决定是否需要横向滚动
    let max_w = {
        let font = FontId::new(size * 0.88, pal.mono());
        let mut m: f32 = 0.0;
        for line in lines {
            let text: String = line.iter().map(|t| t.text.as_str()).collect();
            let w = ui
                .painter()
                .layout_no_wrap(text, font.clone(), pal.text)
                .size()
                .x;
            m = m.max(w);
        }
        m
    };

    Frame::new()
        .fill(pal.code_bg)
        .stroke(Stroke::new(1.0, pal.code_border))
        .corner_radius(CornerRadius::same(R_MD))
        .inner_margin(Margin {
            left: 14,
            right: 10,
            top: 8,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.set_width(avail - 24.0);
            // 顶部信息条
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(lang)
                        .size(10.5)
                        .family(pal.mono_bold())
                        .color(pal.text_faint),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let copy = ui.add(
                        egui::Button::new(
                            RichText::new("复制").size(11.0).color(pal.text_dim),
                        )
                        .frame(false)
                        .small(),
                    );
                    if copy.clicked() {
                        cx.acts.push(Action::Copy(plain.clone()));
                        ui.ctx().copy_text(plain.clone());
                    }
                });
            });
            ui.add_space(2.0);

            let body = |ui: &mut Ui| {
                let mut job = LayoutJob::default();
                job.wrap.max_width = f32::INFINITY;
                for line in lines {
                    for t in line {
                        let fmt = TextFormat {
                            font_id: FontId::new(size * 0.88, pal.mono()),
                            color: pal.tokens.color(t.style),
                            ..Default::default()
                        };
                        job.append(&t.text, 0.0, fmt);
                    }
                }
                let label = Label::new(job)
                    .selectable(true)
                    .wrap_mode(egui::TextWrapMode::Extend);
                ui.add(label);
            };

            if max_w > inner_w {
                egui::ScrollArea::horizontal()
                    .id_salt(("code", plain.len(), max_w as i32))
                    .auto_shrink([false, true])
                    .show(ui, body);
            } else {
                body(ui);
            }
        });
}

// ---------------------------------------------------------------------------
// 列表
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn list(
    ui: &mut Ui,
    cx: &mut Ctx,
    ordered: bool,
    start: u64,
    items: &[ListItem],
    pending: &mut Option<String>,
    depth: usize,
    dim: bool,
) {
    let pal = cx.pal;
    let size = cx.opts.size;
    let row_h = size * cx.opts.line_height;

    for (i, item) in items.iter().enumerate() {
        let marker_w = 20.0;
        ui.horizontal_top(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(marker_w, row_h), Sense::hover());
            match item.task {
                Some(done) => {
                    let box_size = 13.0;
                    let b = Rect::from_min_size(
                        Pos2::new(rect.min.x + 2.0, rect.min.y + (row_h - box_size) * 0.5 - 1.0),
                        Vec2::splat(box_size),
                    );
                    if done {
                        ui.painter().rect_filled(
                            b,
                            CornerRadius::same(4),
                            alpha(pal.check, 230),
                        );
                        icons::paint(
                            ui.painter(),
                            b.center(),
                            11.0,
                            Color32::from_rgb(0x14, 0x12, 0x10),
                            Icon::Check,
                            pal.panel,
                        );
                    } else {
                        ui.painter().rect_stroke(
                            b,
                            CornerRadius::same(4),
                            Stroke::new(1.3, pal.text_faint),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
                None => {
                    let text = if ordered {
                        format!("{}.", start + i as u64)
                    } else {
                        match depth % 3 {
                            0 => "•".to_string(),
                            1 => "◦".to_string(),
                            _ => "▪".to_string(),
                        }
                    };
                    let color = if dim { pal.text_dim } else { pal.text_dim };
                    let g = ui.painter().layout_no_wrap(
                        text,
                        FontId::new(size * 0.92, egui::FontFamily::Proportional),
                        color,
                    );
                    ui.painter().galley(
                        Pos2::new(rect.max.x - g.size().x - 6.0, rect.min.y + (row_h - g.size().y) * 0.5),
                        g,
                        color,
                    );
                }
            }

            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                for (bi, b) in item.blocks.iter().enumerate() {
                    let is_last = bi + 1 == item.blocks.len();
                    if let Block::Para(runs) = b {
                        inline(
                            ui,
                            cx,
                            runs,
                            size,
                            cx.opts.line_height,
                            if dim { pal.text_dim } else { pal.text },
                            false,
                            false,
                        );
                        if !is_last {
                            ui.add_space(size * 0.2);
                        }
                    } else {
                        block(ui, cx, b, pending, depth + 1, dim);
                    }
                }
            });
        });
        ui.add_space(size * 0.12);
    }
}

// ---------------------------------------------------------------------------
// 表格
// ---------------------------------------------------------------------------

fn cell_text(runs: &[Run]) -> String {
    runs.iter().map(|r| r.text.as_str()).collect()
}

fn table(ui: &mut Ui, cx: &mut Ctx, aligns: &[TAlign], head: &[Vec<Run>], rows: &[Vec<Vec<Run>>]) {
    let pal = cx.pal;
    let size = cx.opts.size;
    let cols = head
        .len()
        .max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
    if cols == 0 {
        return;
    }
    let avail = ui.available_width();
    let pad_x = 12.0;
    let pad_y = 8.0;
    let fs = size * 0.92;

    // 1. 估算每列的自然宽度
    let font = FontId::new(fs, egui::FontFamily::Proportional);
    let bold = FontId::new(fs, pal.bold());
    let mut natural = vec![64.0f32; cols];
    let mut measure = |cells: &[Vec<Run>], is_head: bool| {
        for (i, c) in cells.iter().enumerate() {
            if i >= cols {
                break;
            }
            let text = cell_text(c);
            let w = ui
                .painter()
                .layout_no_wrap(
                    text,
                    if is_head { bold.clone() } else { font.clone() },
                    pal.text,
                )
                .size()
                .x
                + pad_x * 2.0;
            natural[i] = natural[i].max(w.min(avail * 0.6));
        }
    };
    measure(head, true);
    for r in rows {
        measure(r, false);
    }

    let total: f32 = natural.iter().sum();
    let widths: Vec<f32> = if total <= avail {
        let extra = (avail - total) / cols as f32;
        natural.iter().map(|w| w + extra).collect()
    } else {
        let k = avail / total;
        natural.iter().map(|w| (w * k).max(56.0)).collect()
    };

    // 2. 逐行排版，算出每行高度
    let mut layouts: Vec<(bool, Vec<Option<std::sync::Arc<egui::Galley>>>)> = Vec::new();
    let mut heights: Vec<f32> = Vec::new();
    let all_rows = std::iter::once((true, head)).chain(rows.iter().map(|r| (false, r.as_slice())));
    for (is_head, cells) in all_rows {
        let mut galleys = Vec::with_capacity(cols);
        let mut h = fs * 1.5;
        for i in 0..cols {
            let runs = cells.get(i).cloned().unwrap_or_default();
            let w = (widths[i] - pad_x * 2.0).max(24.0);
            let (mut job, _) = build_job(
                cx,
                &runs,
                fs,
                1.55,
                pal.text,
                w,
            );
            if is_head {
                for sec in &mut job.sections {
                    sec.format.font_id = FontId::new(fs, pal.bold());
                }
            }
            let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
            h = h.max(galley.size().y);
            galleys.push(Some(galley));
        }
        layouts.push((is_head, galleys));
        heights.push(h + pad_y * 2.0);
    }

    let total_h: f32 = heights.iter().sum();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(avail, total_h), Sense::hover());
    let painter = ui.painter().clone();

    // 3. 表头底色
    let mut y = rect.min.y;
    for (idx, (is_head, galleys)) in layouts.iter().enumerate() {
        let h = heights[idx];
        if *is_head {
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(rect.min.x, y), Vec2::new(avail, h)),
                0,
                pal.table_head,
            );
        }
        // 单元格文字
        let mut x = rect.min.x;
        for (i, g) in galleys.iter().enumerate() {
            let w = widths[i];
            if let Some(g) = g {
                let align = aligns.get(i).copied().unwrap_or(TAlign::None);
                let tx = match align {
                    TAlign::Center => x + (w - g.size().x) * 0.5,
                    TAlign::Right => x + w - pad_x - g.size().x,
                    _ => x + pad_x,
                };
                painter.galley(
                    Pos2::new(tx.max(x + pad_x), y + pad_y),
                    g.clone(),
                    pal.text,
                );
            }
            x += w;
        }
        y += h;
        // 行分隔线
        if idx + 1 < layouts.len() {
            painter.hline(
                rect.x_range(),
                y,
                Stroke::new(1.0, pal.table_line),
            );
        }
    }

    // 4. 竖向分隔线
    let mut x = rect.min.x;
    for i in 0..cols.saturating_sub(1) {
        x += widths[i];
        let top = rect.min.y + if layouts.first().map(|l| l.0).unwrap_or(false) {
            heights[0]
        } else {
            0.0
        };
        painter.line_segment(
            [Pos2::new(x, top), Pos2::new(x, rect.max.y)],
            Stroke::new(1.0, pal.table_line),
        );
    }

    // 5. 外框
    painter.rect_stroke(
        rect,
        CornerRadius::same(R_SM),
        Stroke::new(1.0, pal.table_line),
        egui::StrokeKind::Inside,
    );
}

pub fn open_external(url: &str) {
    let url = url.trim();
    if url.is_empty() {
        return;
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("rundll32")
            .arg("url.dll,FileProtocolHandler")
            .arg(url)
            .spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// 解析链接：返回 (是否外部链接, 本地路径)
pub fn classify_link(base: Option<&std::path::Path>, url: &str) -> (bool, Option<PathBuf>) {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:") {
        return (true, None);
    }
    if lower.starts_with('#') {
        return (false, None);
    }
    let path = mojuan_core::doc::resolve(base, url);
    (false, path)
}
