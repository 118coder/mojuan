//! 通用组件：按钮、分段控件、弹窗、提示条、右键菜单。

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Margin, Pos2, Rect, Response,
    RichText, Sense, Stroke, Ui, Vec2,
};

use crate::icons::{self, Icon};
use crate::theme::{alpha, Palette, R_LG, R_MD, R_SM};

pub fn hairline(ui: &mut Ui, pal: &Palette) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, pal.border_soft);
}

/// 图标按钮
pub fn icon_button(ui: &mut Ui, pal: &Palette, icon: Icon, active: bool, tip: &str) -> Response {
    let size = Vec2::new(28.0, 26.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    let bg = if active {
        pal.accent_soft
    } else if hovered {
        alpha(pal.text, if pal.dark { 18 } else { 12 })
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, CornerRadius::same(7), bg);
    }
    let col = if active {
        pal.accent
    } else if hovered {
        pal.text
    } else {
        pal.text_dim
    };
    icons::paint(ui.painter(), rect.center(), 16.0, col, icon, pal.panel);
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if tip.is_empty() {
        resp
    } else {
        resp.on_hover_text(
            RichText::new(tip).size(12.0).color(pal.text),
        )
    }
}

/// 文本按钮
pub fn text_button(ui: &mut Ui, pal: &Palette, label: &str, accent: bool) -> Response {
    let font = FontId::new(13.0, if accent { pal.bold() } else { egui::FontFamily::Proportional });
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, pal.text);
    let pad = Vec2::new(12.0, 6.0);
    let size = galley.size() + pad * 2.0;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    let fill = if accent {
        if hovered {
            pal.accent_hover
        } else {
            pal.accent
        }
    } else if hovered {
        alpha(pal.text, if pal.dark { 20 } else { 14 })
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, CornerRadius::same(R_SM), fill);
    }
    let color = if accent {
        Color32::from_rgb(0xFB, 0xF7, 0xF2)
    } else if hovered {
        pal.text
    } else {
        pal.text_dim
    };
    ui.painter().galley(
        rect.center() - galley.size() / 2.0,
        galley,
        color,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// 圆角标签
#[allow(dead_code)]
pub fn badge(ui: &mut Ui, pal: &Palette, label: &str, color: Color32) -> Response {
    let font = FontId::new(11.0, pal.bold());
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, color);
    let pad = Vec2::new(6.0, 2.0);
    let size = galley.size() + pad * 2.0;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(4), alpha(color, if pal.dark { 26 } else { 22 }));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    resp
}

/// 分段控件：返回是否有变化
pub fn segmented(ui: &mut Ui, pal: &Palette, current: &mut usize, items: &[&str]) -> bool {
    let font = FontId::new(12.5, egui::FontFamily::Proportional);
    let mut widths = Vec::new();
    for it in items {
        let g = ui.painter().layout_no_wrap(it.to_string(), font.clone(), pal.text);
        widths.push(g.size().x + 20.0);
    }
    let total = widths.iter().sum::<f32>() + 4.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total, 26.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(8), alpha(pal.text, if pal.dark { 12 } else { 10 }));

    let mut changed = false;
    let mut x = rect.min.x + 2.0;
    for (i, it) in items.iter().enumerate() {
        let w = widths[i];
        let cell = Rect::from_min_size(
            Pos2::new(x, rect.min.y + 2.0),
            Vec2::new(w, rect.height() - 4.0),
        );
        let id = Id::unique("seg").with((rect.min.x as i32, rect.min.y as i32, i));
        let resp = ui.interact(cell, id, Sense::click());
        if resp.clicked() && *current != i {
            *current = i;
            changed = true;
        }
        if *current == i {
            ui.painter()
                .rect_filled(cell, CornerRadius::same(6), pal.elev);
            ui.painter().rect_stroke(
                cell,
                CornerRadius::same(6),
                Stroke::new(1.0, pal.border),
                egui::StrokeKind::Inside,
            );
        }
        let col = if *current == i {
            pal.text
        } else if resp.hovered() {
            pal.text
        } else {
            pal.text_dim
        };
        let g = ui.painter().layout_no_wrap(it.to_string(), font.clone(), col);
        ui.painter()
            .galley(cell.center() - g.size() / 2.0, g, col);
        x += w;
    }
    changed
}

