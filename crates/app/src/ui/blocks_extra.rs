//! 虚拟预览里的特殊块：数学公式、Mermaid 流程图、front matter、HTML。
//!
//! 全部用 egui 自绘，不引入 JS 引擎；解析逻辑在 `mojuan_core::math` / `mermaid`。

use eframe::egui::{
    self, text::LayoutJob, Align, Color32, CornerRadius, FontId, Frame, Margin, Pos2, Rect,
    RichText, Sense, Stroke, TextFormat, Ui, Vec2,
};
use mojuan_core::vindex::VKind;

use crate::icons::{self, Icon};
use crate::theme::{alpha, Palette, R_MD, R_SM};
use crate::ui::preview::{Action, Opts};

pub fn render(
    ui: &mut Ui,
    pal: &Palette,
    opts: &Opts,
    kind: VKind,
    slice: &str,
    imgs: &mut crate::img::Loader,
) -> Vec<Action> {
    match kind {
        VKind::Math => {
            math_block(ui, pal, opts, slice);
            ui.add_space(opts.size * 0.6);
        }
        VKind::Mermaid => {
            mermaid_block(ui, pal, opts, slice);
            ui.add_space(opts.size * 0.6);
        }
        VKind::FrontMatter => {
            front_matter(ui, pal, opts, slice);
            ui.add_space(opts.size * 0.4);
        }
        VKind::Html => {
            ui.add(
                egui::Label::new(
                    RichText::new(slice.trim())
                        .size(opts.size * 0.82)
                        .color(pal.text_faint),
                )
                .wrap(),
            );
            ui.add_space(opts.size * 0.5);
        }
        _ => {}
    }
    let _ = imgs;
    Vec::new()
}

// ---------------------------------------------------------------------------
// 数学公式
// ---------------------------------------------------------------------------

fn math_block(ui: &mut Ui, pal: &Palette, opts: &Opts, slice: &str) {
    let body = strip_fence(slice, "$$");
    render_math_expr(ui, pal, opts, &body);
}

/// 供完整解析路径复用的块级公式入口（expr 已经是 $$ 之间的内容）
pub fn render_math_expr(ui: &mut Ui, pal: &Palette, opts: &Opts, expr: &str) {
    let layout = mojuan_core::math::parse_block(expr);
    let plain = mojuan_core::math::render_unicode(&layout);
    let size = opts.size * 1.12;
    let mut job = LayoutJob::default();
    job.wrap.max_width = ui.available_width() - 36.0;
    for span in &layout.spans {
        if span.text.is_empty() {
            continue;
        }
        let (fs, color, italics, valign) = match span.style {
            mojuan_core::math::MathStyle::Super => (size * 0.72, pal.text_dim, false, Align::TOP),
            mojuan_core::math::MathStyle::Sub => {
                (size * 0.72, pal.text_dim, false, Align::BOTTOM)
            }
            mojuan_core::math::MathStyle::Symbol => (size, pal.accent, false, Align::Center),
            mojuan_core::math::MathStyle::Operator => {
                (size, pal.text_dim, false, Align::Center)
            }
            mojuan_core::math::MathStyle::Bold => (size, pal.text, false, Align::Center),
            mojuan_core::math::MathStyle::Italic => (size, pal.text, true, Align::Center),
            mojuan_core::math::MathStyle::Normal => (size, pal.text, false, Align::Center),
        };
        job.append(
            &span.text,
            0.0,
            TextFormat {
                font_id: FontId::new(fs, egui::FontFamily::Proportional),
                color,
                italics,
                valign,
                ..Default::default()
            },
        );
    }
    if job.is_empty() {
        job.append(
            &plain,
            0.0,
            TextFormat {
                font_id: FontId::new(size, egui::FontFamily::Proportional),
                color: pal.text,
                ..Default::default()
            },
        );
    }

    Frame::new()
        .fill(alpha(pal.accent, if pal.dark { 12 } else { 9 }))
        .stroke(Stroke::new(1.0, pal.border_soft))
        .corner_radius(CornerRadius::same(R_MD))
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add(egui::Label::new(job).selectable(true).wrap());
                ui.add_space(6.0);
                if ui
                    .add(
                        egui::Label::new(
                            RichText::new(format!("复制公式  {}", layout.source))
                                .size(10.0)
                                .color(pal.text_faint),
                        )
                        .sense(Sense::click())
                        .truncate(),
                    )
                    .clicked()
                {
                    ui.ctx().copy_text(layout.source.clone());
                }
            });
        });
}

// ---------------------------------------------------------------------------
// Mermaid 流程图
// ---------------------------------------------------------------------------

fn mermaid_block(ui: &mut Ui, pal: &Palette, opts: &Opts, slice: &str) {
    render_mermaid_source(ui, pal, opts, &strip_fence(slice, "```"));
}

