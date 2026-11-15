//! 字体加载：使用系统里的中文字体，避免把几 MB 字体塞进安装包。
//!
//! egui 没有合成粗体，所以必须单独加载粗体字面（YaHei Bold），
//! 否则标题和 **加粗** 都看不出层次。

use std::sync::Arc;

use eframe::egui;

pub const SANS: &str = "mj-sans";
pub const SANS_BOLD: &str = "mj-sans-bold";
pub const MONO: &str = "mj-mono";
pub const MONO_BOLD: &str = "mj-mono-bold";
pub const EMOJI: &str = "mj-emoji";
/// 数学符号兜底：雅黑缺下标数字（₀₁₃）、₌、ᵢ 这类字形，Segoe UI 全覆盖
pub const FALLBACK: &str = "mj-fallback";

#[allow(dead_code)]
pub struct FontSet {
    pub found_cjk: bool,
    pub sans_name: String,
}

fn windir() -> String {
    std::env::var("WINDIR").unwrap_or_else(|_| "C:/Windows".to_string())
}

fn load_first(paths: &[(&str, u32)]) -> Option<(Vec<u8>, u32)> {
    for (p, idx) in paths {
        if let Ok(bytes) = std::fs::read(p) {
            if bytes.len() > 1024 {
                return Some((bytes, *idx));
            }
        }
    }
    None
}

pub fn install(ctx: &egui::Context) -> FontSet {
    let win = windir();
    let f = |name: &str| format!("{win}/Fonts/{name}");

    // eframe 的 default_fonts 特性已关：不再内嵌 Ubuntu/Noto（省 ~1MB exe），
    // 字形全部来自系统字体；Family 列表在下方从零构建。
    let mut fonts = egui::FontDefinitions::default();
    let mut found_cjk = false;

    // 常规中文无衬线
    let sans = load_first(&[
        (&f("msyh.ttc"), 0),
        (&f("Deng.ttf"), 0),
        (&f("simhei.ttf"), 0),
        (&f("NotoSansSC-VF.ttf"), 0),
    ]);
    let mut sans_name = String::new();
    if let Some((bytes, idx)) = sans {
        let mut fd = egui::FontData::from_owned(bytes);
        fd.index = idx;
        fonts.font_data.insert(SANS.to_owned(), Arc::new(fd));
        sans_name = "Microsoft YaHei".to_string();
        found_cjk = true;
    }

    // 粗体
    let bold = load_first(&[
        (&f("msyhbd.ttc"), 0),
        (&f("Dengb.ttf"), 0),
        (&f("NotoSansSC-VF.ttf"), 0),
    ]);
    if let Some((bytes, idx)) = bold {
        let mut fd = egui::FontData::from_owned(bytes);
        fd.index = idx;
        fonts.font_data.insert(SANS_BOLD.to_owned(), Arc::new(fd));
    }

    // 等宽（代码）：Cascadia Mono 优先
    // Emoji 兜底（表格里的 ✅ ❌ 之类）
    let emoji = load_first(&[(&f("seguiemj.ttf"), 0)]);
    if let Some((bytes, idx)) = emoji {
        let mut fd = egui::FontData::from_owned(bytes);
        fd.index = idx;
        fonts.font_data.insert(EMOJI.to_owned(), Arc::new(fd));
    }

    let mono = load_first(&[
        (&f("CascadiaMono.ttf"), 0),
        (&f("consola.ttf"), 0),
        (&f("CascadiaCode.ttf"), 0),
    ]);
    if let Some((bytes, idx)) = mono {
        let mut fd = egui::FontData::from_owned(bytes);
        fd.index = idx;
        fonts.font_data.insert(MONO.to_owned(), Arc::new(fd));
    }
    let mono_bold = load_first(&[(&f("consolab.ttf"), 0), (&f("CascadiaCode.ttf"), 0)]);
    if let Some((bytes, idx)) = mono_bold {
        let mut fd = egui::FontData::from_owned(bytes);
        fd.index = idx;
        fonts.font_data.insert(MONO_BOLD.to_owned(), Arc::new(fd));
    }

    // 数学符号兜底：雅黑缺 ₀₁₃、₌、ᵢ 这类上下标字形（U+2080 区段只有 2/20），
    // Segoe UI 实测 20/20 全覆盖，挂在字族末位按字形回退
    if let Some((bytes, idx)) = load_first(&[(&f("segoeui.ttf"), 0)]) {
        let mut fd = egui::FontData::from_owned(bytes);
        fd.index = idx;
        fonts.font_data.insert(FALLBACK.to_owned(), Arc::new(fd));
    }

    let mut prop: Vec<String> = Vec::new();
    if found_cjk {
        prop.push(SANS.to_owned());
    }
    if fonts.font_data.contains_key(FALLBACK) {
        prop.push(FALLBACK.to_owned());
    }
    prop.extend(default_family(&fonts, egui::FontFamily::Proportional));
    if fonts.font_data.contains_key(EMOJI) {
        prop.push(EMOJI.to_owned());
    }
    let fallback_bold = fonts.font_data.get(SANS).cloned();
    if !fonts.font_data.contains_key(SANS_BOLD) {
        // 没有粗体字面时，用常规字重兜底
        if let Some(fd) = fallback_bold {
            fonts.font_data.insert(SANS_BOLD.to_owned(), fd);
        }
    }
    let mut sans_bold_list = vec![SANS_BOLD.to_owned(), SANS.to_owned()];
    if fonts.font_data.contains_key(FALLBACK) {
        sans_bold_list.push(FALLBACK.to_owned());
    }
    if fonts.font_data.contains_key(SANS_BOLD) || fonts.font_data.contains_key(SANS) {
        fonts
            .families
            .insert(egui::FontFamily::Name(SANS_BOLD.into()), sans_bold_list);
    }

    let mut mono_list = Vec::new();
    if fonts.font_data.contains_key(MONO) {
        mono_list.push(MONO.to_owned());
    }
    mono_list.extend(default_family(&fonts, egui::FontFamily::Monospace));
    if fonts.font_data.contains_key(SANS) {
        mono_list.push(SANS.to_owned());
    }
    if fonts.font_data.contains_key(EMOJI) {
        mono_list.push(EMOJI.to_owned());
    }
    fonts
        .families
        .insert(egui::FontFamily::Monospace, mono_list.clone());

    let mut mono_bold_list = Vec::new();
    if fonts.font_data.contains_key(MONO_BOLD) {
        mono_bold_list.push(MONO_BOLD.to_owned());
    }
    mono_bold_list.extend(mono_list.iter().cloned());
    fonts
        .families
        .insert(egui::FontFamily::Name(MONO_BOLD.into()), mono_bold_list);

    fonts
        .families
        .insert(egui::FontFamily::Proportional, prop);

    ctx.set_fonts(fonts);

    FontSet {
        found_cjk,
        sans_name,
    }
}

fn default_family(fonts: &egui::FontDefinitions, family: egui::FontFamily) -> Vec<String> {
    fonts
        .families
        .get(&family)
        .cloned()
        .unwrap_or_else(Vec::new)
}
