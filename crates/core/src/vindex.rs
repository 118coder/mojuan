//! 轻量文档块索引：单次线性扫描，只记录字节偏移，不建 AST。
//!
//! 千万字符（10MB）文档如果交给 comrak 会生成数百万节点。这里只做一件事：
//! 把文本切成「逻辑块」，每块记住起止字节。阅读区按需把可见块的切片
//! 单独喂给 comrak，于是任意时刻的解析量与屏幕高度成正比，与文档长度无关。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VKind {
    Heading(u8),
    Para,
    Code,
    Quote,
    List,
    Table,
    Rule,
    Image,
    Math,
    Mermaid,
    FrontMatter,
    Html,
}

#[derive(Debug, Clone)]
pub struct VBlock {
    /// 字节偏移，含
    pub start: usize,
    /// 字节偏移，不含
    pub end: usize,
    pub kind: VKind,
    /// 标题级别 / 缩进层级
    pub level: u8,
}

#[derive(Debug, Clone)]
pub struct VIndex {
    pub blocks: Vec<VBlock>,
    pub text_len: usize,
    /// 每块的行数（估算高度用）
    lines: Vec<u32>,
    /// 每块的大致字符数（估算高度用）
    chars: Vec<u32>,
}

fn is_blank(line: &str) -> bool {
    line.trim_matches([' ', '\t', '\r', '\n']).is_empty()
}

fn trimmed_start(line: &str) -> &str {
    line.trim_start_matches([' ', '\t'])
}

fn fence_marker(line: &str) -> Option<(&str, &str)> {
    let t = trimmed_start(line);
    for mark in ["```", "~~~"] {
        if let Some(rest) = t.strip_prefix(mark) {
            return Some((mark, rest.trim()));
        }
    }
    None
}

fn heading_level(line: &str) -> u8 {
    let t = trimmed_start(line);
    let n = t.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&n) && t.as_bytes().get(n) == Some(&b' ') {
        n as u8
    } else {
        0
    }
}

fn is_rule(line: &str) -> bool {
    let t = line.trim_matches([' ', '\t', '\r', '\n']);
    if t.len() < 3 {
        return false;
    }
    let c = t.as_bytes()[0];
    (c == b'-' || c == b'*' || c == b'_') && t.bytes().all(|b| b == c || b == b' ' || b == b'\t')
}

fn is_list_start(line: &str) -> bool {
    let t = trimmed_start(line);
    if t.starts_with("- [") || t.starts_with("* [") || t.starts_with("+ [") {
        return t.len() > 4;
    }
    for m in ["- ", "* ", "+ "] {
        if t.starts_with(m) {
            return true;
        }
    }
    let digits = t.bytes().take_while(|b| b.is_ascii_digit()).count();
    digits > 0 && t.as_bytes().get(digits) == Some(&b'.') && t.as_bytes().get(digits + 1) == Some(&b' ')
}

fn is_image_only(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("![") && t.ends_with(')') && t.contains("](")
}

fn starts_new_block(line: &str) -> bool {
    let t = trimmed_start(line);
    fence_marker(t).is_some()
        || t.starts_with("$$")
        || t.starts_with('|')
        || t.starts_with('>')
        || t.starts_with('<')
        || heading_level(t) > 0
        || is_rule(t)
        || is_image_only(t)
        || is_list_start(t)
}

