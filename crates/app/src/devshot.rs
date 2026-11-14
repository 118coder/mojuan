//! 开发用：把窗口内容原样截图到文件（用于检查排版，不依赖系统截屏）。
//!
//! 用法：设置环境变量 `MJ_SHOT=输出路径.ppm` 启动，程序会在第 N 帧后截图。
//! PPM 是无依赖的简单格式，可以用 ffmpeg 转成 PNG 看。

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

static FRAMES: AtomicU32 = AtomicU32::new(0);
static TAKEN: AtomicBool = AtomicBool::new(false);

pub fn tick(ctx: &egui::Context) {
    let Ok(path) = std::env::var("MJ_SHOT") else {
        return;
    };
    let n = FRAMES.fetch_add(1, Ordering::Relaxed);
    if n == 0 {
        // 抢到前台，避免被其它窗口遮挡导致重绘被节流
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::AlwaysOnTop,
        ));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }
    // 等界面（字体、图片加载）稳定后再截
    let wait: u32 = std::env::var("MJ_SHOT_FRAME")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(14);
    // 也可以按秒等（网络请求这类跟帧数无关的等待更可靠）
    let secs: Option<f64> = std::env::var("MJ_SHOT_SECS")
        .ok()
        .and_then(|v| v.parse().ok());
    let ready = match secs {
        Some(s) => ctx.input(|i| i.time) >= s,
        None => n == wait,
    };
    if n < wait + 8 || secs.is_some() {
        ctx.request_repaint();
    }
    if ready && !TAKEN.swap(true, Ordering::Relaxed) {
        let ctx2 = ctx.clone();
        ctx.request_screenshot(move |image| {
            if let Err(e) = write_ppm(&path, &image) {
                eprintln!("screenshot failed: {e}");
            } else {
                eprintln!("screenshot saved: {path}");
            }
            ctx2.send_viewport_cmd(egui::ViewportCommand::Close);
        });
        ctx.request_repaint();
    }
}

fn write_ppm(path: &str, image: &egui::ColorImage) -> std::io::Result<()> {
    let [w, h] = image.size;
    let mut out = Vec::with_capacity(w * h * 3 + 32);
    out.extend_from_slice(format!("P6\n{w} {h}\n255\n").as_bytes());
    for px in &image.pixels {
        out.push(px.r());
        out.push(px.g());
        out.push(px.b());
    }
    std::fs::write(path, out)
}
