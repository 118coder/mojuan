//! 文档解析：把 Markdown 转成与 UI 无关的中间模型。
//!
//! 这一层只依赖 comrak + syntect，不依赖任何 GUI 库，
//! 因此可以被单元测试，也可以被别的前端复用。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use comrak::nodes::{AstNode, NodeValue, TableAlignment};
use comrak::{parse_document, Arena, Options};

use crate::highlight;

// ---------------------------------------------------------------------------
// 数据模型
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Default)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    /// 链接地址（原始写法），点击时由 UI 决定打开外部还是跳转本地
    pub link: Option<String>,
    /// 段落中混排的内联图片地址
    pub image: Option<String>,
}

impl Run {
    fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub enum Block {
    Heading {
        level: u8,
        runs: Vec<Run>,
        plain: String,
        id: String,
    },
    Para(Vec<Run>),
    Code {
        lang: String,
        lines: Vec<Vec<highlight::Token>>,
    },
    Quote(Vec<Block>),
    List {
        ordered: bool,
        start: u64,
        items: Vec<ListItem>,
    },
    Table {
        aligns: Vec<Align>,
        head: Vec<Vec<Run>>,
        rows: Vec<Vec<Vec<Run>>>,
    },
    Rule,
    Image {
        src: String,
        path: Option<PathBuf>,
        alt: String,
        title: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct ListItem {
    /// None = 普通列表项；Some(true) = 已勾选；Some(false) = 未勾选
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone)]
pub struct TocEntry {
    pub level: u8,
    pub text: String,
    pub id: String,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub chars: usize,
    pub cjk: usize,
    pub words: usize,
    pub minutes: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Doc {
    pub blocks: Vec<Block>,
    pub toc: Vec<TocEntry>,
    pub stats: Stats,
}

// ---------------------------------------------------------------------------
// 解析入口
// ---------------------------------------------------------------------------

/// 解析 Markdown。`base` 用于把相对图片路径解析成绝对路径。
pub fn parse(md: &str, base: Option<&Path>) -> Doc {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    options.extension.footnotes = true;
    options.extension.cjk_friendly_emphasis = true;
    options.extension.front_matter_delimiter = Some("---".to_string());

    let arena = Arena::new();
    let root = parse_document(&arena, md, &options);
    let ctx = Ctx { base };
    let mut ids: HashMap<String, usize> = HashMap::new();

    let blocks = collect_blocks(root, &ctx, &mut ids, 0);
    let mut toc = Vec::new();
    collect_toc(&blocks, &mut toc);

    Doc {
        blocks,
        toc,
        stats: stats(md),
    }
}

struct Ctx<'a> {
    base: Option<&'a Path>,
}

fn collect_blocks<'a>(
    node: &'a AstNode<'a>,
    ctx: &Ctx,
    ids: &mut HashMap<String, usize>,
    depth: usize,
) -> Vec<Block> {
    let mut out = Vec::new();
    if depth > 24 {
        return out;
    }
    for child in node.children() {
        if let Some(b) = block_of(child, ctx, ids, depth) {
            out.push(b);
        }
    }
    out
}

enum Kind {
    Para,
    Heading(u8),
    Code(String, String),
    Quote,
    List { ordered: bool, start: u64 },
    Table(Vec<Align>),
    Row,
    Cell,
    /// 游离的 Item（正常解析不会单独出现，列表在 List 分支里统一处理）
    StrayItem,
    Rule,
    Image(String, String),
    Skip,
}

fn block_of<'a>(
    node: &'a AstNode<'a>,
    ctx: &Ctx,
    ids: &mut HashMap<String, usize>,
    depth: usize,
) -> Option<Block> {
    // 先取出轻量信息，避免递归时持有 RefCell 借用
    let kind = {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::Paragraph => Kind::Para,
            NodeValue::Heading(h) => Kind::Heading(h.level),
            NodeValue::CodeBlock(c) => Kind::Code(lang_of(&c.info), c.literal.clone()),
            NodeValue::BlockQuote => Kind::Quote,
            NodeValue::List(l) => Kind::List {
                ordered: matches!(l.list_type, comrak::nodes::ListType::Ordered),
                start: l.start as u64,
            },
            NodeValue::Item(_) | NodeValue::TaskItem(_) => Kind::StrayItem,
            NodeValue::Table(t) => Kind::Table(t.alignments.iter().map(|a| align_of(*a)).collect()),
            NodeValue::TableRow(_) => Kind::Row,
            NodeValue::TableCell => Kind::Cell,
            NodeValue::ThematicBreak => Kind::Rule,
            NodeValue::Image(l) => Kind::Image(l.url.clone(), l.title.clone()),
            NodeValue::HtmlBlock(_) | NodeValue::FrontMatter(_) => Kind::Skip,
            _ => Kind::Skip,
        }
    };

    match kind {
        Kind::Para => {
            // 只有一张图片的段落 → 直接当成图片块
            let mut kids = node.children().filter(|c| !is_blank_text(c));
            let first = kids.next();
            if let (Some(only), None) = (first, kids.next()) {
                if let NodeValue::Image(l) = &only.data.borrow().value {
                    let alt = plain_text(only);
                    return Some(Block::Image {
                        src: l.url.clone(),
                        path: resolve(ctx.base, &l.url),
                        alt,
                        title: l.title.clone(),
                    });
                }
            }
            let mut runs = Vec::new();
            collect_inlines(node, &mut runs);
            trim_runs(&mut runs);
            if runs.is_empty() {
                return None;
            }
            Some(Block::Para(runs))
        }
        Kind::Heading(level) => {
            let mut runs = Vec::new();
            collect_inlines(node, &mut runs);
            trim_runs(&mut runs);
            let plain = runs_text(&runs);
            let base_id = slug(&plain);
            let n = ids.entry(base_id.clone()).or_insert(0);
            let id = if *n == 0 {
                base_id.clone()
            } else {
                format!("{base_id}-{}", *n)
            };
            *n += 1;
            Some(Block::Heading {
                level,
                runs,
                plain,
                id,
            })
        }
        Kind::Code(lang, literal) => Some(Block::Code {
            lang: highlight::display_lang(&lang),
            lines: highlight::highlight(&literal, &lang),
        }),
        Kind::Quote => Some(Block::Quote(collect_blocks(node, ctx, ids, depth + 1))),
        Kind::List { ordered, start } => list_block(node, ctx, ids, depth, ordered, start),
        Kind::Table(aligns) => {
            let mut head = Vec::new();
            let mut rows = Vec::new();
            for row in node.children() {
                let is_header = {
                    let d = row.data.borrow();
                    matches!(&d.value, NodeValue::TableRow(true))
                };
                let mut cells: Vec<Vec<Run>> = Vec::new();
                for cell in row.children() {
                    let mut runs = Vec::new();
                    collect_inlines(cell, &mut runs);
                    trim_runs(&mut runs);
                    cells.push(runs);
                }
                if is_header {
                    head = cells;
                } else {
                    rows.push(cells);
                }
            }
            Some(Block::Table {
                aligns,
                head,
                rows,
            })
        }
        Kind::Row | Kind::Cell | Kind::StrayItem => None,
        Kind::Rule => Some(Block::Rule),
        Kind::Image(src, title) => {
            let alt = plain_text(node);
            Some(Block::Image {
                path: resolve(ctx.base, &src),
                src,
                alt,
                title,
            })
        }
        Kind::Skip => {
            let mut runs = Vec::new();
            collect_inlines(node, &mut runs);
            trim_runs(&mut runs);
            if runs.is_empty() {
                None
            } else {
                Some(Block::Para(runs))
            }
        }
    }
}

