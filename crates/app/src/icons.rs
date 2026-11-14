//! 手绘矢量图标。不用图标字体、不依赖 emoji，
//! 保证在任意 DPI 下都清晰，也不会有字体缺字问题。

use eframe::egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Search,
    Sun,
    Moon,
    ChevronRight,
    ChevronDown,
    Folder,
    FileText,
    Plus,
    Close,
    More,
    Trash,
    Pencil,
    Copy,
    Check,
    Refresh,
    Sidebar,
    External,
    Minus,
    Save,
    Outline,
    Clock,
    Layout,
    Image,
}

/// 在 center 处画一个 size x size 的图标
pub fn paint(p: &Painter, center: Pos2, size: f32, color: Color32, icon: Icon, bg: Color32) {
    let s = size / 16.0;
    let pt = |x: f32, y: f32| center + vec2((x - 8.0) * s, (y - 8.0) * s);
    let w = (1.55 * s).max(1.0);
    let st = Stroke::new(w, color);

    match icon {
        Icon::Search => {
            p.circle_stroke(pt(7.0, 7.0), 4.6 * s, st);
            p.line_segment([pt(10.4, 10.4), pt(13.8, 13.8)], st);
        }
        Icon::Sun => {
            p.circle_stroke(pt(8.0, 8.0), 3.3 * s, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (sin, cos) = a.sin_cos();
                p.line_segment(
                    [
                        pt(8.0 + cos * 5.4, 8.0 + sin * 5.4),
                        pt(8.0 + cos * 7.3, 8.0 + sin * 7.3),
                    ],
                    st,
                );
            }
        }
        Icon::Moon => {
            // 用两段圆弧组成月牙
            let outer = pt(8.0, 8.0);
            let mut outer_pts = Vec::new();
            let mut inner_pts = Vec::new();
            let steps = 26;
            for i in 0..=steps {
                let t = -1.2 + (4.6 * (i as f32 / steps as f32));
                outer_pts.push(pos2(outer.x + 5.6 * s * t.cos(), outer.y + 5.6 * s * t.sin()));
            }
            let c2 = pt(11.9, 5.6);
            for i in 0..=steps {
                let t = 3.05 + (3.6 * (i as f32 / steps as f32));
                inner_pts.push(pos2(c2.x + 5.0 * s * t.cos(), c2.y + 5.0 * s * t.sin()));
            }
            let mut pts = outer_pts;
            pts.extend(inner_pts);
            p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
            let _ = bg;
        }
        Icon::ChevronRight => {
            p.line_segment([pt(6.5, 4.0), pt(10.5, 8.0)], st);
            p.line_segment([pt(10.5, 8.0), pt(6.5, 12.0)], st);
        }
        Icon::ChevronDown => {
            p.line_segment([pt(4.0, 6.5), pt(8.0, 10.5)], st);
            p.line_segment([pt(8.0, 10.5), pt(12.0, 6.5)], st);
        }
        Icon::Folder => {
            // 带小页签的文件夹轮廓
            let radius = 2.0 * s;
            let top = 4.4;
            let bottom = 12.6;
            let left = 2.2;
            let right = 13.8;
            let mut pts = Vec::new();
            // 左上页签
            pts.push(pt(left + radius / s * 0.0, bottom));
            pts.push(pt(left, bottom));
            pts.push(pt(left, top + 1.4));
            pts.push(pt(left + 1.2, top));
            pts.push(pt(left + 5.6, top));
            pts.push(pt(left + 6.8, top + 1.6));
            pts.push(pt(right - 1.2, top + 1.6));
            pts.push(pt(right, top + 2.8));
            pts.push(pt(right, bottom));
            pts.push(pt(left, bottom));
            p.add(Shape::line(pts, st));
        }
        Icon::FileText => {
            let r = Rect::from_min_max(pt(3.6, 2.2), pt(12.4, 13.8));
            rounded_stroke(p, r, 2.0 * s, st);
            p.line_segment([pt(5.8, 6.4), pt(10.2, 6.4)], st);
            p.line_segment([pt(5.8, 9.0), pt(10.2, 9.0)], st);
            p.line_segment([pt(5.8, 11.4), pt(8.6, 11.4)], st);
        }
        Icon::Plus => {
            p.line_segment([pt(8.0, 4.0), pt(8.0, 12.0)], st);
            p.line_segment([pt(4.0, 8.0), pt(12.0, 8.0)], st);
        }
        Icon::Close => {
            p.line_segment([pt(5.0, 5.0), pt(11.0, 11.0)], st);
            p.line_segment([pt(11.0, 5.0), pt(5.0, 11.0)], st);
        }
        Icon::More => {
            for x in [4.4, 8.0, 11.6] {
                p.circle_filled(pt(x, 8.0), 1.15 * s, color);
            }
        }
        Icon::Trash => {
            p.line_segment([pt(3.4, 4.8), pt(12.6, 4.8)], st);
            p.line_segment([pt(6.4, 4.8), pt(6.4, 3.0)], st);
            p.line_segment([pt(9.6, 4.8), pt(9.6, 3.0)], st);
            p.line_segment([pt(6.4, 3.0), pt(9.6, 3.0)], st);
            p.line_segment([pt(4.8, 4.8), pt(5.6, 13.2)], st);
            p.line_segment([pt(11.2, 4.8), pt(10.4, 13.2)], st);
            p.line_segment([pt(5.6, 13.2), pt(10.4, 13.2)], st);
        }
        Icon::Pencil => {
            p.line_segment([pt(4.0, 12.0), pt(11.2, 4.8)], st);
            p.line_segment([pt(9.6, 3.2), pt(12.8, 6.4)], st);
            p.line_segment([pt(3.4, 12.6), pt(3.9, 10.6)], st);
        }
        Icon::Copy => {
            let r = Rect::from_min_max(pt(5.6, 5.6), pt(13.2, 13.2));
            rounded_stroke(p, r, 2.0 * s, st);
            p.line_segment([pt(3.0, 10.6), pt(2.8, 3.2)], st);
            p.line_segment([pt(2.8, 3.2), pt(10.4, 3.0)], st);
        }
        Icon::Check => {
            p.line_segment([pt(3.4, 8.4), pt(6.6, 11.6)], st);
            p.line_segment([pt(6.6, 11.6), pt(12.6, 4.6)], st);
        }
        Icon::Refresh => {
            arc(p, pt(8.0, 8.0), 5.0 * s, -1.1, 3.4, st);
            p.line_segment([pt(11.4, 2.6), pt(12.8, 5.4)], st);
            p.line_segment([pt(12.8, 5.4), pt(9.9, 5.9)], st);
        }
        Icon::Sidebar => {
            let r = Rect::from_min_max(pt(2.4, 3.6), pt(13.6, 12.4));
            rounded_stroke(p, r, 2.0 * s, st);
            p.line_segment([pt(6.4, 3.8), pt(6.4, 12.2)], st);
        }
        Icon::External => {
            p.line_segment([pt(9.4, 3.2), pt(12.8, 3.2)], st);
            p.line_segment([pt(12.8, 3.2), pt(12.8, 6.6)], st);
            p.line_segment([pt(12.6, 3.4), pt(7.4, 8.6)], st);
            p.line_segment([pt(11.0, 9.6), pt(11.0, 12.8)], st);
            p.line_segment([pt(11.0, 12.8), pt(3.2, 12.8)], st);
            p.line_segment([pt(3.2, 12.8), pt(3.2, 5.0)], st);
            p.line_segment([pt(3.2, 5.0), pt(6.4, 5.0)], st);
        }
        Icon::Minus => {
            p.line_segment([pt(4.0, 8.0), pt(12.0, 8.0)], st);
        }
        Icon::Save => {
            let r = Rect::from_min_max(pt(3.0, 3.0), pt(13.0, 13.0));
            rounded_stroke(p, r, 2.2 * s, st);
            p.line_segment([pt(6.0, 3.2), pt(6.0, 6.4)], st);
            p.line_segment([pt(6.0, 6.4), pt(10.6, 6.4)], st);
            p.line_segment([pt(10.6, 6.4), pt(10.6, 3.2)], st);
            let inner = Rect::from_min_max(pt(5.4, 9.0), pt(10.6, 12.8));
            rounded_stroke(p, inner, 1.4 * s, st);
        }
        Icon::Outline => {
            p.line_segment([pt(3.4, 4.4), pt(12.6, 4.4)], st);
            p.line_segment([pt(5.6, 8.0), pt(12.6, 8.0)], st);
            p.line_segment([pt(5.6, 11.6), pt(12.6, 11.6)], st);
        }
        Icon::Clock => {
            p.circle_stroke(pt(8.0, 8.0), 5.2 * s, st);
            p.line_segment([pt(8.0, 4.8), pt(8.0, 8.2)], st);
            p.line_segment([pt(8.0, 8.2), pt(10.6, 9.6)], st);
        }
        Icon::Image => {
            let r = Rect::from_min_max(pt(2.6, 3.4), pt(13.4, 12.6));
            rounded_stroke(p, r, 2.0 * s, st);
            p.circle_stroke(pt(5.8, 6.6), 1.2 * s, st);
            p.line_segment([pt(3.4, 11.2), pt(7.0, 7.8)], st);
            p.line_segment([pt(7.0, 7.8), pt(9.6, 10.4)], st);
            p.line_segment([pt(9.6, 10.4), pt(11.2, 8.8)], st);
            p.line_segment([pt(11.2, 8.8), pt(13.2, 11.0)], st);
        }
        Icon::Layout => {
            let r = Rect::from_min_max(pt(2.6, 3.6), pt(13.4, 12.4));
            rounded_stroke(p, r, 2.0 * s, st);
            p.line_segment([pt(8.0, 3.8), pt(8.0, 12.2)], st);
        }
    }
}

fn rounded_stroke(p: &Painter, rect: Rect, r: f32, stroke: Stroke) {
    let r = r.min(rect.width() * 0.5).min(rect.height() * 0.5);
    p.add(Shape::rect_stroke(
        rect,
        eframe::egui::CornerRadius::same(r.round() as u8),
        stroke,
        eframe::egui::StrokeKind::Inside,
    ));
}

fn arc(p: &Painter, center: Pos2, r: f32, from: f32, to: f32, stroke: Stroke) {
    let steps = 22;
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = from + (to - from) * (i as f32 / steps as f32);
        pts.push(pos2(center.x + r * t.cos(), center.y + r * t.sin()));
    }
    p.add(Shape::line(pts, stroke));
}