/// 遮罩 + 居中卡片
pub fn modal<R>(
    ctx: &egui::Context,
    pal: &Palette,
    id: &str,
    width: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let screen = ctx.viewport_rect();
    let mut out = None;

    egui::Area::new(Id::unique(("modal-dim", id)))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .interactable(true)
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
            let _ = resp;
            ui.painter().rect_filled(
                rect,
                0,
                Color32::from_black_alpha(if pal.dark { 120 } else { 70 }),
            );
        });

    egui::Area::new(Id::unique(("modal-card", id)))
        .order(egui::Order::Tooltip)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_max_width(width);
            egui::Frame::new()
                .fill(pal.elev)
                .stroke(Stroke::new(1.0, pal.border))
                .corner_radius(CornerRadius::same(R_LG))
                .inner_margin(Margin::same(20))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 12],
                    blur: 32,
                    spread: 0,
                    color: Color32::from_black_alpha(if pal.dark { 120 } else { 40 }),
                })
                .show(ui, |ui| {
                    ui.set_width(width);
                    out = Some(add(ui));
                });
        });

    out
}

/// 底部提示条
pub fn toast(ctx: &egui::Context, pal: &Palette, text: &str, progress: f32) {
    let screen = ctx.viewport_rect();
    let alpha_f = (progress * 4.0).clamp(0.0, 1.0);
    egui::Area::new(Id::unique("toast"))
        .order(egui::Order::Tooltip)
        .fixed_pos(Pos2::new(screen.center().x, screen.max.y - 64.0))
        .pivot(Align2::CENTER_BOTTOM)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(alpha(pal.elev, (245.0 * alpha_f) as u8))
                .stroke(Stroke::new(1.0, alpha(pal.border, (255.0 * alpha_f) as u8)))
                .corner_radius(CornerRadius::same(R_MD))
                .inner_margin(Margin::symmetric(14, 8))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(text)
                            .size(12.5)
                            .color(alpha(pal.text, (255.0 * alpha_f) as u8)),
                    );
                });
        });
}

/// 空状态
pub fn empty_state(ui: &mut Ui, pal: &Palette, icon: Icon, title: &str, hint: &str) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(64.0, 64.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::same(18), alpha(pal.text, 10));
        icons::paint(ui.painter(), rect.center(), 28.0, pal.text_faint, icon, pal.panel);
        ui.add_space(14.0);
        ui.label(RichText::new(title).size(14.5).family(pal.bold()).color(pal.text_dim));
        if !hint.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(hint).size(12.0).color(pal.text_faint));
        }
    });
}

/// 简易右键菜单项
pub fn menu_item(ui: &mut Ui, pal: &Palette, icon: Icon, label: &str, danger: bool) -> Response {
    let h = 30.0;
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let hovered = resp.hovered();
    if hovered {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 1.0)),
            CornerRadius::same(R_SM),
            alpha(pal.text, if pal.dark { 18 } else { 12 }),
        );
    }
    let col = if danger {
        pal.danger
    } else if hovered {
        pal.text
    } else {
        pal.text_dim
    };
    let icon_pos = Pos2::new(rect.min.x + 18.0, rect.center().y);
    icons::paint(ui.painter(), icon_pos, 14.0, col, icon, pal.panel);
    let g = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::new(13.0, egui::FontFamily::Proportional),
        col,
    );
    ui.painter().galley(
        Pos2::new(rect.min.x + 34.0, rect.center().y - g.size().y / 2.0),
        g,
        col,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// 菜单外框
pub fn menu_frame(pal: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(pal.elev)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(CornerRadius::same(R_MD))
        .inner_margin(Margin::symmetric(2, 4))
        .shadow(egui::epaint::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(if pal.dark { 110 } else { 40 }),
        })
}