/// 供完整解析路径复用的 Mermaid 入口（src 不含围栏）
pub fn render_mermaid_source(ui: &mut Ui, pal: &Palette, opts: &Opts, src: &str) {
    let Some(graph) = mojuan_core::mermaid::parse(src) else {
        code_notice(ui, pal, opts, src);
        return;
    };

    let avail = ui.available_width() - 4.0;
    let scale = (avail / graph.width.max(1.0)).min(1.0).max(0.25);
    let w = graph.width * scale;
    let h = (graph.height * scale).max(90.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(avail, h), Sense::hover());
    let painter = ui.painter().with_clip_rect(rect);
    let origin = Pos2::new(rect.min.x + (avail - w) * 0.5, rect.min.y + 6.0);

    painter.rect_filled(
        Rect::from_min_size(origin, Vec2::new(w, h - 12.0)),
        CornerRadius::same(R_SM),
        alpha(pal.text, if pal.dark { 7 } else { 5 }),
    );

    let boxes = graph.boxes();
    let node_color = if pal.dark {
        Color32::from_rgb(0x2C, 0x27, 0x24)
    } else {
        Color32::from_rgb(0xFF, 0xFD, 0xF9)
    };
    let line = Stroke::new(1.2 * scale.max(0.6), pal.text_dim);

    // 先画边，节点盖在上面
    for e in &graph.edges {
        let a = graph.nodes.iter().position(|n| n.id == e.from);
        let b = graph.nodes.iter().position(|n| n.id == e.to);
        let (Some(ai), Some(bi)) = (a, b) else { continue };
        // boxes() 给的是左上角 + 宽高，这里换算成中心点
        let (_, ax, ay, aw, ah) = boxes[ai];
        let (_, bx, by, bw, bh) = boxes[bi];
        let p1 = origin + Vec2::new((ax + aw * 0.5) * scale, (ay + ah * 0.5) * scale);
        let p2 = origin + Vec2::new((bx + bw * 0.5) * scale, (by + bh * 0.5) * scale);
        // 简单直线 + 箭头
        let dir = (p2 - p1).normalized();
        let start = p1 + dir * (18.0 * scale);
        let end = p2 - dir * (18.0 * scale);
        painter.line_segment([start, end], line);
        let left = Pos2::new(
            end.x - dir.x * 8.0 + dir.y * 4.0,
            end.y - dir.y * 8.0 - dir.x * 4.0,
        );
        let right = Pos2::new(
            end.x - dir.x * 8.0 - dir.y * 4.0,
            end.y - dir.y * 8.0 + dir.x * 4.0,
        );
        painter.line_segment([left, end], line);
        painter.line_segment([right, end], line);
    }

    for (ni, x, y, bw, bh) in boxes {
        let node = &graph.nodes[ni];
        let r = Rect::from_min_size(
            origin + Vec2::new(x * scale, y * scale),
            Vec2::new(bw * scale, bh * scale),
        );
        let bg = if matches!(node.shape, mojuan_core::mermaid::NodeShape::Diamond) {
            alpha(pal.accent, 28)
        } else {
            node_color
        };
        let cr = match node.shape {
            mojuan_core::mermaid::NodeShape::Round
            | mojuan_core::mermaid::NodeShape::Circle => CornerRadius::same(18),
            mojuan_core::mermaid::NodeShape::Diamond => CornerRadius::same(3),
            _ => CornerRadius::same(6),
        };
        painter.rect(r, cr, bg, Stroke::new(1.0, alpha(pal.accent, 150)), egui::StrokeKind::Inside);
        let g = painter.layout_no_wrap(
            node.label.clone(),
            FontId::new((11.0 * scale).max(8.5), egui::FontFamily::Proportional),
            pal.text,
        );
        if g.size().x <= r.width() - 8.0 {
            painter.galley(r.center() - g.size() / 2.0, g, pal.text);
        }
    }
    ui.add_space(opts.size * 0.3);
}

fn code_notice(ui: &mut Ui, pal: &Palette, opts: &Opts, body: &str) {
    Frame::new()
        .fill(alpha(pal.text, 7))
        .stroke(Stroke::new(1.0, pal.border_soft))
        .corner_radius(CornerRadius::same(R_SM))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
                icons::paint(ui.painter(), r.center(), 14.0, pal.text_faint, Icon::Image, pal.panel);
                ui.add(
                    egui::Label::new(
                        RichText::new("这段图暂时无法解析，已按源码显示")
                            .size(opts.size * 0.78)
                            .color(pal.text_faint),
                    )
                    .wrap(),
                );
            });
            ui.add_space(6.0);
            ui.add(
                egui::Label::new(
                    RichText::new(body.trim())
                        .size(opts.size * 0.76)
                        .color(pal.text_dim)
                        .family(pal.mono()),
                )
                .wrap()
                .selectable(true),
            );
        });
}

// ---------------------------------------------------------------------------
// Front matter
// ---------------------------------------------------------------------------

fn front_matter(ui: &mut Ui, pal: &Palette, opts: &Opts, slice: &str) {
    let body = slice.trim().trim_matches('-').trim();
    if body.is_empty() {
        return;
    }
    Frame::new()
        .fill(alpha(pal.text, 7))
        .corner_radius(CornerRadius::same(R_SM))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(body)
                        .size(opts.size * 0.74)
                        .color(pal.text_faint)
                        .family(pal.mono()),
                )
                .wrap()
                .selectable(true),
            );
        });
}

/// 去掉围栏标记，保留中间内容。`fence` 是 "$$" 或 "```"。
fn strip_fence(slice: &str, fence: &str) -> String {
    let one = slice.trim();
    if fence == "$$" && one.starts_with("$$") && one.ends_with("$$") && one.len() >= 4 {
        return one[2..one.len() - 2].trim().to_string();
    }
    let mut lines: Vec<&str> = slice.lines().collect();
    if lines.first().map(|l| l.trim_start().starts_with(fence)).unwrap_or(false) {
        lines.remove(0);
    }
    if lines.last().map(|l| l.trim() == fence).unwrap_or(false) {
        lines.pop();
    }
    // mermaid 的语言标记单独一行
    if fence == "```" {
        if let Some(first) = lines.first() {
            let t = first.trim().to_ascii_lowercase();
            if t.starts_with("mermaid") {
                lines.remove(0);
            }
        }
    }
    lines.join("\n")
}
