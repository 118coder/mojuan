//! 导出：Word (.docx) / PDF / 长图 PNG。
//!
//! 排版思路：一个 GDI 画笔（DocPainter）负责把 `Vec<Block>` 画到任意 HDC——
//! 内存位图（PNG 长图）或打印机 DC（Microsoft Print to PDF，DOCINFO.lpszOutput
//! 直接落文件，不弹对话框）。PDF 是真文字（可选中、可搜索），中文走系统字体。
//! docx 不走 GDI：直接生成 OOXML，用自带的 STORED 方式 ZIP 打包（CRC32 手写），
//! 全程零新依赖，不破坏单文件体积红线。
//!
//! 纸面固定用亮色主题（白纸黑字）。超大文档走 ChunkIter 按 VIndex 切片流式解析，
//! 与虚拟预览同一套索引，不会撑爆内存；PNG 长图仅支持阈值以内文档（量高需要两遍）。

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use mojuan_core::doc::{Block, ListItem, Run};
use mojuan_core::highlight::TokenStyle;
use mojuan_core::vindex::VIndex;

use crate::theme;

// ---------------------------------------------------------------------------
// 颜色（纸面 = 亮色主题固定值）
// ---------------------------------------------------------------------------

const INK: u32 = 0x1B1B1B;
const INK_DIM: u32 = 0x5E5E5E;
const INK_FAINT: u32 = 0x949494;
const ACCENT: u32 = 0x0067C0;
const CODE_BG: u32 = 0xF6F6F6;
const INLINE_CODE_BG: u32 = 0xEFEFEF;
const QUOTE_BAR: u32 = 0xC9C9C9;
const TABLE_HEAD: u32 = 0xF7F7F7;
const WHITE: u32 = 0xFFFFFF;
const TABLE_LINE: u32 = 0xE3E3E3;
const HR_LINE: u32 = 0xE5E5E5;
const PLACEHOLDER_BG: u32 = 0xEEEEEE;

/// RGB(u32) → GDI COLORREF(0x00BBGGRR)
fn cref(rgb: u32) -> u32 {
    ((rgb & 0xFF) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 0xFF)
}

fn code_color(style: TokenStyle) -> u32 {
    let pal = theme::Palette::new(mojuan_core::Theme::Light);
    let c = pal.tokens.color(style);
    ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | c.b() as u32
}

// ---------------------------------------------------------------------------
// GDI 绑定
// ---------------------------------------------------------------------------

type HDC = isize;
type HGDIOBJ = isize;
type HFONT = isize;
type BOOL = i32;

const PATCOPY: u32 = 0x00F0_0021;
const SRCCOPY: u32 = 0x00CC_0020;
const TA_TOP_LEFT: u32 = 0;
const BK_TRANSPARENT: i32 = 1;
const FW_NORMAL: i32 = 400;
const FW_BOLD: i32 = 700;
const CLEARTYPE_QUALITY: u32 = 5;
const DEFAULT_CHARSET: u32 = 1;
const LOGPIXELSX: i32 = 88;
const HORZRES: i32 = 8;
const VERTRES: i32 = 10;

