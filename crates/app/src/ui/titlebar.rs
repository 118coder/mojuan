//! 无边框窗口的自绘标题栏件：右上角窗口三键 + 标题栏拖拽区。
//!
//! 系统边框去掉后（with_decorations(false)）：
//! - 拖动 = `ViewportCommand::StartDrag`（系统模态拖拽，手感与原生一致）
//! - 双击标题栏 = 最大化/还原
//! - 边缘缩放 = app.rs 的 handle_edge_resize（BeginResize 系统模态缩放）
//! - 三键 = 自绘 Win11 记事本样式，关闭键悬停红底

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Ui, Vec2};
use crate::app::App;
use crate::theme::{alpha, Palette};

/// Win11 关闭键悬停色
const CLOSE_HOVER: Color32 = Color32::from_rgb(0xC4, 0x2B, 0x1C);
const BTN_W: f32 = 44.0;

#[derive(Clone, Copy, PartialEq)]
enum Btn {
    Min,
    Max,
    Restore,
    Close,
}

/// 右上角窗口三键。必须加在 right_to_left 布局的最前面（先添加 = 最右）。
pub(crate) fn window_buttons(app: &mut App, ui: &mut Ui, pal: &Palette) {
    if app.fullscreen {
        return;
    }
    let ctx = ui.ctx().clone();
    let maxed = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    let h = ui.available_height().max(1.0);
    let mut action: Option<u8> = None;

    if draw_btn(ui, pal, h, Btn::Close).clicked() {
        action = Some(0);
    }
    if draw_btn(ui, pal, h, if maxed { Btn::Restore } else { Btn::Max }).clicked() {
        action = Some(1);
    }
    if draw_btn(ui, pal, h, Btn::Min).clicked() {
        action = Some(2);
    }

    if let Some(a) = action {
        match a {
            0 => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            1 => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maxed)),
            _ => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
        }
    }
}

fn draw_btn(ui: &mut Ui, pal: &Palette, h: f32, kind: Btn) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(BTN_W, h), Sense::click());
    let hover = resp.hovered();
    let is_close = kind == Btn::Close;
    let bg = if hover {
        if is_close {
            CLOSE_HOVER
        } else {
            alpha(pal.text, if pal.dark { 26 } else { 18 })
        }
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 0, bg);
    }
    let icon_col = if hover && is_close {
        Color32::from_rgb(0xFF, 0xFF, 0xFF)
    } else {
        pal.text_dim
    };
    let c = rect.center();
    let p = ui.painter();
    match kind {
        Btn::Min => {
            p.line_segment([Pos2::new(c.x - 5.0, c.y + 3.0), Pos2::new(c.x + 5.0, c.y + 3.0)], stroke(icon_col));
        }
        Btn::Max => {
            p.rect_stroke(
                Rect::from_min_size(Pos2::new(c.x - 5.0, c.y - 5.0), Vec2::splat(10.0)),
                egui::CornerRadius::ZERO,
                stroke(icon_col),
                egui::StrokeKind::Inside,
            );
        }
        Btn::Restore => {
            // 前后两个叠置方框（Win11 还原图标）
            p.rect_stroke(
                Rect::from_min_size(Pos2::new(c.x - 3.0, c.y - 5.5), Vec2::splat(8.5)),
                egui::CornerRadius::ZERO,
                stroke(icon_col),
                egui::StrokeKind::Inside,
            );
            p.rect_filled(
                Rect::from_min_size(Pos2::new(c.x - 5.5, c.y - 3.0), Vec2::splat(8.5)),
                0.0,
                pal.panel,
            );
            p.rect_stroke(
                Rect::from_min_size(Pos2::new(c.x - 5.5, c.y - 3.0), Vec2::splat(8.5)),
                egui::CornerRadius::ZERO,
                stroke(icon_col),
                egui::StrokeKind::Inside,
            );
        }
        Btn::Close => {
            p.line_segment([Pos2::new(c.x - 4.5, c.y - 4.5), Pos2::new(c.x + 4.5, c.y + 4.5)], stroke(icon_col));
            p.line_segment([Pos2::new(c.x - 4.5, c.y + 4.5), Pos2::new(c.x + 4.5, c.y - 4.5)], stroke(icon_col));
        }
    }
    resp
}

fn stroke(col: Color32) -> egui::Stroke {
    egui::Stroke::new(1.2, col)
}

/// 把一整条栏注册为标题栏拖拽区（先注册 = 底层，后画的按钮优先响应；
/// hover 型 Label 不会挡住拖拽，egui 命中测试只挑可交互控件）。
pub(crate) fn drag_region(app: &mut App, ui: &mut Ui, id: &str) {
    let bar = ui.max_rect();
    let bg = ui.interact(
        bar,
        egui::Id::unique(("titlebar-drag", id)),
        Sense::click_and_drag(),
    );
    let ctx = ui.ctx().clone();
    app.titlebar_track_drag(&ctx, &bg);
}