impl VIndex {
    pub fn build(text: &str) -> Self {
        let bytes = text.as_bytes();
        let mut lines: Vec<(usize, usize)> = Vec::with_capacity(bytes.len() / 48 + 1);
        let mut pos = 0usize;
        while pos < bytes.len() {
            let mut end = pos;
            while end < bytes.len() && bytes[end] != b'\n' {
                end += 1;
            }
            if end < bytes.len() {
                end += 1;
            }
            lines.push((pos, end));
            pos = end;
        }
        let mut blocks: Vec<VBlock> = Vec::with_capacity(lines.len() / 3 + 1);
        let mut line_counts: Vec<u32> = Vec::new();
        let mut char_counts: Vec<u32> = Vec::new();

        let line_str = |k: usize| -> &str { &text[lines[k].0..lines[k].1] };
        let mut push = |start: usize, end: usize, kind: VKind, level: u8, from: usize, to: usize| {
            // 空行归入上一块（保证所有字节都被覆盖，且 comrak 解析结果不变）
            if let Some(prev) = blocks.last_mut() {
                prev.end = start;
            } else if start > 0 {
                blocks.push(VBlock { start: 0, end: start, kind: VKind::Para, level: 0 });
                line_counts.push(0);
                char_counts.push(0);
            }
            let nlines = to.saturating_sub(from) as u32;
            let nchars = end.saturating_sub(start) as u32;
            blocks.push(VBlock { start, end, kind, level });
            line_counts.push(nlines);
            char_counts.push(nchars);
        };

        let mut i = 0usize;
        // 文档开头的 YAML front matter
        if !lines.is_empty() && line_str(0).trim() == "---" {
            let mut close = None;
            for k in 1..lines.len() {
                if line_str(k).trim() == "---" {
                    close = Some(k);
                    break;
                }
            }
            if let Some(k) = close {
                push(lines[0].0, lines[k].1, VKind::FrontMatter, 0, 0, k + 1);
                i = k + 1;
            }
        }

        while i < lines.len() {
            if is_blank(line_str(i)) {
                i += 1;
                continue;
            }
            let start = lines[i].0;
            let from = i;
            let t = trimmed_start(line_str(i));
            let mut kind = VKind::Para;
            let mut level = 0u8;
            let mut to = i + 1;

            if let Some((mark, lang)) = fence_marker(t) {
                // 围栏：一直吃到闭合行
                kind = if lang.to_ascii_lowercase().starts_with("mermaid") {
                    VKind::Mermaid
                } else {
                    VKind::Code
                };
                to = lines.len();
                for k in i + 1..lines.len() {
                    if trimmed_start(line_str(k)).starts_with(mark) {
                        to = k + 1;
                        break;
                    }
                }
            } else if t.starts_with("$$") {
                kind = VKind::Math;
                to = lines.len();
                for k in i + 1..lines.len() {
                    if line_str(k).trim() == "$$" {
                        to = k + 1;
                        break;
                    }
                }
            } else if let h @ 1..=6 = heading_level(t) {
                kind = VKind::Heading(h);
                level = h;
            } else if is_rule(t) {
                kind = VKind::Rule;
            } else if is_image_only(t) {
                kind = VKind::Image;
            } else if t.starts_with('|') {
                kind = VKind::Table;
                while to < lines.len() && trimmed_start(line_str(to)).starts_with('|') {
                    to += 1;
                }
            } else if t.starts_with('>') {
                kind = VKind::Quote;
                while to < lines.len() && trimmed_start(line_str(to)).starts_with('>') {
                    to += 1;
                }
            } else if is_list_start(t) {
                kind = VKind::List;
                level = ((t.len() - trimmed_start(line_str(i)).len()) / 2).min(6) as u8;
                while to < lines.len() {
                    let l = line_str(to);
                    if is_blank(l) {
                        break;
                    }
                    if is_list_start(l) || l.starts_with(' ') || l.starts_with('\t') {
                        to += 1;
                    } else {
                        break;
                    }
                }
            } else if t.starts_with('<') {
                kind = VKind::Html;
                while to < lines.len() && !is_blank(line_str(to)) {
                    to += 1;
                }
            } else {
                // 普通段落：连续非空行，遇到明显的新块起始就收尾
                while to < lines.len() {
                    let l = line_str(to);
                    if is_blank(l) || starts_new_block(l) {
                        break;
                    }
                    to += 1;
                }
            }

            let end = lines[to - 1].1;
            push(start, end, kind, level, from, to);
            i = to;
        }

        if let Some(last) = blocks.last_mut() {
            last.end = text.len();
        } else if !text.is_empty() {
            blocks.push(VBlock { start: 0, end: text.len(), kind: VKind::Para, level: 0 });
            line_counts.push(lines.len() as u32);
            char_counts.push(text.len() as u32);
        }

        VIndex {
            blocks,
            text_len: text.len(),
            lines: line_counts,
            chars: char_counts,
        }
    }