#[allow(non_snake_case)]
#[link(name = "gdi32")]
extern "system" {
    fn CreateDCW(driver: *const u16, device: *const u16, port: *const u16, data: *const u8) -> HDC;
    fn CreateCompatibleDC(hdc: HDC) -> HDC;
    fn DeleteDC(hdc: HDC) -> BOOL;
    fn GetDeviceCaps(hdc: HDC, index: i32) -> i32;
    fn CreateFontW(
        height: i32, width: i32, esc: i32, orient: i32, weight: i32, italic: u32,
        underline: u32, strike: u32, charset: u32, out_prec: u32, clip_prec: u32,
        quality: u32, pitch: u32, face: *const u16,
    ) -> HFONT;
    fn SelectObject(hdc: HDC, obj: HGDIOBJ) -> HGDIOBJ;
    fn DeleteObject(obj: HGDIOBJ) -> BOOL;
    fn SetTextColor(hdc: HDC, color: u32) -> u32;
    fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    fn SetTextAlign(hdc: HDC, align: u32) -> u32;
    fn GetTextExtentPoint32W(hdc: HDC, text: *const u16, len: i32, size: *mut SIZE) -> BOOL;
    fn TextOutW(hdc: HDC, x: i32, y: i32, text: *const u16, len: i32) -> BOOL;
    fn CreateSolidBrush(color: u32) -> HGDIOBJ;
    fn PatBlt(hdc: HDC, x: i32, y: i32, w: i32, h: i32, rop: u32) -> BOOL;
    fn CreatePen(style: i32, width: i32, color: u32) -> HGDIOBJ;
    fn MoveToEx(hdc: HDC, x: i32, y: i32, old: *mut u8) -> BOOL;
    fn LineTo(hdc: HDC, x: i32, y: i32) -> BOOL;
    fn StartDocW(hdc: HDC, doc: *const DOCINFOW) -> i32;
    fn StartPage(hdc: HDC) -> i32;
    fn EndPage(hdc: HDC) -> i32;
    fn EndDoc(hdc: HDC) -> i32;
    fn CreateDIBSection(
        hdc: HDC, bmi: *const BITMAPINFO, usage: u32, bits: *mut *mut u8,
        section: isize, offset: u32,
    ) -> HGDIOBJ;
    fn StretchDIBits(
        hdc: HDC, dx: i32, dy: i32, dw: i32, dh: i32, sx: i32, sy: i32, sw: i32, sh: i32,
        bits: *const u8, bmi: *const BITMAPINFO, usage: u32, rop: u32,
    ) -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[repr(C)]
struct SIZE {
    cx: i32,
    cy: i32,
}

#[repr(C)]
struct DOCINFOW {
    cb_size: i32,
    doc_name: *const u16,
    output: *const u16,
    data_type: *const u16,
    fw_type: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BITMAPINFOHEADER {
    size: u32,
    width: i32,
    height: i32, // 负数 = 自顶向下
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_ppm: i32,
    y_ppm: i32,
    clr_used: u32,
    clr_important: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BITMAPINFO {
    header: BITMAPINFOHEADER,
    colors: [u32; 1],
}


/// 直连 Microsoft Print to PDF（Win10/11 全语言系统不变名）。
fn open_pdf_dc() -> Option<HDC> {
    let w = wide("Microsoft Print to PDF");
    let hdc = unsafe { CreateDCW(std::ptr::null(), w.as_ptr(), std::ptr::null(), std::ptr::null()) };
    if hdc != 0 {
        Some(hdc)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// 字体（Face 只是句柄+字号的轻包装；真正的句柄在 Fonts 里统一释放）
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Face {
    handle: HFONT,
    px: f32,
}

struct Fonts {
    body: Face,
    body_bold: Face,
    mono: Face,
    mono_cjk: Face, // 代码里的中文（Cascadia 无 CJK，GDI 不会自动回退）
    heads: [Face; 4],
    h5: Face,
}

fn make_font(face: &str, px: f32, weight: i32, handles: &mut Vec<HFONT>) -> Face {
    let w = wide(face);
    let h = unsafe {
        CreateFontW(
            -(px.round() as i32), 0, 0, 0, weight, 0, 0, 0, DEFAULT_CHARSET, 0, 0,
            CLEARTYPE_QUALITY, 0, w.as_ptr(),
        )
    };
    handles.push(h);
    Face { handle: h, px }
}

impl Fonts {
    fn new(scale: f32, base_px: f32) -> Self {
        let mut handles = Vec::new();
        let s = scale;
        Self {
            body: make_font("Microsoft YaHei", base_px * s, FW_NORMAL, &mut handles),
            body_bold: make_font("Microsoft YaHei", base_px * s, FW_BOLD, &mut handles),
            mono: make_font("Cascadia Mono", base_px * 0.85 * s, FW_NORMAL, &mut handles),
            mono_cjk: make_font("Microsoft YaHei", base_px * 0.85 * s, FW_NORMAL, &mut handles),
            heads: [
                make_font("Microsoft YaHei", base_px * 1.85 * s, FW_BOLD, &mut handles),
                make_font("Microsoft YaHei", base_px * 1.5 * s, FW_BOLD, &mut handles),
                make_font("Microsoft YaHei", base_px * 1.25 * s, FW_BOLD, &mut handles),
                make_font("Microsoft YaHei", base_px * 1.05 * s, FW_BOLD, &mut handles),
            ],
            h5: make_font("Microsoft YaHei", base_px * 0.95 * s, FW_BOLD, &mut handles),
        }
        // 句柄进程生命周期内不释放（导出一次性，数量固定小）
    }

    fn for_run(&self, run: &Run) -> Face {
        if run.code {
            self.mono
        } else if run.bold {
            self.body_bold
        } else {
            self.body
        }
    }
}

// ---------------------------------------------------------------------------
// DocPainter
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ImageBits {
    bgra: Vec<u8>,
    bmi: BITMAPINFO,
    w: i32,
    h: i32,
}

fn decode_img(bytes: &[u8]) -> Option<ImageBits> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let rgba = img.into_raw();
    let mut bgra = Vec::with_capacity(rgba.len());
    for px in rgba.chunks_exact(4) {
        bgra.extend_from_slice(&[px[2], px[1], px[0], 255]);
    }
    Some(ImageBits {
        bmi: BITMAPINFO {
            header: BITMAPINFOHEADER {
                size: 40,
                width: w as i32,
                height: -(h as i32),
                planes: 1,
                bit_count: 32,
                compression: 0,
                size_image: 0,
                x_ppm: 0,
                y_ppm: 0,
                clr_used: 0,
                clr_important: 0,
            },
            colors: [0; 1],
        },
        bgra,
        w: w as i32,
        h: h as i32,
    })
}

type ImgCache = Rc<RefCell<HashMap<String, Option<ImageBits>>>>;

pub struct DocPainter {
    hdc: HDC,
    dry: bool,
    x0: f32,
    y: f32,
    w: f32,
    fonts: Fonts,
    scale: f32,
    base_px: f32,
    img_cache: ImgCache,
    url_cache: ImgCache,
}

impl DocPainter {
    fn new(hdc: HDC, w: f32, dry: bool, scale: f32, base_px: f32, caches: (ImgCache, ImgCache)) -> Self {
        let fonts = Fonts::new(scale, base_px);
        unsafe {
            SetBkMode(hdc, BK_TRANSPARENT);
            SetTextAlign(hdc, TA_TOP_LEFT);
        }
        Self {
            hdc,
            dry,
            x0: 0.0,
            y: 0.0,
            w,
            fonts,
            scale,
            base_px,
            img_cache: caches.0,
            url_cache: caches.1,
        }
    }

    fn lh(&self, px: f32, factor: f32) -> f32 {
        px * factor
    }

    fn text_w(&self, f: Face, s: &str) -> f32 {
        let units: Vec<u16> = s.encode_utf16().collect();
        let mut sz = SIZE { cx: 0, cy: 0 };
        unsafe {
            SelectObject(self.hdc, f.handle);
            GetTextExtentPoint32W(self.hdc, units.as_ptr(), units.len() as i32, &mut sz);
        }
        sz.cx as f32
    }

    fn draw_text(&self, f: Face, x: f32, y: f32, s: &str, color: u32) {
        if self.dry || s.is_empty() {
            return;
        }
        let units: Vec<u16> = s.encode_utf16().collect();
        unsafe {
            SelectObject(self.hdc, f.handle);
            SetTextColor(self.hdc, cref(color));
            TextOutW(self.hdc, x.round() as i32, y.round() as i32, units.as_ptr(), units.len() as i32);
        }
    }

    fn fill(&self, x: f32, y: f32, w: f32, h: f32, color: u32) {
        if self.dry || w <= 0.0 || h <= 0.0 {
            return;
        }
        let brush = unsafe { CreateSolidBrush(cref(color)) };
        unsafe {
            let old = SelectObject(self.hdc, brush);
            PatBlt(self.hdc, x.round() as i32, y.round() as i32, w.round() as i32, h.round() as i32, PATCOPY);
            SelectObject(self.hdc, old);
            DeleteObject(brush);
        }
    }

    fn hline(&self, x1: f32, x2: f32, y: f32, color: u32, thickness: f32) {
        if self.dry {
            return;
        }
        let pen = unsafe { CreatePen(0, (thickness * self.scale).max(1.0).round() as i32, cref(color)) };
        unsafe {
            let old = SelectObject(self.hdc, pen);
            MoveToEx(self.hdc, x1.round() as i32, y.round() as i32, std::ptr::null_mut());
            LineTo(self.hdc, x2.round() as i32, y.round() as i32);
            SelectObject(self.hdc, old);
            DeleteObject(pen);
        }
    }

    fn content_w(&self) -> f32 {
        self.w - self.x0 * 2.0
    }
}

// -- 富文本折行 --------------------------------------------------------------

fn is_wide(ch: char) -> bool {
    let c = ch as u32;
    (0x2E80..=0x9FFF).contains(&c)
        || (0xF900..=0xFAFF).contains(&c)
        || (0xFF00..=0xFF60).contains(&c)
        || (0x3000..=0x303F).contains(&c)
}

impl DocPainter {
    /// 把带样式的 runs 按宽度折成行。行内元素是 (run 下标, 文本片段)。
    /// font_of 决定每个 run 用哪张字面（正文按 run 属性选，标题整行统一）。
    fn wrap_runs(
        &mut self,
        runs: &[(String, Run)],
        max_w: f32,
        font_of: impl Fn(&Run, &Fonts) -> Face,
    ) -> Vec<Vec<(usize, String)>> {
        let mut lines: Vec<Vec<(usize, String)>> = Vec::new();
        let mut cur: Vec<(usize, String)> = Vec::new();
        let mut cur_w = 0.0f32;
        macro_rules! flush_line {
            () => {{
                while cur.last().map(|(_, s)| s == " ").unwrap_or(false) {
                    cur.pop();
                }
                if !cur.is_empty() {
                    lines.push(std::mem::take(&mut cur));
                }
            }};
        }
        for (i, (text, run)) in runs.iter().enumerate() {
            let f = font_of(run, &self.fonts);
            let mut word = String::new();
            macro_rules! place {
                ($piece:expr, $w:expr) => {{
                    let mut piece: String = $piece;
                    let mut w: f32 = $w;
                    // 超宽硬拆（长 URL / 长单词）
                    while w > max_w && piece.chars().count() > 1 {
                        let mut head = String::new();
                        let mut head_w = 0.0f32;
                        for ch in piece.chars() {
                            let cw = self.text_w(f, &ch.to_string());
                            if cur_w + head_w + cw > max_w && !head.is_empty() {
                                break;
                            }
                            head.push(ch);
                            head_w += cw;
                            if head_w + cur_w >= max_w {
                                break;
                            }
                        }
                        if head.is_empty() {
                            break;
                        }
                        cur.push((i, head.clone()));
                        flush_line!();
                        cur_w = 0.0;
                        piece = piece[head.len()..].to_string();
                        w = self.text_w(f, &piece);
                    }
                    if cur_w + w > max_w && !cur.is_empty() {
                        flush_line!();
                        cur_w = 0.0;
                    }
                    cur.push((i, piece));
                    cur_w += w;
                }};
            }
            macro_rules! flush_word {
                () => {{
                    if !word.is_empty() {
                        let piece = std::mem::take(&mut word);
                        let w = self.text_w(f, &piece);
                        place!(piece, w);
                    }
                }};
            }
            for ch in text.chars() {
                if ch.is_whitespace() {
                    flush_word!();
                    let sw = self.text_w(f, " ");
                    if cur_w + sw > max_w && !cur.is_empty() {
                        flush_line!();
                        cur_w = 0.0;
                    } else {
                        cur.push((i, " ".to_string()));
                        cur_w += sw;
                    }
                } else if is_wide(ch) {
                    flush_word!();
                    let cw = self.text_w(f, &ch.to_string());
                    if cur_w + cw > max_w && !cur.is_empty() {
                        flush_line!();
                        cur_w = 0.0;
                    }
                    cur.push((i, ch.to_string()));
                    cur_w += cw;
                } else {
                    word.push(ch);
                }
            }
            flush_word!();
        }
        flush_line!();
        lines
    }
}

impl DocPainter {
    /// 渲染一组块。PDF 分页由外部在调用前检查高度后自行换页。
    pub fn paint_blocks(&mut self, blocks: &[Block], base: Option<&Path>) {
        for b in blocks {
            self.paint_block(b, base, 0);
            self.y += 12.0 * self.scale;
        }
    }

    fn paint_block(&mut self, b: &Block, base: Option<&Path>, level: usize) {
        match b {
            Block::Heading { level: hl, runs, .. } => self.paint_heading(*hl, runs),
            Block::Para(runs) => {
                let owned: Vec<(String, Run)> = runs.iter().map(|r| (r.text.clone(), r.clone())).collect();
                let lines = self.wrap_runs(&owned, self.content_w(), |r, fonts| fonts.for_run(r));
                self.draw_lines(&lines, &owned, 1.55);
            }
            Block::Code { lines: code_lines, .. } => self.paint_code(code_lines),
            Block::Quote(inner) => {
                self.y += 4.0 * self.scale;
                let start_y = self.y;
                let old_x0 = self.x0;
                self.x0 += 16.0 * self.scale;
                for q in inner {
                    self.paint_block(q, base, level);
                }
                self.x0 = old_x0;
                let bar_h = self.y - start_y;
                self.fill(old_x0 + 2.0, start_y + 2.0, 3.0 * self.scale, (bar_h - 6.0).max(4.0), QUOTE_BAR);
                self.y += 8.0 * self.scale;
            }
            Block::List { ordered, start, items } => {
                for (i, item) in items.iter().enumerate() {
                    let num = if *ordered { *start + i as u64 } else { 0 };
                    self.paint_list_item(item, *ordered, num, level, base);
                }
            }
            Block::Table { head, rows, .. } => self.paint_table(head, rows),
            Block::Rule => {
                self.y += 8.0 * self.scale;
                self.hline(self.x0, self.x0 + self.content_w(), self.y, HR_LINE, 1.0);
                self.y += 10.0 * self.scale;
            }
            Block::Image { src, path, alt, .. } => self.paint_image(src, path.as_deref(), alt, base),
        }
    }

    fn paint_heading(&mut self, hl: u8, runs: &[Run]) {
        let fi = match hl {
            1 => 0,
            2 => 1,
            3 => 2,
            4 => 3,
            _ => 4,
        };
        self.y += match hl {
            1 => 16.0 * self.scale,
            2 => 12.0 * self.scale,
            _ => 8.0 * self.scale,
        };
        let face = if fi == 4 { self.fonts.h5 } else { self.fonts.heads[fi] };
        let factor = 1.3;
        let owned: Vec<(String, Run)> = runs
            .iter()
            .map(|r| (r.text.clone(), Run { bold: true, ..r.clone() }))
            .collect();
        let lines = self.wrap_runs(&owned, self.content_w(), |_, _| face);
        for line in &lines {
            let mut cx = self.x0;
            for (run_i, seg) in line {
                let run = &owned[*run_i].1;
                let f = face;
                let w = self.text_w(f, seg);
                let color = if run.link.is_some() { ACCENT } else { INK };
                self.draw_text(f, cx, self.y, seg, color);
                cx += w;
            }
            self.y += self.lh(face.px, factor);
        }
        if hl == 1 {
            self.y += 6.0 * self.scale;
            self.hline(self.x0, self.x0 + self.content_w(), self.y, HR_LINE, 1.0);
        }
        self.y += 8.0 * self.scale;
    }

    fn draw_lines(&mut self, lines: &[Vec<(usize, String)>], runs: &[(String, Run)], factor: f32) {
        for line in lines {
            let mut cx = self.x0;
            let mut max_fpx = self.base_px * self.scale;
            for (run_i, seg) in line {
                let run = &runs[*run_i].1;
                let f = self.fonts.for_run(run);
                max_fpx = max_fpx.max(f.px);
                let w = self.text_w(f, seg);
                let color = if run.link.is_some() { ACCENT } else { INK };
                if run.code {
                    self.fill(cx - 3.0, self.y + f.px * 0.12, w + 6.0, f.px * 1.18, INLINE_CODE_BG);
                }
                self.draw_text(f, cx, self.y, seg, color);
                if run.strike {
                    self.hline(cx, cx + w, self.y + f.px * 0.62, INK_FAINT, 1.0);
                }
                cx += w;
            }
            self.y += self.lh(max_fpx, factor);
        }
    }

    fn paint_code(&mut self, code_lines: &[Vec<mojuan_core::highlight::Token>]) {
        let pad = 10.0 * self.scale;
        let lh = self.lh(self.fonts.mono.px, 1.42);
        let inner_w = self.content_w() - pad * 2.0;
        // 量高（与绘制同一套逐字符 walk，保证一致）
        let mut total = pad * 2.0;
        for cl in code_lines {
            total += self.code_count(cl, inner_w) as f32 * lh;
        }
        self.fill(self.x0, self.y, self.content_w(), total, CODE_BG);
        self.y += pad;
        for cl in code_lines {
            self.code_draw(cl, self.x0 + pad, inner_w, lh);
            self.y += lh;
        }
        self.y += pad;
        self.y += 10.0 * self.scale;
    }

    /// 等宽逐字符走查：返回折行数（只计数，不落墨）
    fn code_count(&self, tokens: &[mojuan_core::highlight::Token], max_w: f32) -> usize {
        let w_ascii = self.text_w(self.fonts.mono, "M").max(1.0);
        let w_cjk = self.text_w(self.fonts.mono_cjk, "中").max(1.0);
        let mut lines = 1usize;
        let mut cx = 0.0f32;
        for t in tokens {
            for ch in t.text.replace('\t', "    ").chars() {
                let w = if is_wide(ch) { w_cjk } else { w_ascii };
                if cx + w > max_w {
                    lines += 1;
                    cx = 0.0;
                    if ch == ' ' {
                        continue;
                    }
                }
                cx += w;
            }
        }
        lines
    }

    /// 等宽逐字符绘制（与 code_count 同一折行规则）
    fn code_draw(&mut self, tokens: &[mojuan_core::highlight::Token], x: f32, max_w: f32, lh: f32) {
        let w_ascii = self.text_w(self.fonts.mono, "M").max(1.0);
        let w_cjk = self.text_w(self.fonts.mono_cjk, "中").max(1.0);
        let start_y = self.y;
        let mut cx = 0.0f32;
        for t in tokens {
            let color = code_color(t.style);
            for ch in t.text.replace('\t', "    ").chars() {
                let (w, f) = if is_wide(ch) { (w_cjk, self.fonts.mono_cjk) } else { (w_ascii, self.fonts.mono) };
                if cx + w > max_w {
                    self.y += lh;
                    cx = 0.0;
                    if ch == ' ' {
                        continue;
                    }
                }
                self.draw_text(f, x + cx, self.y, &ch.to_string(), color);
                cx += w;
            }
        }
        let _ = start_y;
    }

    fn paint_list_item(&mut self, item: &ListItem, ordered: bool, num: u64, level: usize, base: Option<&Path>) {
        let marker = if ordered {
            format!("{}.", num)
        } else {
            ["•", "◦", "▪"][level % 3].to_string()
        };
        let base_x = self.x0;
        let indent = (20.0 + level as f32 * 18.0) * self.scale;
        let marker_f = self.fonts.body_bold;
        let mw = self.text_w(marker_f, &marker) + 6.0 * self.scale;
        self.x0 = base_x + indent;
        self.draw_text(marker_f, base_x + indent - mw, self.y, &marker, INK_DIM);
        for b in &item.blocks {
            match b {
                Block::Para(runs) => {
                    let mut owned: Vec<(String, Run)> = Vec::new();
                    match item.task {
                        Some(true) => owned.push(("☑  ".to_string(), Run::default())),
                        Some(false) => owned.push(("☐  ".to_string(), Run::default())),
                        None => {}
                    }
                    owned.extend(runs.iter().map(|r| (r.text.clone(), r.clone())));
                    let lines = self.wrap_runs(&owned, self.content_w(), |r, fonts| fonts.for_run(r));
                    self.draw_lines(&lines, &owned, 1.5);
                }
                other => self.paint_block(other, base, level + 1),
            }
        }
        self.x0 = base_x;
        self.y += 3.0 * self.scale;
    }

    fn paint_table(&mut self, head: &[Vec<Run>], rows: &[Vec<Vec<Run>>]) {
        let cols = head.len().max(1);
        let mut weights = vec![1usize; cols];
        let weight_of = |cell: &Vec<Run>, w: &mut Vec<usize>| {
            for (i, r) in cell.iter().enumerate() {
                if i < cols {
                    w[i] = w[i].max(r.text.chars().count().max(1));
                }
            }
        };
        for cell in head {
            weight_of(cell, &mut weights);
        }
        for row in rows {
            for cell in row {
                weight_of(cell, &mut weights);
            }
        }
        let sum: usize = weights.iter().sum();
        let avail = self.content_w();
        let col_ws: Vec<f32> = weights
            .iter()
            .map(|&wt| (avail * wt as f32 / sum as f32).max(60.0 * self.scale))
            .collect();
        let pad = 6.0 * self.scale;
        let lh = self.lh(self.fonts.body.px, 1.4);

        let paint_row = |p: &mut DocPainter, cells: &[(Vec<Run>, bool)], y_start: &mut f32| {
            let mut row_h = 0.0f32;
            let mut prepared: Vec<(Vec<(String, Run)>, Vec<Vec<(usize, String)>>)> = Vec::new();
            for (i, (runs, is_head)) in cells.iter().enumerate() {
                let mut owned: Vec<(String, Run)> = runs
                    .iter()
                    .map(|r| (r.text.clone(), Run { bold: *is_head || r.bold, ..r.clone() }))
                    .collect();
                if owned.is_empty() {
                    owned.push((String::new(), Run::default()));
                }
                let cw = col_ws.get(i).copied().unwrap_or(avail) - pad * 2.0;
                let lines = p.wrap_runs(&owned, cw, |r, fonts| fonts.for_run(r));
                row_h = row_h.max(lines.len() as f32 * lh + pad * 2.0);
                prepared.push((owned, lines));
            }
            let head_bg = cells.first().map(|c| c.1).unwrap_or(false);
            p.fill(p.x0, *y_start, avail, row_h, if head_bg { TABLE_HEAD } else { WHITE });
            let y = *y_start + pad;
            for (i, (owned, lines)) in prepared.iter().enumerate() {
                let cx = p.x0 + col_ws[..i].iter().sum::<f32>() + pad;
                let mut cy = y;
                for line in lines {
                    let mut lx = cx;
                    for (run_i, seg) in line {
                        let run = &owned[*run_i].1;
                        let f = p.fonts.for_run(run);
                        let w = p.text_w(f, seg);
                        p.draw_text(f, lx, cy, seg, INK);
                        if run.strike {
                            p.hline(lx, lx + w, cy + lh * 0.55, INK_FAINT, 1.0);
                        }
                        lx += w;
                    }
                    cy += lh;
                }
            }
            p.hline(p.x0, p.x0 + avail, *y_start, TABLE_LINE, 1.0);
            p.hline(p.x0, p.x0 + avail, *y_start + row_h, TABLE_LINE, 1.0);
            *y_start += row_h;
        };

        let mut y = self.y;
        let head_cells: Vec<(Vec<Run>, bool)> = head.iter().map(|r| (r.clone(), true)).collect();
        paint_row(self, &head_cells, &mut y);
        for row in rows {
            let cells: Vec<(Vec<Run>, bool)> = row.iter().map(|r| (r.clone(), false)).collect();
            paint_row(self, &cells, &mut y);
        }
        self.y = y + 12.0 * self.scale;
    }

    fn load_image(&mut self, src: &str, path: Option<&Path>, base: Option<&Path>) -> Option<ImageBits> {
        if let Some(p) = path {
            let key = p.to_string_lossy().to_string();
            let hit = self.img_cache.borrow().get(&key).cloned().flatten();
            return match hit {
                Some(v) => Some(v),
                None => {
                    let v = std::fs::read(p).ok().and_then(|b| decode_img(&b));
                    self.img_cache.borrow_mut().insert(key, v.clone());
                    v
                }
            };
        }
        if src.starts_with("http://") || src.starts_with("https://") {
            let hit = self.url_cache.borrow().get(src).cloned().flatten();
            return match hit {
                Some(v) => Some(v),
                None => {
                    let v = mojuan_core::http::https_get(src, 8 * 1024 * 1024, 8000)
                        .ok()
                        .filter(|r| r.status == 200)
                        .and_then(|r| decode_img(&r.body));
                    self.url_cache.borrow_mut().insert(src.to_string(), v.clone());
                    v
                }
            };
        }
        if let Some(b) = base {
            let p = b.join(src);
            let key = p.to_string_lossy().to_string();
            let hit = self.img_cache.borrow().get(&key).cloned().flatten();
            return match hit {
                Some(v) => Some(v),
                None => {
                    let v = std::fs::read(&p).ok().and_then(|b| decode_img(&b));
                    self.img_cache.borrow_mut().insert(key, v.clone());
                    v
                }
            };
        }
        None
    }

    fn paint_image(&mut self, src: &str, path: Option<&Path>, alt: &str, base: Option<&Path>) {
        let Some(bits) = self.load_image(src, path, base) else {
            let ph_h = 60.0 * self.scale;
            self.fill(self.x0, self.y, self.content_w(), ph_h, PLACEHOLDER_BG);
            let label = format!("（图片无法加载：{}）", if alt.is_empty() { src } else { alt });
            self.draw_text(
                self.fonts.body,
                self.x0 + 12.0 * self.scale,
                self.y + ph_h / 2.0 - self.fonts.body.px / 2.0,
                &label,
                INK_FAINT,
            );
            self.y += ph_h + 12.0 * self.scale;
            return;
        };
        let avail_w = self.content_w().min(720.0 * self.scale);
        let ratio = bits.h as f32 / bits.w.max(1) as f32;
        let mut draw_w = avail_w;
        let mut draw_h = avail_w * ratio;
        if draw_h > 880.0 * self.scale {
            draw_h = 880.0 * self.scale;
            draw_w = draw_h / ratio.max(0.001);
        }
        if !self.dry {
            unsafe {
                StretchDIBits(
                    self.hdc,
                    self.x0.round() as i32,
                    self.y.round() as i32,
                    draw_w.round() as i32,
                    draw_h.round() as i32,
                    0, 0, bits.w, bits.h,
                    bits.bgra.as_ptr(),
                    &bits.bmi,
                    0,
                    SRCCOPY,
                );
            }
        }
        self.y += draw_h + 8.0 * self.scale;
    }
}

// ---------------------------------------------------------------------------
// ChunkIter：普通文档一个 chunk 出全量；超大文档按 VIndex 逐块流式解析
// ---------------------------------------------------------------------------

pub struct ChunkIter<'a> {
    text: &'a str,
    base: Option<&'a Path>,
    idx: Option<VIndex>,
    pos: usize,
    first: bool,
}

impl<'a> ChunkIter<'a> {
    pub fn new(text: &'a str, base: Option<&'a Path>) -> Self {
        let idx = if text.len() > crate::ui::vpreview::VIRTUAL_THRESHOLD {
            Some(VIndex::build(text))
        } else {
            None
        };
        Self {
            text,
            base,
            idx,
            pos: 0,
            first: true,
        }
    }

    pub fn huge(&self) -> bool {
        self.idx.is_some()
    }
}

impl<'a> Iterator for ChunkIter<'a> {
    type Item = Vec<Block>;

    fn next(&mut self) -> Option<Self::Item> {
        match &self.idx {
            None => {
                if self.first {
                    self.first = false;
                    let doc = mojuan_core::doc::parse(self.text, self.base);
                    Some(doc.blocks)
                } else {
                    None
                }
            }
            Some(idx) => {
                if self.pos >= idx.blocks.len() {
                    return None;
                }
                let slice = idx.slice(self.text, self.pos);
                self.pos += 1;
                let doc = mojuan_core::doc::parse(slice, self.base);
                Some(doc.blocks)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// PNG 长图
// ---------------------------------------------------------------------------

const PNG_WIDTH: f32 = 960.0;
const PNG_MARGIN: f32 = 48.0;
const PNG_MAX_H: f32 = 28000.0;

pub fn export_png(text: &str, base: Option<&Path>, out: &Path) -> Result<String, String> {
    let mut it = ChunkIter::new(text, base);
    if it.huge() {
        return Err("文档太长，长图导出仅支持 18 万字以内的文档（PDF / Word 不限）".into());
    }
    let caches: (ImgCache, ImgCache) =
        (Rc::new(RefCell::new(HashMap::new())), Rc::new(RefCell::new(HashMap::new())));

    let mut blocks_all = Vec::new();
    for chunk in &mut it {
        blocks_all.extend(chunk);
    }

    // 第一遍：量高
    let scratch = unsafe { CreateDCW(std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null()) };
    let mut meas = DocPainter::new(scratch, PNG_WIDTH, true, 1.0, 16.0, caches.clone());
    meas.x0 = PNG_MARGIN;
    meas.y = PNG_MARGIN;
    meas.paint_blocks(&blocks_all, base);
    let truncated = meas.y + PNG_MARGIN > PNG_MAX_H;
    let total_h = (meas.y + PNG_MARGIN).min(PNG_MAX_H);
    let _ = &meas; // 字体句柄随 painter 存续

    // 第二遍：画到位图
    let bmi = BITMAPINFO {
        header: BITMAPINFOHEADER {
            size: 40,
            width: PNG_WIDTH as i32,
            height: -total_h as i32,
            planes: 1,
            bit_count: 32,
            compression: 0,
            size_image: 0,
            x_ppm: 0,
            y_ppm: 0,
            clr_used: 0,
            clr_important: 0,
        },
        colors: [0; 1],
    };
    let screen = unsafe { CreateDCW(std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null()) };
    let mem = unsafe { CreateCompatibleDC(screen) };
    let mut bits_ptr: *mut u8 = std::ptr::null_mut();
    let dib = unsafe { CreateDIBSection(mem, &bmi, 0, &mut bits_ptr, 0, 0) };
    if bits_ptr.is_null() {
        return Err("位图创建失败（文档太长或内存不足）".into());
    }
    unsafe {
        SelectObject(mem, dib);
        let brush = CreateSolidBrush(cref(WHITE));
        let old = SelectObject(mem, brush);
        PatBlt(mem, 0, 0, PNG_WIDTH as i32, total_h as i32, PATCOPY);
        SelectObject(mem, old);
        DeleteObject(brush);
    }
    let mut p = DocPainter::new(mem, PNG_WIDTH, false, 1.0, 16.0, caches.clone());
    p.x0 = PNG_MARGIN;
    p.y = PNG_MARGIN;
    p.paint_blocks(&blocks_all, base);
    let mut rgba = Vec::with_capacity(total_h as usize * PNG_WIDTH as usize * 4);
    for i in 0..(total_h as usize * PNG_WIDTH as usize) {
        let off = i * 4;
        unsafe {
            rgba.extend_from_slice(&[*bits_ptr.add(off + 2), *bits_ptr.add(off + 1), *bits_ptr.add(off), 255]);
        }
    }
    unsafe {
        DeleteObject(dib);
        DeleteDC(mem);
        DeleteDC(screen);
        DeleteDC(scratch);
    }
    let img = image::RgbaImage::from_raw(PNG_WIDTH as u32, total_h as u32, rgba)
        .ok_or("位图数据不完整")?;
    img.save(out).map_err(|e| format!("保存 PNG 失败：{e}"))?;
    Ok(if truncated {
        "已导出 PNG（文档过长，超出部分已截断）".to_string()
    } else {
        "已导出 PNG 长图".to_string()
    })
}

// ---------------------------------------------------------------------------
// PDF（Microsoft Print to PDF）
// ---------------------------------------------------------------------------

const PAGE_MARGIN_MM: f32 = 18.0;

pub fn export_pdf(text: &str, base: Option<&Path>, title: &str, out: &Path) -> Result<String, String> {
    let hdc = open_pdf_dc().ok_or("系统里没有找到 PDF 打印机（需要启用 Microsoft Print to PDF）")?;
    let caches: (ImgCache, ImgCache) =
        (Rc::new(RefCell::new(HashMap::new())), Rc::new(RefCell::new(HashMap::new())));
    let ret = (|| -> Result<(), String> {
        unsafe {
            let dpi_x = GetDeviceCaps(hdc, LOGPIXELSX).max(96) as f32;
            let page_w = GetDeviceCaps(hdc, HORZRES) as f32;
            let page_h = GetDeviceCaps(hdc, VERTRES) as f32;
            let scale = dpi_x / 96.0;
            let margin = PAGE_MARGIN_MM / 25.4 * dpi_x;
            let content_w = page_w - margin * 2.0;
            let bottom = page_h - margin;

            let mut doc_name = wide(&format!("{title} · 墨卷"));
            let mut out_path = wide(&out.to_string_lossy());
            let docinfo = DOCINFOW {
                cb_size: std::mem::size_of::<DOCINFOW>() as i32,
                doc_name: doc_name.as_mut_ptr(),
                output: out_path.as_mut_ptr(),
                data_type: std::ptr::null(),
                fw_type: 0,
            };
            if StartDocW(hdc, &docinfo) <= 0 {
                return Err("无法开始打印任务（目标目录不可写？）".into());
            }

            let mut p = DocPainter::new(hdc, content_w, false, scale, 16.0, caches.clone());
            let mut probe = DocPainter::new(hdc, content_w, true, scale, 16.0, caches.clone());
            let mut page_open = false;
            let start_page = |page_open: &mut bool| {
                if !*page_open {
                    StartPage(hdc);
                    *page_open = true;
                }
            };
            start_page(&mut page_open);
            p.y = margin;

            let mut it = ChunkIter::new(text, base);
            for chunk in &mut it {
                for b in &chunk {
                    // 预算高度：放不下先换页（整块比整页还高的就直接画，允许溢出裁切）
                    probe.y = p.y;
                    probe.paint_block(b, base, 0);
                    let need = probe.y - p.y + 12.0 * scale;
                    if p.y + need > bottom && need < bottom - margin * 2.0 {
                        EndPage(hdc);
                        page_open = false;
                        start_page(&mut page_open);
                        p.y = margin;
                    }
                    p.paint_block(b, base, 0);
                    p.y += 12.0 * scale;
                }
            }
            if page_open {
                EndPage(hdc);
            }
            EndDoc(hdc);
        }
        Ok(())
    })();
    unsafe {
        DeleteDC(hdc);
    }
    ret?;
    Ok(format!("已导出 PDF：{title}"))
}

// ---------------------------------------------------------------------------
// Word (.docx)：手写 OOXML + STORED ZIP（零依赖）
// ---------------------------------------------------------------------------

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

struct Zip {
    out: Vec<u8>,
    entries: Vec<(String, u32, u32, u32)>,
}

impl Zip {
    fn new() -> Self {
        Self {
            out: Vec::new(),
            entries: Vec::new(),
        }
    }
    fn add(&mut self, name: &str, data: &[u8]) {
        let crc = crc32(data);
        let offset = self.out.len() as u32;
        let n = name.as_bytes();
        let mut h = Vec::with_capacity(30 + n.len());
        h.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        h.extend_from_slice(&20u16.to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes()); // stored
        h.extend_from_slice(&0u16.to_le_bytes()); // time
        h.extend_from_slice(&0x21u16.to_le_bytes()); // date
        h.extend_from_slice(&crc.to_le_bytes());
        h.extend_from_slice(&(data.len() as u32).to_le_bytes());
        h.extend_from_slice(&(data.len() as u32).to_le_bytes());
        h.extend_from_slice(&(n.len() as u16).to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes());
        h.extend_from_slice(n);
        self.out.extend_from_slice(&h);
        self.out.extend_from_slice(data);
        self.entries.push((name.to_string(), crc, data.len() as u32, offset));
    }
    fn finish(mut self) -> Vec<u8> {
        let cd_start = self.out.len() as u32;
        let mut cd = Vec::new();
        for (name, crc, size, offset) in &self.entries {
            let n = name.as_bytes();
            cd.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            cd.extend_from_slice(&20u16.to_le_bytes());
            cd.extend_from_slice(&20u16.to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes());
            cd.extend_from_slice(&0x21u16.to_le_bytes());
            cd.extend_from_slice(&crc.to_le_bytes());
            cd.extend_from_slice(&size.to_le_bytes());
            cd.extend_from_slice(&size.to_le_bytes());
            cd.extend_from_slice(&(n.len() as u16).to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes());
            cd.extend_from_slice(&0u32.to_le_bytes());
            cd.extend_from_slice(&offset.to_le_bytes());
            cd.extend_from_slice(n);
        }
        self.out.extend_from_slice(&cd);
        let count = self.entries.len() as u16;
        self.out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes());
        self.out.extend_from_slice(&count.to_le_bytes());
        self.out.extend_from_slice(&count.to_le_bytes());
        self.out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
        self.out.extend_from_slice(&cd_start.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes());
        self.out
    }
}

fn xml_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '&' => o.push_str("&amp;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            _ => o.push(c),
        }
    }
    o
}

fn xml_clean(s: &str) -> String {
    s.chars()
        .filter(|&c| c == '\t' || c == '\n' || c == '\r' || c >= '\u{20}')
        .collect()
}

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

struct DocxBuilder {
    zip: Zip,
    rels: Vec<(String, String, bool)>,
    body: String,
    rid: usize,
    img_i: usize,
}

impl DocxBuilder {
    fn new() -> Self {
        Self {
            zip: Zip::new(),
            rels: Vec::new(),
            body: String::with_capacity(1 << 16),
            rid: 0,
            img_i: 0,
        }
    }

    fn next_rid(&mut self) -> String {
        self.rid += 1;
        format!("rId{}", self.rid)
    }

    fn text_run(&mut self, text: &str, bold: bool, italic: bool, strike: bool, code: bool) {
        if text.is_empty() {
            return;
        }
        let mut rpr = String::new();
        if code {
            rpr.push_str("<w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\" w:eastAsia=\"Microsoft YaHei\"/><w:shd w:val=\"clear\" w:fill=\"EFEFEF\"/><w:sz w:val=\"19\"/>");
        }
        if bold {
            rpr.push_str("<w:b/>");
        }
        if italic {
            rpr.push_str("<w:i/>");
        }
        if strike {
            rpr.push_str("<w:strike/>");
        }
        self.body.push_str(&format!(
            "<w:r><w:rPr>{rpr}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>",
            xml_escape(&xml_clean(text))
        ));
    }

    fn link_run(&mut self, url: &str, text: &str) {
        let rid = self.next_rid();
        self.rels.push((rid.clone(), url.to_string(), true));
        self.body.push_str(&format!(
            "<w:hyperlink r:id=\"{rid}\"><w:r><w:rPr><w:color w:val=\"0067C0\"/><w:u w:val=\"single\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:hyperlink>",
            xml_escape(&xml_clean(text))
        ));
    }

    fn para_of(&mut self, runs: &[Run], props: &str, force_bold: bool, dim: bool) {
        self.body.push_str("<w:p>");
        if !props.is_empty() {
            self.body.push_str(&format!("<w:pPr>{props}</w:pPr>"));
        }
        for r in runs {
            if let Some(url) = &r.link {
                self.link_run(url, &r.text);
                continue;
            }
            let mut rpr = String::new();
            if dim {
                rpr.push_str("<w:color w:val=\"595959\"/>");
            }
            if force_bold || r.bold {
                rpr.push_str("<w:b/>");
            }
            if r.italic {
                rpr.push_str("<w:i/>");
            }
            if r.strike {
                rpr.push_str("<w:strike/>");
            }
            if r.code {
                rpr.push_str("<w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\" w:eastAsia=\"Microsoft YaHei\"/><w:shd w:val=\"clear\" w:fill=\"EFEFEF\"/><w:sz w:val=\"19\"/>");
            }
            self.body.push_str(&format!(
                "<w:r><w:rPr>{rpr}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                xml_escape(&xml_clean(&r.text))
            ));
        }
        self.body.push_str("</w:p>");
    }

    fn write_blocks(&mut self, blocks: &[Block], base: Option<&Path>, level: usize) {
        for b in blocks {
            self.write_block(b, base, level);
        }
    }

    fn write_block(&mut self, b: &Block, base: Option<&Path>, level: usize) {
        match b {
            Block::Heading { level: hl, runs, .. } => {
                let style = match hl {
                    1..=4 => format!("Heading{hl}"),
                    _ => "Heading4".to_string(),
                };
                self.para_of(runs, &format!("<w:pStyle w:val=\"{style}\"/>"), true, false);
            }
            Block::Para(runs) => self.para_of(runs, "", false, false),
            Block::Code { lines, .. } => {
                let n = lines.len();
                for (i, cl) in lines.iter().enumerate() {
                    let mut props = String::from(
                        "<w:shd w:val=\"clear\" w:fill=\"F6F6F6\"/><w:ind w:left=\"113\" w:right=\"113\"/><w:spacing w:after=\"0\" w:line=\"276\" w:lineRule=\"auto\"/>",
                    );
                    if i == 0 {
                        props.push_str("<w:pBdr><w:top w:val=\"single\" w:sz=\"4\" w:color=\"E8E8E8\"/></w:pBdr>");
                    }
                    if i + 1 == n {
                        props.push_str("<w:pBdr><w:bottom w:val=\"single\" w:sz=\"4\" w:color=\"E8E8E8\"/></w:pBdr>");
                    }
                    self.body.push_str(&format!("<w:p><w:pPr>{props}</w:pPr>"));
                    for t in cl {
                        self.body.push_str(&format!(
                            "<w:r><w:rPr><w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\" w:eastAsia=\"Microsoft YaHei\"/><w:sz w:val=\"19\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                            xml_escape(&xml_clean(&t.text))
                        ));
                    }
                    self.body.push_str("</w:p>");
                }
            }
            Block::Quote(inner) => {
                for q in inner {
                    match q {
                        Block::Para(runs) => self.para_of(
                            runs,
                            "<w:pBdr><w:left w:val=\"single\" w:sz=\"18\" w:color=\"C9C9C9\"/></w:pBdr><w:ind w:left=\"340\"/><w:spacing w:after=\"120\"/>",
                            false,
                            true,
                        ),
                        other => self.write_block(other, base, level),
                    }
                }
            }
            Block::List { ordered, start, items } => {
                for (i, item) in items.iter().enumerate() {
                    let marker = if *ordered {
                        format!("{}. ", *start + i as u64)
                    } else {
                        "• ".to_string()
                    };
                    self.write_list_item(item, &marker, level, base);
                }
            }
            Block::Table { head, rows, .. } => self.write_table(head, rows),
            Block::Rule => {
                self.body.push_str(
                    "<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:color=\"E5E5E5\"/></w:pBdr><w:spacing w:after=\"240\"/></w:pPr></w:p>",
                );
            }
            Block::Image { src, path, alt, .. } => self.write_image(src, path.as_deref(), alt, base),
        }
    }

    fn write_list_item(&mut self, item: &ListItem, marker: &str, level: usize, base: Option<&Path>) {
        let indent = 340 + level as i32 * 340;
        let props = format!(
            "<w:ind w:left=\"{indent}\" w:hanging=\"227\"/><w:spacing w:after=\"60\"/>"
        );
        for b in &item.blocks {
            match b {
                Block::Para(runs) => {
                    let task = match item.task {
                        Some(true) => "☑ ",
                        Some(false) => "☐ ",
                        None => "",
                    };
                    self.body.push_str(&format!("<w:p><w:pPr>{props}</w:pPr>"));
                    self.text_run(&format!("{marker}{task}"), false, false, false, false);
                    for r in runs {
                        if let Some(url) = &r.link {
                            self.link_run(url, &r.text);
                            continue;
                        }
                        self.text_run(&r.text, r.bold, r.italic, r.strike, r.code);
                    }
                    self.body.push_str("</w:p>");
                }
                other => self.write_block(other, base, level + 1),
            }
        }
    }

    fn write_table(&mut self, head: &[Vec<Run>], rows: &[Vec<Vec<Run>>]) {
        let cols = head.len().max(1);
        let mut grid = String::new();
        for _ in 0..cols {
            grid.push_str("<w:gridCol/>");
        }
        self.body.push_str(&format!(
            "<w:tbl><w:tblPr><w:tblW w:w=\"5000\" w:type=\"pct\"/><w:tblBorders>\
             <w:top w:val=\"single\" w:sz=\"4\" w:color=\"E3E3E3\"/><w:left w:val=\"single\" w:sz=\"4\" w:color=\"E3E3E3\"/>\
             <w:bottom w:val=\"single\" w:sz=\"4\" w:color=\"E3E3E3\"/><w:right w:val=\"single\" w:sz=\"4\" w:color=\"E3E3E3\"/>\
             <w:insideH w:val=\"single\" w:sz=\"4\" w:color=\"E3E3E3\"/><w:insideV w:val=\"single\" w:sz=\"4\" w:color=\"E3E3E3\"/>\
             </w:tblBorders></w:tblPr><w:tblGrid>{grid}</w:tblGrid>"
        ));
        let cell = |b: &mut Self, runs: &[Run], head_cell: bool| {
            b.body.push_str("<w:tc><w:tcPr>");
            if head_cell {
                b.body.push_str("<w:shd w:val=\"clear\" w:fill=\"F7F7F7\"/>");
            }
            b.body.push_str("</w:tcPr><w:p>");
            for r in runs {
                let mut rpr = String::new();
                if head_cell || r.bold {
                    rpr.push_str("<w:b/>");
                }
                if r.italic {
                    rpr.push_str("<w:i/>");
                }
                if r.strike {
                    rpr.push_str("<w:strike/>");
                }
                if r.code {
                    rpr.push_str("<w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\"/>");
                }
                b.body.push_str(&format!(
                    "<w:r><w:rPr>{rpr}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                    xml_escape(&xml_clean(&r.text))
                ));
            }
            b.body.push_str("</w:p></w:tc>");
        };
        self.body.push_str("<w:tr>");
        for r in head {
            cell(self, r, true);
        }
        self.body.push_str("</w:tr>");
        for row in rows {
            self.body.push_str("<w:tr>");
            for r in row {
                cell(self, r, false);
            }
            self.body.push_str("</w:tr>");
        }
        self.body.push_str("</w:tbl><w:p/>");
    }

    fn write_image(&mut self, src: &str, path: Option<&Path>, alt: &str, base: Option<&Path>) {
        let found: Option<(Vec<u8>, &'static str)> = if let Some(p) = path {
            std::fs::read(p).ok().map(|b| {
                let ext = match p.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()).as_deref() {
                    Some("jpg") | Some("jpeg") => "jpeg",
                    Some("gif") => "gif",
                    Some("bmp") => "bmp",
                    _ => "png",
                };
                (b, ext)
            })
        } else if src.starts_with("http://") || src.starts_with("https://") {
            mojuan_core::http::https_get(src, 8 * 1024 * 1024, 8000)
                .ok()
                .filter(|r| r.status == 200)
                .map(|r| {
                    let ext = if r.body.starts_with(&[0x89, b'P', b'N', b'G']) {
                        "png"
                    } else if r.body.starts_with(&[0xFF, 0xD8]) {
                        "jpeg"
                    } else if r.body.starts_with(b"GIF8") {
                        "gif"
                    } else {
                        "png"
                    };
                    (r.body, ext)
                })
        } else if let Some(b) = base {
            let p = b.join(src);
            std::fs::read(&p).ok().map(|bytes| {
                let ext = match src.rsplit('.').next().map(|s| s.to_ascii_lowercase()).as_deref() {
                    Some("jpg") | Some("jpeg") => "jpeg",
                    Some("gif") => "gif",
                    Some("bmp") => "bmp",
                    _ => "png",
                };
                (bytes, ext)
            })
        } else {
            None
        };
        let Some((bytes, ext)) = found else {
            self.body.push_str(&format!(
                "<w:p><w:r><w:rPr><w:color w:val=\"949494\"/><w:i/></w:rPr><w:t xml:space=\"preserve\">（图片无法加载：{}）</w:t></w:r></w:p>",
                xml_escape(&xml_clean(if alt.is_empty() { src } else { alt }))
            ));
            return;
        };
        let Some(img) = image::load_from_memory(&bytes).ok() else {
            return;
        };
        let (w, h) = (img.width(), img.height());
        self.img_i += 1;
        let file = format!("media/image{}.{}", self.img_i, if ext == "jpeg" { "jpg" } else { ext });
        self.zip.add(&format!("word/{file}"), &bytes);
        let rid = self.next_rid();
        self.rels.push((rid.clone(), file, false));

        let scale = (600.0 / w as f32).min(1.0);
        let dw = (w as f32 * scale) as i64 * 9525;
        let dh = (h as f32 * scale) as i64 * 9525;
        self.body.push_str(&format!(
            "<w:p><w:r><w:drawing>\
             <wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\">\
             <wp:extent cx=\"{dw}\" cy=\"{dh}\"/><wp:docPr id=\"{}\" name=\"Picture {}\"/>\
             <a:graphic xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">\
             <a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
             <pic:pic xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
             <pic:nvPicPr><pic:cNvPr id=\"{}\" name=\"Picture {}\"/><pic:cNvPicPr/></pic:nvPicPr>\
             <pic:blipFill><a:blip r:embed=\"{rid}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>\
             <pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{dw}\" cy=\"{dh}\"/></a:xfrm>\
             <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr>\
             </pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>",
            self.img_i, self.img_i, self.img_i, self.img_i
        ));
    }

    fn finish(mut self, title: &str) -> Vec<u8> {
        self.zip.add("[Content_Types].xml", include_bytes!("export_content_types.xml"));
        self.zip.add("_rels/.rels", include_bytes!("export_rels.xml"));
        self.zip.add("word/styles.xml", include_str!("export_styles.xml").as_bytes());

        let mut rels = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
             <Relationship Id=\"rIdStyles\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>",
        );
        for (rid, target, external) in &self.rels {
            let (t, mode) = if *external {
                (
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink",
                    " TargetMode=\"External\"",
                )
            } else {
                (
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
                    "",
                )
            };
            rels.push_str(&format!("<Relationship Id=\"{rid}\" Type=\"{t}\" Target=\"{target}\"{mode}/>"));
        }
        rels.push_str("</Relationships>");
        self.zip.add("word/_rels/document.xml.rels", rels.as_bytes());

        let body = std::mem::take(&mut self.body);
        let doc = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <w:document xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\"><w:body>{body}\
             <w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/>\
             <w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/>\
             </w:sectPr></w:body></w:document>"
        );
        self.zip.add("word/document.xml", doc.as_bytes());
        self.zip.add(
            "docProps/core.xml",
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
                 <cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
                 xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>{}</dc:title><dc:creator>墨卷</dc:creator></cp:coreProperties>",
                xml_escape(&xml_clean(title))
            )
            .as_bytes(),
        );
        self.zip.add(
            "docProps/app.xml",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
              <Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\">\
              <Application>Mojuan</Application></Properties>",
        );
        self.zip.finish()
    }
}

