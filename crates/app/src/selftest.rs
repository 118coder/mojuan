//! 无头自测模式：`mojuan.exe --selftest`
//!
//! 不开窗口、不进主循环：直接用纯 `egui::Context` 驱动 App 真实布局代码，
//! 按帧注入指针/键盘事件，断言交互的「状态 + 真实渲染」。
//!
//! 为什么强调「真实渲染」：曾经出过状态已设置但菜单没人画出来的 bug
//! （完整界面分支漏调 ui_content_menu），只断言内部状态发现不了它。
//! 所以每一步都同时断言状态位和控件标记（`Id::unique("mojuan-menu-marker")`）。
//!
//! 输出：每步一行 `[SELFTEST] PASS/FAIL …`；全部通过返回 0，任一失败返回 1。
//! `cargo test` 里也会跑同一份脚本（见下方 #[test]）。

use crate::app::App;
use egui::{
    Context, Event, Id, Key, Modifiers, PointerButton, Pos2, Rect, RawInput, ViewportId,
    ViewportInfo, Vec2,
};
use mojuan_core::ViewMode;

/// 右键菜单里的固定标记控件（overlays.rs 渲染），用于断言菜单真的画出来了
pub const MENU_MARKER: &str = "mojuan-menu-marker";

const WIN: Vec2 = Vec2::new(1000.0, 800.0);
/// 右键注入点：窗口中部，肯定落在中央内容区（避开顶部 44px 标签条）
const CLICK: Pos2 = Pos2::new(500.0, 400.0);

const DT: f64 = 1.0 / 30.0;

fn viewport_info() -> ViewportInfo {
    let rect = Rect::from_min_size(Pos2::ZERO, WIN);
    ViewportInfo {
        focused: Some(true),
        inner_rect: Some(rect),
        outer_rect: Some(Rect::from_min_size(Pos2::ZERO, WIN + Vec2::splat(2.0))),
        ..Default::default()
    }
}

fn frame_input(events: Vec<Event>, t: f64) -> RawInput {
    let mut raw = RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, WIN));
    raw.time = Some(t);
    raw.predicted_dt = DT as f32;
    raw.events = events;
    raw.viewports.insert(ViewportId::ROOT, viewport_info());
    raw
}

fn pointer(button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos: CLICK,
        button,
        pressed,
        modifiers: Modifiers::default(),
    }
}

fn esc(pressed: bool) -> Event {
    Event::Key {
        key: Key::Escape,
        physical_key: Some(Key::Escape),
        pressed,
        repeat: false,
        modifiers: Modifiers::default(),
    }
}

/// 无头自测主入口：返回退出码（0 全过 / 1 有失败），不直接 process::exit。
pub fn run_full_selftest() -> i32 {
    println!("[SELFTEST] 墨卷无头自测开始（无窗口，纯 egui 驱动真实布局代码）");

    let ctx = Context::default();
    let mut failures: Vec<String> = Vec::new();

    // 测试夹具：临时 md 文档
    let fixture = std::env::temp_dir().join("mojuan-selftest-fixture.md");
    let _ = std::fs::write(
        &fixture,
        "# 自测文档\n\n正文第一段。\n\n## 小节\n\n- 列表项\n\n```rust\nlet a = 1;\n```\n",
    );

    // 真实构造 App（含字体/主题/设置加载），再固定测试环境，避免本地设置干扰
    let mut app = App::init_with(&ctx);
    app.open_path(fixture.clone());
    app.restore_last_scroll();
    app.minimal = false;
    app.store.settings.mode = ViewMode::Read;
    app.store.settings.check_updates = false;
    app.store.settings.autosave = false;
    app.updater.auto_done = true;

    let mut check = |failures: &mut Vec<String>, name: &str, ok: bool, extra: &str| {
        println!(
            "[SELFTEST] {} {} {}",
            if ok { "PASS" } else { "FAIL" },
            name,
            extra
        );
        if !ok {
            failures.push(format!("{name}: {extra}"));
        }
    };

    // 跑一帧；events 在本帧开头被处理
    let mut t = 0.0f64;
    let mut frame = |app: &mut App, events: Vec<Event>, t: f64| {
        let _ = ctx.run_pass(frame_input(events, t), |ctx| {
            ctx.root_ui(|ui| {
                ui.painter().rect_filled(ui.max_rect(), 0, app.pal.bg);
                app.layout(ui);
            });
        });
    };

    for f in 0..40i32 {
        let events = match f {
            // egui 级右键注入（模拟 winit 上交的事件）：按下 → 释放
            16 => vec![pointer(PointerButton::Secondary, true)],
            17 => vec![pointer(PointerButton::Secondary, false)],
            18 => vec![esc(true)],
            19 => vec![esc(false)],
            // 极简模式：右键 + Esc
            26 => vec![pointer(PointerButton::Secondary, true)],
            27 => vec![pointer(PointerButton::Secondary, false)],
            28 => vec![esc(true)],
            29 => vec![esc(false)],
            _ => Vec::new(),
        };
        frame(&mut app, events, t);
        t += DT;

        match f {
            10 => check(
                &mut failures,
                "夹具加载",
                app.tab().map(|tab| tab.path == fixture).unwrap_or(false),
                &format!("tabs={}", app.tabs.len()),
            ),
            // 右键已在 16/17 帧注入，18 帧断言（此时 Esc 尚未处理完也行，先断状态）
            17 => {
                let opened = app.content_menu.is_some();
                let rendered = ctx.read_response(Id::unique(MENU_MARKER)).is_some();
                check(
                    &mut failures,
                    "完整界面·右键打开",
                    opened,
                    &format!("content_menu.is_some()={opened}"),
                );
                // 关键断言：菜单控件真实出现在了控件树里
                check(
                    &mut failures,
                    "完整界面·菜单真实渲染",
                    rendered,
                    &format!(
                        "read_response(marker)={rendered}（历史 bug：状态置位但没人渲染）"
                    ),
                );
            }
            21 => check(
                &mut failures,
                "完整界面·Esc 关闭",
                app.content_menu.is_none(),
                &format!("content_menu.is_some()={}", app.content_menu.is_some()),
            ),
            24 => {
                // 切极简模式（回归验证：极简分支一直有渲染调用）
                app.minimal = true;
            }
            // 极简模式右键：26 按下 27 释放
            27 => {
                let opened = app.content_menu.is_some();
                let rendered = ctx.read_response(Id::unique(MENU_MARKER)).is_some();
                check(
                    &mut failures,
                    "极简模式·右键打开",
                    opened,
                    &format!("content_menu.is_some()={opened}"),
                );
                check(
                    &mut failures,
                    "极简模式·菜单真实渲染",
                    rendered,
                    &format!("read_response(marker)={rendered}"),
                );
            }
            _ => {}
        }
    }

    // Esc 关闭极简菜单（28/29 帧已注入）
    check(
        &mut failures,
        "极简模式·Esc 关闭",
        app.content_menu.is_none(),
        &format!("content_menu.is_some()={}", app.content_menu.is_some()),
    );

    if failures.is_empty() {
        println!("[SELFTEST] 全部通过 ✓");
        0
    } else {
        println!("[SELFTEST] 失败 {} 项 ✗", failures.len());
        1
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn headless_right_click_menu() {
        assert_eq!(super::run_full_selftest(), 0, "无头右键自测未全部通过");
    }
}
