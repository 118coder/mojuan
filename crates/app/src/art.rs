//! 程序图标：用代码画出来，不额外带图片资源。
//!
//! 图形含义：朱砂底 + 米白纸张 + 三道文字线。

use eframe::egui;

const ACCENT: [u8; 3] = [0xC4, 0x61, 0x4A];
const PAPER: [u8; 3] = [0xFC, 0xF8, 0xF2];

pub fn app_icon() -> egui::IconData {
    const N: usize = 64;
    let mut pixels = vec![0u8; N * N * 4];

    for y in 0..N {
        for x in 0..N {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;

            // 底色圆角方块
            let mut col = [0.0f32; 3];
            let a = coverage(fx, fy, 32.0, 32.0, 30.0, 30.0, 14.0);
            if a > 0.0 {
                mix(&mut col, ACCENT, a);
            }

            // 纸片
            let paper = coverage(fx, fy, 32.0, 32.0, 17.0, 21.0, 4.0);
            if paper > 0.0 {
                mix_over(&mut col, PAPER, paper);
            }

            // 三道文字线
            for (i, w) in [22.0f32, 22.0, 13.0].iter().enumerate() {
                let ly = 24.0 + i as f32 * 6.0;
                let line = coverage(fx, fy, 22.0 + w * 0.5, ly, w * 0.5, 1.4, 1.2);
                if line > 0.0 {
                    mix_over(&mut col, ACCENT, line * 0.85);
                }
            }

            let idx = (y * N + x) * 4;
            pixels[idx] = col[0].round().clamp(0.0, 255.0) as u8;
            pixels[idx + 1] = col[1].round().clamp(0.0, 255.0) as u8;
            pixels[idx + 2] = col[2].round().clamp(0.0, 255.0) as u8;
            pixels[idx + 3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }

    egui::IconData {
        rgba: pixels,
        width: N as u32,
        height: N as u32,
    }
}

/// 圆角矩形的覆盖率（带 1px 抗锯齿）
fn coverage(x: f32, y: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (x - cx).abs() - (hw - r);
    let qy = (y - cy).abs() - (hh - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    let d = outside + qx.max(qy).min(0.0) - r;
    (0.5 - d).clamp(0.0, 1.0)
}

fn mix(dst: &mut [f32; 3], color: [u8; 3], a: f32) {
    for i in 0..3 {
        dst[i] = dst[i] * (1.0 - a) + color[i] as f32 * a;
    }
}

fn mix_over(dst: &mut [f32; 3], color: [u8; 3], a: f32) {
    for i in 0..3 {
        dst[i] = dst[i] * (1.0 - a) + color[i] as f32 * a;
    }
}
