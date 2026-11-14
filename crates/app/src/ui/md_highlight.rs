//! 编辑器里的 Markdown 内联着色（轻量、逐行扫描，不追求完整语法树）。
//!
//! 这里只改颜色 / 字重 / 字号，不改文字内容，所以光标位置依然精确。

use eframe::egui::{
    text::LayoutJob, Color32, FontId, Stroke, TextFormat,
};

use crate::theme::{alpha, Palette};

#[derive(Clone, Copy)]
struct Sty {
    color: Color32,
    bold: bool,
    italic: bool,
    strike: bool,
    mono: bool,
    scale: f32,
}

impl Sty {
    fn base(pal: &Palette) -> Self {
        Self {
            color: pal.text,
            bold: false,
            italic: false,
            strike: false,
            mono: false,
            scale: 1.0,
        }
    }
}

pub fn job(text: &str, pal: &Palette, size: f32, wrap: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;

    if text.len() > 500_000 {
        // 超大文件不做着色，保证输入不卡
        let fmt = fmt_of(pal, size, Sty::base(pal));
        job.append(text, 0.0, fmt);
        return job;
    }

    let mut in_fence: Option<String> = None;

    for line in text.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = content.trim_start();
        let indent = content.len() - trimmed.len();

        // 代码围栏
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let fence = trimmed.chars().take(3).collect::<String>();
            if in_fence.is_some() {
                in_fence = None;
            } else {
                in_fence = Some(fence);
            }
            push_line_marker(&mut job, &content[..indent], pal, size);
            let mut fmt = fmt_of(pal, size, Sty::base(pal));
            fmt.font_id = FontId::new(size * 0.94, pal.mono());
            fmt.color = pal.accent;
            fmt.background = pal.code_bg;
            job.append(&content[indent..], 0.0, fmt);
            append_nl(&mut job, pal, size, pal.code_bg);
            continue;
        }

        if let Some(_fence) = &in_fence {
            let mut fmt = fmt_of(pal, size, Sty::base(pal));
            fmt.font_id = FontId::new(size * 0.94, pal.mono());
            fmt.color = pal.text_dim;
            fmt.background = pal.code_bg;
            job.append(content, 0.0, fmt);
            append_nl(&mut job, pal, size, pal.code_bg);
            continue;
        }

        // 标题
        if trimmed.starts_with('#') {
            let hashes = trimmed.chars().take_while(|c| *c == '#').count();
            if hashes <= 6 && trimmed.chars().nth(hashes).map(|c| c == ' ').unwrap_or(true) {
                let scale = match hashes {
                    1 => 1.5,
                    2 => 1.32,
                    3 => 1.18,
                    4 => 1.08,
                    _ => 1.0,
                };
                push_line_marker(&mut job, &content[..indent + hashes], pal, size);
                let rest_start = (indent + hashes).min(content.len());
                let mut st = Sty::base(pal);
                st.bold = true;
                st.scale = scale;
                push_inline(
                    &mut job,
                    &content[rest_start..],
                    pal,
                    size,
                    st,
                    Some(size * scale * 1.42),
                );
                append_nl(&mut job, pal, size, Color32::TRANSPARENT);
                continue;
            }
        }

        // 引用
        if trimmed.starts_with('>') {
            push_line_marker(&mut job, &content[..indent], pal, size);
            let mut st = Sty::base(pal);
            st.color = pal.text_dim;
            st.italic = true;
            let mut fmt = fmt_of(pal, size, st);
            fmt.color = pal.accent;
            job.append(">", 0.0, fmt);
            push_inline(
                &mut job,
                &content[(indent + 1).min(content.len())..],
                pal,
                size,
                st,
                None,
            );
            append_nl(&mut job, pal, size, Color32::TRANSPARENT);
            continue;
        }

        // 分隔线
        if is_hr(trimmed) {
            let mut st = Sty::base(pal);
            st.color = pal.border;
            let fmt = fmt_of(pal, size, st);
            job.append(content, 0.0, fmt);
            append_nl(&mut job, pal, size, Color32::TRANSPARENT);
            continue;
        }

        // 列表
        if let Some((marker_len, is_task)) = list_marker(trimmed) {
            push_line_marker(&mut job, &content[..indent], pal, size);
            let mut st = Sty::base(pal);
            st.bold = true;
            st.color = alpha(pal.accent, 210);
            let mut fmt = fmt_of(pal, size, st);
            fmt.color = if is_task { pal.check } else { alpha(pal.accent, 215) };
            job.append(&content[indent..indent + marker_len], 0.0, fmt);
            let rest = indent + marker_len;
            let mut st2 = Sty::base(pal);
            if is_task {
                st2.color = pal.text;
            }
            push_inline(
                &mut job,
                &content[rest.min(content.len())..],
                pal,
                size,
                st2,
                None,
            );
            append_nl(&mut job, pal, size, Color32::TRANSPARENT);
            continue;
        }

        // 普通段落
        push_line_marker(&mut job, &content[..indent], pal, size);
        push_inline(&mut job, &content[indent..], pal, size, Sty::base(pal), None);
        append_nl(&mut job, pal, size, Color32::TRANSPARENT);
    }

    job
}