/// 列表：List → 若干 Item / TaskItem → 块
fn list_block<'a>(
    node: &'a AstNode<'a>,
    ctx: &Ctx,
    ids: &mut HashMap<String, usize>,
    depth: usize,
    ordered: bool,
    start: u64,
) -> Option<Block> {
    let mut items: Vec<ListItem> = Vec::new();

    for item_node in node.children() {
        // 个别情况下 TaskItem 就是 Item 本身
        let mut task = {
            let d = item_node.data.borrow();
            match &d.value {
                NodeValue::TaskItem(t) => Some(t.symbol.is_some()),
                _ => None,
            }
        };
        let mut blocks = Vec::new();
        for child in item_node.children() {
            let child_task = {
                let d = child.data.borrow();
                match &d.value {
                    NodeValue::TaskItem(t) => Some(t.symbol.is_some()),
                    _ => None,
                }
            };
            match child_task {
                Some(done) => {
                    task = Some(done);
                    blocks.extend(collect_blocks(child, ctx, ids, depth + 2));
                }
                None => {
                    if let Some(b) = block_of(child, ctx, ids, depth + 2) {
                        blocks.push(b);
                    }
                }
            }
        }
        items.push(ListItem { task, blocks });
    }

    if items.is_empty() {
        return None;
    }

    Some(Block::List {
        ordered,
        start: start.max(1),
        items,
    })
}