    pub fn slice<'a>(&self, text: &'a str, i: usize) -> &'a str {
        let Some(b) = self.blocks.get(i) else { return "" };
        let mut s = b.start.min(text.len());
        let mut e = b.end.min(text.len()).max(s);
        while s > 0 && !text.is_char_boundary(s) {
            s -= 1;
        }
        while e > s && !text.is_char_boundary(e) {
            e -= 1;
        }
        &text[s..e]
    }

    /// 未实测时的高度估算。宁可略高，也不要小到让滚动条一帧一跳。
    pub fn estimate(&self, i: usize, size: f32, width: f32) -> f32 {
        let Some(b) = self.blocks.get(i) else { return 24.0 };
        let lines = self.lines.get(i).copied().unwrap_or(1).max(1) as f32;
        // 字节数折算成字符数：中文 3 字节/字，英文 1 字节/字，取一个偏保守的 1.5
        let chars = self.chars.get(i).copied().unwrap_or(0) as f32 / 1.5;
        let per_line = (width / (size * 0.98)).max(1.0);
        let wrapped = (chars / per_line).max(lines);
        let lh = size * 1.75;
        let h = match b.kind {
            VKind::Heading(n) => {
                let k = match n {
                    1 => 1.92,
                    2 => 1.42,
                    3 => 1.18,
                    _ => 1.02,
                };
                size * k * 1.5 + if n <= 2 { 62.0 } else { 40.0 }
            }
            VKind::Para => wrapped * lh + size * 0.8,
            VKind::Quote => wrapped * lh + 20.0,
            VKind::List => wrapped * lh + 12.0,
            VKind::Code => lines * size * 1.45 + 42.0,
            VKind::Table => lines * size * 1.9 + 20.0,
            VKind::Rule => size * 1.6,
            VKind::Image => 260.0,
            VKind::Math => size * 3.6 + 28.0,
            VKind::Mermaid => 240.0,
            VKind::FrontMatter => lines * size * 1.35 + 18.0,
            VKind::Html => lines * size * 1.4 + 8.0,
        };
        h.max(12.0)
    }
}

/// Fenwick 树：块高度单点更新，O(log n) 前缀和与「给定 y 找块」。
#[derive(Debug, Clone, Default)]
pub struct HeightTree {
    n: usize,
    bit: Vec<f32>,
    vals: Vec<f32>,
}

impl HeightTree {
    pub fn new(initial: &[f32]) -> Self {
        let n = initial.len();
        let mut t = Self {
            n,
            bit: vec![0.0; n + 1],
            vals: initial.to_vec(),
        };
        // 标准 O(n) 建树
        for i in 0..n {
            t.bit[i + 1] = initial[i];
        }
        for i in 1..=n {
            let parent = i + (i & i.wrapping_neg());
            if parent <= n {
                t.bit[parent] += t.bit[i];
            }
        }
        t
    }

    pub fn set(&mut self, i: usize, h: f32) {
        if i >= self.n {
            return;
        }
        let delta = h - self.vals[i];
        if delta == 0.0 {
            return;
        }
        self.vals[i] = h;
        let mut k = i + 1;
        while k <= self.n {
            self.bit[k] += delta;
            k += k & k.wrapping_neg();
        }
    }

    pub fn total(&self) -> f32 {
        self.prefix(self.n)
    }