pub fn export_docx(text: &str, base: Option<&Path>, title: &str, out: &Path) -> Result<String, String> {
    let mut b = DocxBuilder::new();
    let it = ChunkIter::new(text, base);
    for chunk in it {
        b.write_blocks(&chunk, base, 0);
    }
    let data = b.finish(title);
    std::fs::write(out, data).map_err(|e| format!("保存 docx 失败：{e}"))?;
    Ok(format!("已导出 Word：{title}.docx"))
}

// ---------------------------------------------------------------------------
// 导出冒烟测试：覆盖全部块类型，产物落临时目录
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO: &str = r#"# 导出测试文档

这是**加粗**、*斜体*、`行内代码`、~~删除线~~ 与 [链接](https://example.com) 的段落。
第二行中文长段落用于验证自动折行：墨卷是一个纯 Rust 的 Markdown 阅读器，目标是把注意力还给文字，
在没有 WebView 的前提下提供接近纸面的排版质量。

## 列表

- 第一项
- 第二项 `代码`
- 第三项
1. 有序一
2. 有序二

- [x] 已完成任务
- [ ] 未完成任务

## 引用

> 阅读器不该让你意识到它的存在。
> —— 第二行引用

## 代码

```rust
fn main() {
    let x = 42; // 中文注释
    println!("你好，{x}");
}
```

## 表格

| 能力 | 状态 | 说明 |
| --- | :---: | --- |
| 导出 Word | ✅ | docx |
| 导出 PDF | ✅ | 打印驱动 |

---

![测试图](cover.png)
"#;

    fn setup() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("mojuan-export-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // 本地图片：内置示例封面（真实 PNG）
        std::fs::write(dir.join("cover.png"), include_bytes!("../assets/sample/assets/cover.png")).unwrap();
        dir
    }

    #[test]
    fn export_png_smoke() {
        let dir = setup();
        let base = dir.clone();
        let out_png = dir.join("demo.png");
        export_png(DEMO, Some(&base), &out_png).expect("png export");
        let img = image::open(&out_png).expect("png readable");
        assert!(img.width() == 960 && img.height() > 400, "png size {}x{}", img.width(), img.height());
        println!("png ok: {} ({}x{})", out_png.display(), img.width(), img.height());
    }

    #[test]
    fn export_docx_smoke() {
        let dir = setup();
        let base = dir.clone();
        let out_docx = dir.join("demo.docx");
        export_docx(DEMO, Some(&base), "导出测试文档", &out_docx).expect("docx export");
        let data = std::fs::read(&out_docx).unwrap();
        assert!(data.starts_with(b"PK"), "zip header");
        let eocd_off = data
            .windows(4)
            .rposition(|w| w == [0x50, 0x4B, 0x05, 0x06])
            .expect("eocd");
        let count = u16::from_le_bytes([data[eocd_off + 10], data[eocd_off + 11]]);
        assert!(count >= 6, "zip entries {count}");
        println!("docx ok: {} ({} bytes)", out_docx.display(), data.len());
    }

    #[test]
    fn export_pdf_smoke() {
        let dir = setup();
        let base = dir.clone();
        let out_pdf = dir.join("demo.pdf");
        match export_pdf(DEMO, Some(&base), "导出测试文档", &out_pdf) {
            Ok(_) => {
                let data = std::fs::read(&out_pdf).unwrap();
                assert!(data.starts_with(b"%PDF-"), "pdf header");
                assert!(data.windows(5).any(|w| w == b"%%EOF"), "pdf eof");
                let pages = data.windows(10).filter(|w| w == b"/Type/Page").count()
                    + data.windows(11).filter(|w| w == b"/Type /Page").count();
                assert!(pages >= 1, "pdf page objects found={pages}");
                println!("pdf ok: {} ({} bytes)", out_pdf.display(), data.len());
            }
            Err(e) => panic!("pdf export failed: {e}"),
        }
    }
}