fn push_line_marker(job: &mut LayoutJob, prefix: &str, pal: &Palette, size: f32) {
    if prefix.is_empty() {
        return;
    }
    let fmt = fmt_of(pal, size, Sty::base(pal));
    job.append(prefix, 0.0, fmt);
}

fn append_nl(job: &mut LayoutJob, pal: &Palette, size: f32, bg: Color32) {
    let mut fmt = fmt_of(pal, size, Sty::base(pal));
    fmt.background = bg;
    job.append("\n", 0.0, fmt);
}

fn fmt_of(pal: &Palette, size: f32, st: Sty) -> TextFormat {
    let font = if st.mono {
        FontId::new(size * 0.95, pal.mono())
    } else if st.bold {
        FontId::new(size * st.scale, pal.bold())
    } else {
        FontId::new(size * st.scale, eframe::egui::FontFamily::Proportional)
    };
    let mut fmt = TextFormat {
        font_id: font,
        color: st.color,
        italics: st.italic,
        ..Default::default()
    };
    if st.strike {
        fmt.strikethrough = Stroke::new(1.0, st.color);
    }
    fmt
}

fn push_inline(
    job: &mut LayoutJob,
    text: &str,
    pal: &Palette,
    size: f32,
    base: Sty,
    line_height: Option<f32>,
) {
    if text.is_empty() {
        return;
    }
    let bytes = text.as_bytes();
    let mut i = 0usize;
    let mut plain_start = 0usize;

    let flush = |job: &mut LayoutJob, from: usize, to: usize, st: Sty, lh: Option<f32>| {
        if to > from {
            let mut fmt = fmt_of(pal, size, st);
            if let Some(lh) = lh {
                fmt.line_height = Some(lh);
            }
            job.append(&text[from..to], 0.0, fmt);
        }
    };

    while i < bytes.len() {
        let rest = &text[i..];
        // 行内代码
        if rest.starts_with('`') {
            if let Some(end) = rest[1..].find('`') {
                flush(job, plain_start, i, base, line_height);
                let mut st = base;
                st.mono = true;
                st.color = if pal.dark { pal.link } else { pal.accent };
                let mut fmt = fmt_of(pal, size, st);
                fmt.background = pal.inline_code_bg;
                if let Some(lh) = line_height {
                    fmt.line_height = Some(lh);
                }
                job.append(&text[i..i + end + 2], 0.0, fmt);
                i += end + 2;
                plain_start = i;
                continue;
            }
        }
        // 图片 / 链接
        if rest.starts_with('[') || rest.starts_with("![") {
            let is_img = rest.starts_with("![");
            let open = if is_img { 2 } else { 1 };
            if let Some(close) = rest[open..].find(']') {
                let label_end = open + close;
                if rest[label_end + 1..].starts_with('(') {
                    if let Some(paren) = rest[label_end + 1..].find(')') {
                        flush(job, plain_start, i, base, line_height);
                        let mut st = base;
                        st.color = pal.link;
                        let mut fmt = fmt_of(pal, size, st);
                        fmt.underline = Stroke::new(1.0, alpha(pal.link, 120));
                        if let Some(lh) = line_height {
                            fmt.line_height = Some(lh);
                        }
                        job.append(&text[i..i + label_end + 1], 0.0, fmt);
                        let mut st2 = base;
                        st2.color = alpha(pal.text_dim, 190);
                        st2.mono = true;
                        let mut fmt2 = fmt_of(pal, size * 0.92, st2);
                        if let Some(lh) = line_height {
                            fmt2.line_height = Some(lh);
                        }
                        job.append(&text[i + label_end + 1..i + label_end + 1 + paren + 1], 0.0, fmt2);
                        i += label_end + 1 + paren + 1;
                        plain_start = i;
                        continue;
                    }
                }
            }
        }
        // 加粗
        if rest.starts_with("**") || rest.starts_with("__") {
            let mark = &rest[..2];
            if let Some(end) = rest[2..].find(mark) {
                flush(job, plain_start, i, base, line_height);
                let mut st = base;
                st.bold = true;
                st.color = if base.color == pal.text {
                    pal.text
                } else {
                    base.color
                };
                let mut fmt = fmt_of(pal, size, st);
                if let Some(lh) = line_height {
                    fmt.line_height = Some(lh);
                }
                job.append(&text[i..i + end + 4], 0.0, fmt);
                i += end + 4;
                plain_start = i;
                continue;
            }
        }
        // 删除线
        if rest.starts_with("~~") {
            if let Some(end) = rest[2..].find("~~") {
                flush(job, plain_start, i, base, line_height);
                let mut st = base;
                st.strike = true;
                st.color = pal.text_dim;
                let mut fmt = fmt_of(pal, size, st);
                if let Some(lh) = line_height {
                    fmt.line_height = Some(lh);
                }
                job.append(&text[i..i + end + 4], 0.0, fmt);
                i += end + 4;
                plain_start = i;
                continue;
            }
        }
        // 斜体
        if rest.starts_with('*') || rest.starts_with('_') {
            let mark = &rest[..1];
            if let Some(end) = rest[1..].find(mark) {
                if end > 0 {
                    flush(job, plain_start, i, base, line_height);
                    let mut st = base;
                    st.italic = true;
                    let mut fmt = fmt_of(pal, size, st);
                    if let Some(lh) = line_height {
                        fmt.line_height = Some(lh);
                    }
                    job.append(&text[i..i + end + 2], 0.0, fmt);
                    i += end + 2;
                    plain_start = i;
                    continue;
                }
            }
        }
        // 跳过一个字符（按 UTF-8 边界）
        let ch = rest.chars().next().unwrap();
        i += ch.len_utf8();
    }
    flush(job, plain_start, text.len(), base, line_height);
}

fn is_hr(s: &str) -> bool {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    t.len() >= 3
        && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*') || t.chars().all(|c| c == '_'))
}

/// 返回 (标记长度, 是否任务项)
fn list_marker(s: &str) -> Option<(usize, bool)> {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    // 任务项 - [ ] / - [x]
    for prefix in ["- [ ] ", "- [x] ", "- [X] ", "* [ ] ", "* [x] "] {
        if s.starts_with(prefix) {
            return Some((prefix.len(), true));
        }
    }
    // 无序
    if let Some(c) = s.chars().next() {
        if matches!(c, '-' | '*' | '+') && s.chars().nth(1) == Some(' ') {
            return Some((2, false));
        }
    }
    // 有序
    let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits <= 3 {
        let rest: String = s.chars().skip(digits).collect();
        if rest.starts_with(". ") || rest.starts_with(") ") {
            return Some((digits + 2, false));
        }
    }
    None
}