fn collect_inlines<'a>(node: &'a AstNode<'a>, runs: &mut Vec<Run>) {
    for child in node.children() {
        let value = child.data.borrow().value.clone();
        match value {
            NodeValue::Text(t) => runs.push(Run::new(t.to_string())),
            NodeValue::Code(c) => {
                let mut run = Run::new(c.literal.trim_matches('\n'));
                run.code = true;
                runs.push(run);
            }
            NodeValue::SoftBreak => runs.push(Run::new(" ")),
            NodeValue::LineBreak => runs.push(Run::new("\n")),
            NodeValue::Emph => {
                let mut inner = Vec::new();
                collect_inlines(child, &mut inner);
                for mut r in inner {
                    r.italic = true;
                    runs.push(r);
                }
            }
            NodeValue::Strong => {
                let mut inner = Vec::new();
                collect_inlines(child, &mut inner);
                for mut r in inner {
                    r.bold = true;
                    runs.push(r);
                }
            }
            NodeValue::Strikethrough => {
                let mut inner = Vec::new();
                collect_inlines(child, &mut inner);
                for mut r in inner {
                    r.strike = true;
                    runs.push(r);
                }
            }
            NodeValue::Link(l) => {
                let mut inner = Vec::new();
                collect_inlines(child, &mut inner);
                for mut r in inner {
                    if r.link.is_none() {
                        r.link = Some(l.url.clone());
                    }
                    runs.push(r);
                }
            }
            NodeValue::Image(l) => {
                let alt = plain_text(child);
                let mut run = Run::new(if alt.is_empty() { l.title.clone() } else { alt });
                run.image = Some(l.url.clone());
                runs.push(run);
            }
            NodeValue::HtmlInline(_) => {}
            NodeValue::FootnoteReference(r) => runs.push(Run::new(format!("[^{}]", r.name))),
            _ => {
                let mut inner = Vec::new();
                collect_inlines(child, &mut inner);
                runs.extend(inner);
            }
        }
    }
}

fn is_blank_text<'a>(node: &'a AstNode<'a>) -> bool {
    let d = node.data.borrow();
    match &d.value {
        NodeValue::Text(t) => t.trim().is_empty(),
        _ => false,
    }
}

fn plain_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut s = String::new();
    for c in node.descendants() {
        let d = c.data.borrow();
        match &d.value {
            NodeValue::Text(t) => s.push_str(t),
            NodeValue::Code(c) => s.push_str(&c.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => s.push(' '),
            _ => {}
        }
    }
    s.trim().to_string()
}

fn runs_text(runs: &[Run]) -> String {
    runs.iter()
        .map(|r| r.text.as_str())
        .collect::<String>()
        .trim()
        .to_string()
}