    /// 前 i 个块的高度和
    pub fn prefix(&self, i: usize) -> f32 {
        let mut k = i.min(self.n);
        let mut s = 0.0;
        while k > 0 {
            s += self.bit[k];
            k -= k & k.wrapping_neg();
        }
        s
    }

    /// 找到包含 y 的块下标
    pub fn find(&self, y: f32) -> usize {
        if self.n == 0 {
            return 0;
        }
        let target = y.max(0.0).min(self.total());
        let mut idx = 0usize;
        let mut step = 1usize;
        while step * 2 <= self.n {
            step *= 2;
        }
        let mut acc = 0.0f32;
        while step > 0 {
            let next = idx + step;
            if next <= self.n && acc + self.bit[next] <= target {
                idx = next;
                acc += self.bit[next];
            }
            step /= 2;
        }
        idx.min(self.n - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<VKind> {
        VIndex::build(text).blocks.iter().map(|b| b.kind).collect()
    }

    #[test]
    fn empty_and_plain() {
        assert!(VIndex::build("").blocks.is_empty());
        assert_eq!(kinds("你好，世界\n"), vec![VKind::Para]);
        let idx = VIndex::build("第一行\n第二行\n");
        assert_eq!(idx.blocks.len(), 1);
        assert_eq!(idx.slice("第一行\n第二行\n", 0), "第一行\n第二行\n");
    }

    #[test]
    fn headings_and_rules() {
        let t = "# 一\n\n正文\n\n## 二\n\n---\n";
        let k = kinds(t);
        assert!(k.iter().any(|x| matches!(x, VKind::Heading(1))));
        assert!(k.iter().any(|x| matches!(x, VKind::Heading(2))));
        assert!(k.iter().any(|x| *x == VKind::Rule));
    }

    #[test]
    fn fences_tables_quotes() {
        let t = "```rust\nlet a = 1;\n\nlet b = 2;\n```\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n\n> 引用一\n> 引用二\n\n```mermaid\ngraph TD\n  A-->B\n```\n\n$$\nx^2\n$$\n";
        let k = kinds(t);
        assert!(k.contains(&VKind::Code), "{k:?}");
        assert!(k.contains(&VKind::Table), "{k:?}");
        assert!(k.contains(&VKind::Quote), "{k:?}");
        assert!(k.contains(&VKind::Mermaid), "{k:?}");
        assert!(k.contains(&VKind::Math), "{k:?}");
    }

    #[test]
    fn front_matter_and_images() {
        let t = "---\ntitle: 测试\n---\n\n![封面](a.png)\n";
        assert!(kinds(t).contains(&VKind::FrontMatter));
        assert!(kinds(t).contains(&VKind::Image));
    }

    #[test]
    fn coverage_and_utf8() {
        let t = "# 中文标题\n\n段落 **粗体** 内容\n\n```\n代码\n```\n\n结尾。\n";
        let idx = VIndex::build(t);
        let mut cursor = 0;
        for (i, b) in idx.blocks.iter().enumerate() {
            assert_eq!(b.start, cursor, "block {i} gap");
            assert!(b.end > b.start);
            assert!(t.is_char_boundary(b.start) && t.is_char_boundary(b.end));
            assert!(!idx.slice(t, i).is_empty());
            cursor = b.end;
        }
        assert_eq!(cursor, t.len());
    }

    #[test]
    fn height_tree() {
        let t = HeightTree::new(&[10.0, 20.0, 30.0, 40.0]);
        assert_eq!(t.total(), 100.0);
        assert_eq!(t.prefix(0), 0.0);
        assert_eq!(t.prefix(2), 30.0);
        assert_eq!(t.find(0.0), 0);
        assert_eq!(t.find(15.0), 1);
        assert_eq!(t.find(30.0), 2);
        assert_eq!(t.find(99.0), 3);
        let mut t = t;
        t.set(1, 50.0);
        assert_eq!(t.total(), 130.0);
        assert_eq!(t.prefix(2), 60.0);
        assert_eq!(t.find(40.0), 1);
    }
}
