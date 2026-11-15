//! 大文档模式：不建 AST，也能拿到目录与统计信息。
//!
//! 千万字符级文档如果走 `doc::parse` 会生成数百万个 AST 节点，内存和耗时都不可接受。
//! 这里只依赖 `vindex::VIndex`（一张字节偏移表）做一次线性扫描，
//! 产出侧栏大纲和状态栏统计所需的元数据。

use crate::doc::{self, Doc, Stats, TocEntry};
use crate::vindex::{VIndex, VKind};

/// 从块索引构造「空 blocks + 有 toc/stats」的 Doc。
/// `blocks` 为空是刻意的：阅读区会按需解析可见块。
pub fn build_meta(text: &str, index: &VIndex) -> Doc {
    let mut toc = Vec::new();
    let mut used: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for (i, b) in index.blocks.iter().enumerate() {
        let VKind::Heading(level) = b.kind else { continue };
        if level == 0 || level > 6 {
            continue;
        }
        let raw = index.slice(text, i);
        let line = raw.lines().next().unwrap_or("");
        let plain = plain_heading(line);
        if plain.is_empty() {
            continue;
        }
        let base = doc::slug(&plain);
        let n = used.entry(base.clone()).or_insert(0);
        let id = if *n == 0 {
            base
        } else {
            format!("{base}-{n}")
        };
        *n += 1;
        toc.push(TocEntry {
            level,
            text: plain,
            id,
        });
    }

    Doc {
        blocks: Vec::new(),
        toc,
        stats: fast_stats(text),
    }
}

/// 标题行 → 纯文本：去掉 `#`、闭合标记、粗斜体符号、链接地址，保留中文。
fn plain_heading(line: &str) -> String {
    let s = line.trim_start().trim_start_matches('#').trim();
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // 行内代码
        if c == '`' {
            i += 1;
            continue;
        }
        // 强调符号成对出现，直接丢掉
        if c == '*' || c == '_' || c == '~' {
            i += 1;
            continue;
        }
        // 链接/图片：[text](url) 只留 text
        let is_img = c == '!' && i + 1 < chars.len() && chars[i + 1] == '[';
        if c == '[' || is_img {
            let open = if is_img { i + 1 } else { i };
            if let Some(close) = (open + 1..chars.len()).find(|&k| chars[k] == ']') {
                if close + 1 < chars.len() && chars[close + 1] == '(' {
                    if let Some(end) = (close + 2..chars.len()).find(|&k| chars[k] == ')') {
                        for ch in &chars[open + 1..close] {
                            out.push(*ch);
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        out.push(c);
        i += 1;
    }
    out.trim().to_string()
}

/// 单次线性扫描的字符统计；比逐个 `chars().nth()` 快得多。
pub fn fast_stats(text: &str) -> Stats {
    let mut chars = 0usize;
    let mut cjk = 0usize;
    let mut words = 0usize;
    let mut in_word = false;
    for ch in text.chars() {
        chars += 1;
        if doc::is_cjk(ch) {
            cjk += 1;
            in_word = false;
        } else if ch.is_whitespace() {
            in_word = false;
        } else if !in_word {
            words += 1;
            in_word = true;
        }
    }
    // 中文按 350 字/分钟，其余按 220 词/分钟，取较大值
    let minutes = ((cjk as f64 / 350.0).max(words as f64 / 220.0)).ceil() as u32;
    Stats {
        chars,
        cjk,
        words,
        minutes,
    }
}

/// 在大文档里按锚点 id 找块下标（供目录跳转）。
pub fn heading_block_by_id(text: &str, index: &VIndex, target: &str) -> Option<usize> {
    let mut used: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (i, b) in index.blocks.iter().enumerate() {
        let VKind::Heading(level) = b.kind else { continue };
        if level == 0 || level > 6 {
            continue;
        }
        let line = index.slice(text, i).lines().next().unwrap_or("");
        let plain = plain_heading(line);
        if plain.is_empty() {
            continue;
        }
        let base = doc::slug(&plain);
        let n = used.entry(base.clone()).or_insert(0);
        let id = if *n == 0 {
            base
        } else {
            format!("{base}-{n}")
        };
        *n += 1;
        if id == target {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_from_index() {
        let text = "# 标题一\n\n正文 **粗体** 内容\n\n## 标题二 `代码`\n\n[链接](https://example.com)\n";
        let idx = VIndex::build(text);
        let doc = build_meta(text, &idx);
        assert!(doc.blocks.is_empty());
        assert_eq!(doc.toc.len(), 2);
        assert_eq!(doc.toc[0].text, "标题一");
        assert_eq!(doc.toc[1].text, "标题二 代码");
        assert!(doc.stats.chars > 20);
        assert_eq!(heading_block_by_id(text, &idx, "标题一"), Some(0));
    }

    #[test]
    fn stats_are_linear_and_safe() {
        let s = "中文abc ".repeat(100_000);
        let st = fast_stats(&s);
        assert!(st.cjk > 0);
        assert_eq!(st.chars, s.chars().count());
    }
}