fn trim_runs(runs: &mut Vec<Run>) {
    while let Some(first) = runs.first_mut() {
        first.text = first.text.trim_start().to_string();
        if first.text.is_empty() && first.image.is_none() {
            runs.remove(0);
        } else {
            break;
        }
    }
    while let Some(last) = runs.last_mut() {
        last.text = last.text.trim_end().to_string();
        if last.text.is_empty() && last.image.is_none() {
            runs.pop();
        } else {
            break;
        }
    }
}

fn collect_toc(blocks: &[Block], out: &mut Vec<TocEntry>) {
    for b in blocks {
        match b {
            Block::Heading {
                level, plain, id, ..
            } => out.push(TocEntry {
                level: *level,
                text: plain.clone(),
                id: id.clone(),
            }),
            Block::Quote(inner) => collect_toc(inner, out),
            Block::List { items, .. } => {
                for it in items {
                    collect_toc(&it.blocks, out);
                }
            }
            _ => {}
        }
    }
}

fn align_of(a: TableAlignment) -> Align {
    match a {
        TableAlignment::None => Align::None,
        TableAlignment::Left => Align::Left,
        TableAlignment::Center => Align::Center,
        TableAlignment::Right => Align::Right,
    }
}

fn lang_of(info: &str) -> String {
    info.split(|c: char| c.is_whitespace() || c == ',')
        .next()
        .unwrap_or("")
        .trim_start_matches('{')
        .trim_end_matches('}')
        .to_ascii_lowercase()
}

/// 生成标题锚点 id（保留中文）
pub fn slug(text: &str) -> String {
    let mut s = String::new();
    let mut prev_dash = false;
    for ch in text.chars() {
        if ch.is_alphanumeric() || is_cjk(ch) {
            for c in ch.to_lowercase() {
                s.push(c);
            }
            prev_dash = false;
        } else if !prev_dash && !s.is_empty() {
            s.push('-');
            prev_dash = true;
        }
    }
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "section".to_string()
    } else {
        s
    }
}

pub fn is_cjk(ch: char) -> bool {
    matches!(ch as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF |
        0x3040..=0x30FF | 0xAC00..=0xD7AF | 0x20000..=0x2FA1F)
}

/// 把相对路径解析为绝对路径（用于本地图片）
pub fn resolve(base: Option<&Path>, url: &str) -> Option<PathBuf> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("data:")
        || lower.starts_with("mailto:")
    {
        return None;
    }
    let without_frag = url.split('#').next().unwrap_or(url);
    let decoded = percent_decode(without_frag);
    let p = Path::new(&decoded);
    if p.is_absolute() {
        Some(p.to_path_buf())
    } else {
        base.map(|b| b.join(p))
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    fn hex(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

// ---------------------------------------------------------------------------
// 统计信息
// ---------------------------------------------------------------------------

pub fn stats(text: &str) -> Stats {
    let mut chars = 0usize;
    let mut cjk = 0usize;
    let mut words = 0usize;
    let mut in_word = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            in_word = false;
            continue;
        }
        chars += 1;
        if is_cjk(ch) {
            cjk += 1;
            in_word = false;
            words += 1;
        } else if ch.is_alphanumeric() {
            if !in_word {
                words += 1;
                in_word = true;
            }
        } else {
            in_word = false;
        }
    }
    let latin = words.saturating_sub(cjk);
    let minutes = (cjk as f32 / 380.0) + (latin as f32 / 220.0);
    Stats {
        chars,
        cjk,
        words,
        minutes: if chars == 0 {
            0
        } else {
            minutes.ceil().max(1.0) as u32
        },
    }
}

/// 导出独立 HTML（阅读版，内嵌排版样式）
pub fn to_html(md: &str) -> String {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    options.extension.footnotes = true;
    options.extension.cjk_friendly_emphasis = true;
    comrak::markdown_to_html(md, &options)
}
